//! Agent Persona Configuration Layer
//!
//! Provides cognitive diversity through behavioral biases and voice tones.
//! Personas influence peer review decisions and debate dynamics.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use qdrant_client::{
    qdrant::{
        CreateCollection, Distance, PointStruct, ScoredPoint, SearchPoints,
        UpsertPoints, VectorParams, VectorsConfig, Value, ScrollPoints,
        vectors_config::Config, HnswConfigDiff,
    },
    Qdrant,
};
use tracing::{info, warn, error};
use uuid::Uuid;

/// Agent Persona structure defining behavioral characteristics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPersona {
    pub agent_id: String,
    pub name: String, // Unique callsign (e.g., "The Architect", "The Skeptic", "The Speedster")
    pub behavioral_bias: BehavioralBias,
    pub voice_tone: String, // Descriptive string (e.g., "Socratic and inquisitive", "Direct and brevity-focused")
    pub created_at: String,
    pub updated_at: String,
}

/// Behavioral bias weights (0.0 - 1.0) that influence decision-making
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehavioralBias {
    /// Higher values make the agent more cautious (more likely to object to risky proposals)
    pub cautiousness: f64,
    /// Higher values make the agent favor innovative/novel approaches
    pub innovation: f64,
    /// Higher values make the agent focus on detailed analysis and edge cases
    pub detail_orientation: f64,
}

impl Default for BehavioralBias {
    fn default() -> Self {
        Self {
            cautiousness: 0.5,
            innovation: 0.5,
            detail_orientation: 0.5,
        }
    }
}

/// Helper to convert string to Qdrant Value
fn qdrant_string_value(s: String) -> Value {
    Value {
        kind: Some(qdrant_client::qdrant::value::Kind::StringValue(s)),
    }
}

/// Ensure the agent_identities collection exists in Qdrant
pub async fn ensure_persona_collection(
    qdrant_client: Arc<Qdrant>,
    embedding_dim: usize,
) -> Result<(), String> {
    let collection_name = "agent_identities";

    // Check if collection exists
    let collections = qdrant_client
        .list_collections(qdrant_client::qdrant::ListCollectionsRequest {})
        .await
        .map_err(|e| format!("Failed to list Qdrant collections: {}", e))?;

    let collection_exists = collections
        .collections
        .iter()
        .any(|c| c.name == collection_name);

    if !collection_exists {
        info!(
            collection = %collection_name,
            embedding_dim = embedding_dim,
            "Creating agent_identities Qdrant collection"
        );

        let hnsw_config = HnswConfigDiff {
            m: Some(16),
            ef_construct: Some(100),
            full_scan_threshold: None,
            max_indexing_threads: None,
            on_disk: None,
            payload_m: None,
        };

        let create_collection = CreateCollection {
            collection_name: collection_name.to_string(),
            vectors_config: Some(VectorsConfig {
                config: Some(Config::Params(VectorParams {
                    size: embedding_dim as u64,
                    distance: Distance::Cosine as i32,
                    hnsw_config: Some(hnsw_config),
                    ..Default::default()
                })),
            }),
            ..Default::default()
        };

        qdrant_client
            .create_collection(create_collection)
            .await
            .map_err(|e| format!("Failed to create agent_identities collection: {}", e))?;

        info!(collection = %collection_name, "agent_identities collection created");
    }

    Ok(())
}

/// Save or update an agent persona in Qdrant
pub async fn save_persona(
    qdrant_client: Arc<Qdrant>,
    persona: AgentPersona,
    embedding_dim: usize,
) -> Result<(), String> {
    // Ensure collection exists
    ensure_persona_collection(qdrant_client.clone(), embedding_dim).await?;
    
    let collection_name = "agent_identities";

    // Create embedding from persona name and voice tone
    let embedding_text = format!("{} {}", persona.name, persona.voice_tone);
    let embedding = generate_dense_vector(&embedding_text, embedding_dim).await;

    let point_id = Uuid::parse_str(&persona.agent_id)
        .or_else(|_| Ok(Uuid::new_v4()))
        .map_err(|e: uuid::Error| format!("Failed to parse agent_id as UUID: {}", e))?;

    let mut payload: HashMap<String, Value> = HashMap::new();
    payload.insert("agent_id".to_string(), qdrant_string_value(persona.agent_id.clone()));
    payload.insert("name".to_string(), qdrant_string_value(persona.name.clone()));
    payload.insert("voice_tone".to_string(), qdrant_string_value(persona.voice_tone.clone()));
    payload.insert("cautiousness".to_string(), qdrant_string_value(persona.behavioral_bias.cautiousness.to_string()));
    payload.insert("innovation".to_string(), qdrant_string_value(persona.behavioral_bias.innovation.to_string()));
    payload.insert("detail_orientation".to_string(), qdrant_string_value(persona.behavioral_bias.detail_orientation.to_string()));
    payload.insert("created_at".to_string(), qdrant_string_value(persona.created_at.clone()));
    payload.insert("updated_at".to_string(), qdrant_string_value(persona.updated_at.clone()));

    let point = PointStruct {
        id: Some(qdrant_client::qdrant::PointId {
            point_id_options: Some(qdrant_client::qdrant::point_id::PointIdOptions::Uuid(point_id.to_string())),
        }),
        vectors: Some(qdrant_client::qdrant::Vectors {
            vectors_options: Some(qdrant_client::qdrant::vectors::VectorsOptions::Vector(
                qdrant_client::qdrant::Vector { data: embedding },
            )),
        }),
        payload,
    };

    qdrant_client
        .upsert_points(UpsertPoints {
            collection_name: collection_name.to_string(),
            points: vec![point],
            ..Default::default()
        })
        .await
        .map_err(|e| format!("Failed to save persona to Qdrant: {}", e))?;

    info!(
        agent_id = %persona.agent_id,
        persona_name = %persona.name,
        "Persona saved to agent_identities collection"
    );

    Ok(())
}

/// Retrieve an agent persona by agent_id
pub async fn get_persona(
    qdrant_client: Arc<Qdrant>,
    agent_id: &str,
) -> Result<Option<AgentPersona>, String> {
    // Ensure collection exists (idempotent)
    ensure_persona_collection(qdrant_client.clone(), 384).await?;
    
    let collection_name = "agent_identities";

    // Search by agent_id in payload
    let scroll_result = qdrant_client
        .scroll_points(qdrant_client::qdrant::ScrollPoints {
            collection_name: collection_name.to_string(),
            filter: Some(qdrant_client::qdrant::Filter {
                must: vec![qdrant_client::qdrant::Condition {
                    condition_one_of: Some(qdrant_client::qdrant::condition::ConditionOneOf::Field(
                        qdrant_client::qdrant::FieldCondition {
                            key: "agent_id".to_string(),
                            r#match: Some(qdrant_client::qdrant::Match {
                                match_value: Some(qdrant_client::qdrant::r#match::MatchValue::Value(
                                    qdrant_string_value(agent_id.to_string()),
                                )),
                            }),
                            ..Default::default()
                        },
                    )),
                }],
                ..Default::default()
            }),
            limit: Some(1),
            ..Default::default()
        })
        .await
        .map_err(|e| format!("Failed to scroll Qdrant points: {}", e))?;

    if scroll_result.result.is_empty() {
        return Ok(None);
    }

    let point = &scroll_result.result[0];
    let payload = &point.payload;

    let agent_id = payload
        .get("agent_id")
        .and_then(|v| {
            if let Some(qdrant_client::qdrant::value::Kind::StringValue(s)) = &v.kind {
                Some(s.clone())
            } else {
                None
            }
        })
        .ok_or_else(|| "Missing agent_id in payload".to_string())?;

    let name = payload
        .get("name")
        .and_then(|v| {
            if let Some(qdrant_client::qdrant::value::Kind::StringValue(s)) = &v.kind {
                Some(s.clone())
            } else {
                None
            }
        })
        .ok_or_else(|| "Missing name in payload".to_string())?;

    let voice_tone = payload
        .get("voice_tone")
        .and_then(|v| {
            if let Some(qdrant_client::qdrant::value::Kind::StringValue(s)) = &v.kind {
                Some(s.clone())
            } else {
                None
            }
        })
        .ok_or_else(|| "Missing voice_tone in payload".to_string())?;

    let cautiousness = payload
        .get("cautiousness")
        .and_then(|v| {
            if let Some(qdrant_client::qdrant::value::Kind::StringValue(s)) = &v.kind {
                s.parse::<f64>().ok()
            } else {
                None
            }
        })
        .unwrap_or(0.5);

    let innovation = payload
        .get("innovation")
        .and_then(|v| {
            if let Some(qdrant_client::qdrant::value::Kind::StringValue(s)) = &v.kind {
                s.parse::<f64>().ok()
            } else {
                None
            }
        })
        .unwrap_or(0.5);

    let detail_orientation = payload
        .get("detail_orientation")
        .and_then(|v| {
            if let Some(qdrant_client::qdrant::value::Kind::StringValue(s)) = &v.kind {
                s.parse::<f64>().ok()
            } else {
                None
            }
        })
        .unwrap_or(0.5);

    let created_at = payload
        .get("created_at")
        .and_then(|v| {
            if let Some(qdrant_client::qdrant::value::Kind::StringValue(s)) = &v.kind {
                Some(s.clone())
            } else {
                None
            }
        })
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

    let updated_at = payload
        .get("updated_at")
        .and_then(|v| {
            if let Some(qdrant_client::qdrant::value::Kind::StringValue(s)) = &v.kind {
                Some(s.clone())
            } else {
                None
            }
        })
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

    Ok(Some(AgentPersona {
        agent_id,
        name,
        behavioral_bias: BehavioralBias {
            cautiousness,
            innovation,
            detail_orientation,
        },
        voice_tone,
        created_at,
        updated_at,
    }))
}

/// Get all personas
pub async fn get_all_personas(
    qdrant_client: Arc<Qdrant>,
) -> Result<Vec<AgentPersona>, String> {
    // Ensure collection exists (idempotent)
    ensure_persona_collection(qdrant_client.clone(), 384).await?;
    
    let collection_name = "agent_identities";

    let scroll_result = qdrant_client
        .scroll_points(qdrant_client::qdrant::ScrollPoints {
            collection_name: collection_name.to_string(),
            limit: Some(1000), // Reasonable limit
            ..Default::default()
        })
        .await
        .map_err(|e| format!("Failed to scroll Qdrant points: {}", e))?;

    let mut personas = Vec::new();

    for point in scroll_result.result {
        let payload = &point.payload;

        let agent_id = payload
            .get("agent_id")
            .and_then(|v| {
                if let Some(qdrant_client::qdrant::value::Kind::StringValue(s)) = &v.kind {
                    Some(s.clone())
                } else {
                    None
                }
            })?;

        let name = payload
            .get("name")
            .and_then(|v| {
                if let Some(qdrant_client::qdrant::value::Kind::StringValue(s)) = &v.kind {
                    Some(s.clone())
                } else {
                    None
                }
            })?;

        let voice_tone = payload
            .get("voice_tone")
            .and_then(|v| {
                if let Some(qdrant_client::qdrant::value::Kind::StringValue(s)) = &v.kind {
                    Some(s.clone())
                } else {
                    None
                }
            })?;

        let cautiousness = payload
            .get("cautiousness")
            .and_then(|v| {
                if let Some(qdrant_client::qdrant::value::Kind::StringValue(s)) = &v.kind {
                    s.parse::<f64>().ok()
                } else {
                    None
                }
            })
            .unwrap_or(0.5);

        let innovation = payload
            .get("innovation")
            .and_then(|v| {
                if let Some(qdrant_client::qdrant::value::Kind::StringValue(s)) = &v.kind {
                    s.parse::<f64>().ok()
                } else {
                    None
                }
            })
            .unwrap_or(0.5);

        let detail_orientation = payload
            .get("detail_orientation")
            .and_then(|v| {
                if let Some(qdrant_client::qdrant::value::Kind::StringValue(s)) = &v.kind {
                    s.parse::<f64>().ok()
                } else {
                    None
                }
            })
            .unwrap_or(0.5);

        let created_at = payload
            .get("created_at")
            .and_then(|v| {
                if let Some(qdrant_client::qdrant::value::Kind::StringValue(s)) = &v.kind {
                    Some(s.clone())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

        let updated_at = payload
            .get("updated_at")
            .and_then(|v| {
                if let Some(qdrant_client::qdrant::value::Kind::StringValue(s)) = &v.kind {
                    Some(s.clone())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

        personas.push(AgentPersona {
            agent_id,
            name,
            behavioral_bias: BehavioralBias {
                cautiousness,
                innovation,
                detail_orientation,
            },
            voice_tone,
            created_at,
            updated_at,
        });
    }

    Ok(personas)
}

/// Generate dense vector embedding using fastembed (same as phoenix_routes)
async fn generate_dense_vector(text: &str, dim: usize) -> Vec<f32> {
    use std::sync::OnceLock;
    use std::sync::Arc;
    use std::sync::Mutex;
    
    // Try to use the same embedding model as phoenix_routes
    // For now, use a simple hash-based approach that matches the dimension
    // In production, this should share the embedding model instance
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    let hash = hasher.finish();
    
    let mut vec = vec![0.0f32; dim];
    for i in 0..dim {
        let seed = hash.wrapping_mul(i as u64 + 1);
        vec[i] = ((seed % 2000) as f32 - 1000.0) / 1000.0;
    }
    
    // Normalize
    let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for v in &mut vec {
            *v /= norm;
        }
    }
    
    vec
}

/// Apply persona bias to peer review decision
/// Returns a modified decision probability based on persona characteristics
pub fn apply_persona_bias_to_review(
    persona: &AgentPersona,
    tool_reliability: f64,
    playbook_dependencies: Option<usize>,
) -> f64 {
    let mut object_probability = 0.0;

    // Base object probability based on reliability
    if tool_reliability < 0.9 {
        object_probability = 1.0 - tool_reliability;
    }

    // Apply "The Skeptic" bias: 30% more likely to object if reliability < 90%
    if persona.name.to_lowercase().contains("skeptic") {
        if tool_reliability < 0.9 {
            object_probability *= 1.3;
        }
    }

    // Apply cautiousness bias
    object_probability += (1.0 - tool_reliability) * persona.behavioral_bias.cautiousness * 0.2;

    // Apply "The Architect" bias: prioritize playbooks with fewer dependencies
    if persona.name.to_lowercase().contains("architect") {
        if let Some(deps) = playbook_dependencies {
            if deps > 5 {
                object_probability += 0.15; // More likely to object to high-dependency tools
            } else if deps < 2 {
                object_probability -= 0.1; // Less likely to object to low-dependency tools
            }
        }
    }

    // Apply detail orientation: more likely to object if reliability is borderline
    if persona.behavioral_bias.detail_orientation > 0.7 {
        if tool_reliability > 0.85 && tool_reliability < 0.95 {
            object_probability += 0.1; // Scrutinize borderline cases more
        }
    }

    // Clamp to [0.0, 1.0]
    object_probability.min(1.0).max(0.0)
}
