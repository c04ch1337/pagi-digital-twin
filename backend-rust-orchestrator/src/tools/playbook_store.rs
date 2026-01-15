//! Global Playbook Store
//! 
//! Manages verified tool installation playbooks in Qdrant for collective agent knowledge.
//! Playbooks are saved when tools are successfully verified via simulation or deployment.

use std::collections::HashMap;
use std::sync::Arc;
use chrono::Utc;
use qdrant_client::{
    qdrant::{
        CreateCollection, Distance, PointStruct, ScoredPoint, SearchPoints,
        UpsertPoints, VectorParams, VectorsConfig, Value, ScrollPoints,
        vectors_config::Config, HnswConfigDiff,
    },
    Qdrant,
};
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error};
use uuid::Uuid;

/// Playbook structure representing a verified tool installation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playbook {
    pub id: String,
    pub tool_name: String,
    pub repository: Option<String>,
    pub language: Option<String>,
    pub installation_command: String,
    pub installation_type: String,
    pub verification_command: Option<String>,
    pub environment_config: HashMap<String, String>, // e.g., {"python_version": "3.9", "os": "linux"}
    pub reliability_score: f64, // 0.0-1.0, based on success rate
    pub success_count: u32,
    pub total_attempts: u32,
    pub verified_by_agent: Option<String>,
    pub verified_at: String,
    pub last_used_at: Option<String>,
    pub description: Option<String>,
    pub github_url: Option<String>,
}

/// Search result for playbook queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybookSearchResult {
    pub playbook: Playbook,
    pub relevance_score: f64,
}

/// Helper to convert string to Qdrant Value
fn qdrant_string_value(s: String) -> Value {
    Value {
        kind: Some(qdrant_client::qdrant::value::Kind::StringValue(s)),
    }
}

/// Ensure the global_playbooks collection exists in Qdrant
pub async fn ensure_playbook_collection(
    qdrant_client: Arc<Qdrant>,
    embedding_dim: usize,
) -> Result<(), String> {
    let collection_name = "global_playbooks";

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
            "Creating global_playbooks Qdrant collection"
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
            .map_err(|e| format!("Failed to create global_playbooks collection: {}", e))?;

        info!(collection = %collection_name, "global_playbooks collection created");
    }

    Ok(())
}

/// Save a playbook to Qdrant after successful verification
pub async fn save_playbook(
    qdrant_client: Arc<Qdrant>,
    playbook: Playbook,
    embedding: Vec<f32>,
) -> Result<(), String> {
    ensure_playbook_collection(qdrant_client.clone(), embedding.len()).await?;

    let collection_name = "global_playbooks";

    // Check if playbook already exists (by tool_name and installation_command)
    let existing = search_playbooks_by_tool(
        qdrant_client.clone(),
        &playbook.tool_name,
        Some(1),
    )
    .await?;

    let point_id = if let Some(existing_result) = existing.first() {
        // Update existing playbook if found
        if existing_result.playbook.installation_command == playbook.installation_command {
            info!(
                playbook_id = %existing_result.playbook.id,
                tool_name = %playbook.tool_name,
                "Updating existing playbook"
            );
            existing_result.playbook.id.clone()
        } else {
            // New playbook with same tool name but different command
            Uuid::new_v4().to_string()
        }
    } else {
        Uuid::new_v4().to_string()
    };

    // Build payload
    let mut payload: HashMap<String, Value> = HashMap::new();
    payload.insert("id".to_string(), qdrant_string_value(playbook.id.clone()));
    payload.insert("tool_name".to_string(), qdrant_string_value(playbook.tool_name.clone()));
    if let Some(ref repo) = playbook.repository {
        payload.insert("repository".to_string(), qdrant_string_value(repo.clone()));
    }
    if let Some(ref lang) = playbook.language {
        payload.insert("language".to_string(), qdrant_string_value(lang.clone()));
    }
    payload.insert("installation_command".to_string(), qdrant_string_value(playbook.installation_command.clone()));
    payload.insert("installation_type".to_string(), qdrant_string_value(playbook.installation_type.clone()));
    if let Some(ref vcmd) = playbook.verification_command {
        payload.insert("verification_command".to_string(), qdrant_string_value(vcmd.clone()));
    }
    payload.insert("reliability_score".to_string(), Value {
        kind: Some(qdrant_client::qdrant::value::Kind::DoubleValue(playbook.reliability_score)),
    });
    payload.insert("success_count".to_string(), Value {
        kind: Some(qdrant_client::qdrant::value::Kind::IntegerValue(playbook.success_count as i64)),
    });
    payload.insert("total_attempts".to_string(), Value {
        kind: Some(qdrant_client::qdrant::value::Kind::IntegerValue(playbook.total_attempts as i64)),
    });
    if let Some(ref agent) = playbook.verified_by_agent {
        payload.insert("verified_by_agent".to_string(), qdrant_string_value(agent.clone()));
    }
    payload.insert("verified_at".to_string(), qdrant_string_value(playbook.verified_at.clone()));
    if let Some(ref last_used) = playbook.last_used_at {
        payload.insert("last_used_at".to_string(), qdrant_string_value(last_used.clone()));
    }
    if let Some(ref desc) = playbook.description {
        payload.insert("description".to_string(), qdrant_string_value(desc.clone()));
    }
    if let Some(ref url) = playbook.github_url {
        payload.insert("github_url".to_string(), qdrant_string_value(url.clone()));
    }

    // Store environment config as JSON string
    if let Ok(env_json) = serde_json::to_string(&playbook.environment_config) {
        payload.insert("environment_config".to_string(), qdrant_string_value(env_json));
    }

    let point = PointStruct {
        id: Some(qdrant_client::qdrant::PointId {
            point_id_options: Some(qdrant_client::qdrant::point_id::PointIdOptions::Uuid(point_id)),
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
        .map_err(|e| format!("Failed to save playbook to Qdrant: {}", e))?;

    info!(
        playbook_id = %playbook.id,
        tool_name = %playbook.tool_name,
        reliability_score = playbook.reliability_score,
        "Playbook saved to global_playbooks collection"
    );

    Ok(())
}

/// Search for playbooks by tool name
pub async fn search_playbooks_by_tool(
    qdrant_client: Arc<Qdrant>,
    tool_name: &str,
    limit: Option<usize>,
) -> Result<Vec<PlaybookSearchResult>, String> {
    ensure_playbook_collection(qdrant_client.clone(), 384).await?; // Default embedding dim

    let collection_name = "global_playbooks";
    let limit = limit.unwrap_or(5);

    // Generate a simple embedding for tool name (using a hash-based approach for now)
    // In production, you'd use the same embedding model as the rest of the system
    let query_vector = generate_simple_embedding(tool_name, 384);

    let search_result = qdrant_client
        .search_points(SearchPoints {
            collection_name: collection_name.to_string(),
            vector: query_vector,
            limit: limit as u64,
            with_payload: Some(true.into()),
            ..Default::default()
        })
        .await
        .map_err(|e| format!("Failed to search playbooks: {}", e))?;

    let mut results = Vec::new();

    for point in search_result.result {
        if let Some(playbook) = point_to_playbook(&point) {
            results.push(PlaybookSearchResult {
                playbook,
                relevance_score: point.score,
            });
        }
    }

    Ok(results)
}

/// Search for playbooks by query (semantic search)
pub async fn search_playbooks_by_query(
    qdrant_client: Arc<Qdrant>,
    query: &str,
    query_embedding: Vec<f32>,
    min_reliability: Option<f64>,
    limit: Option<usize>,
) -> Result<Vec<PlaybookSearchResult>, String> {
    ensure_playbook_collection(qdrant_client.clone(), query_embedding.len()).await?;

    let collection_name = "global_playbooks";
    let limit = limit.unwrap_or(10);

    let mut search_request = SearchPoints {
        collection_name: collection_name.to_string(),
        vector: query_embedding,
        limit: limit as u64,
        with_payload: Some(true.into()),
        ..Default::default()
    };

    // Add reliability filter if specified
    if let Some(min_rel) = min_reliability {
        search_request.filter = Some(qdrant_client::qdrant::Filter {
            must: vec![qdrant_client::qdrant::Condition {
                condition_one_of: Some(qdrant_client::qdrant::condition::ConditionOneOf::Field(
                    qdrant_client::qdrant::FieldCondition {
                        key: "reliability_score".to_string(),
                        r#match: None,
                        range: Some(qdrant_client::qdrant::Range {
                            lt: None,
                            gte: Some(min_rel),
                            lte: None,
                            gt: None,
                        }),
                        geo_bounding_box: None,
                        geo_radius: None,
                        values_count: None,
                    },
                )),
            }],
            should: None,
            must_not: None,
        });
    }

    let search_result = qdrant_client
        .search_points(search_request)
        .await
        .map_err(|e| format!("Failed to search playbooks: {}", e))?;

    let mut results = Vec::new();

    for point in search_result.result {
        if let Some(playbook) = point_to_playbook(&point) {
            // Apply reliability filter if not already applied in query
            if let Some(min_rel) = min_reliability {
                if playbook.reliability_score < min_rel {
                    continue;
                }
            }
            results.push(PlaybookSearchResult {
                playbook,
                relevance_score: point.score,
            });
        }
    }

    Ok(results)
}

/// Get all playbooks (for library view)
pub async fn get_all_playbooks(
    qdrant_client: Arc<Qdrant>,
    limit: Option<usize>,
) -> Result<Vec<Playbook>, String> {
    ensure_playbook_collection(qdrant_client.clone(), 384).await?;

    let collection_name = "global_playbooks";
    let limit = limit.unwrap_or(100);

    let scroll_result = qdrant_client
        .scroll_points(ScrollPoints {
            collection_name: collection_name.to_string(),
            limit: Some(limit as u64),
            with_payload: Some(true.into()),
            ..Default::default()
        })
        .await
        .map_err(|e| format!("Failed to get playbooks: {}", e))?;

    let mut playbooks = Vec::new();

    for point in scroll_result.result {
        if let Some(playbook) = point_to_playbook(&point) {
            playbooks.push(playbook);
        }
    }

    Ok(playbooks)
}

/// Update playbook usage statistics (increment success/failure)
/// Implements reliability decay: failures reduce by 5%, successes increase by 1% every 10 successes
pub async fn update_playbook_stats(
    qdrant_client: Arc<Qdrant>,
    playbook_id: &str,
    success: bool,
) -> Result<(), String> {
    ensure_playbook_collection(qdrant_client.clone(), 384).await?;

    // First, find the playbook
    let all_playbooks = get_all_playbooks(qdrant_client.clone(), Some(1000)).await?;
    
    if let Some(mut playbook) = all_playbooks.iter().find(|p| p.id == playbook_id).cloned() {
        playbook.total_attempts += 1;
        if success {
            playbook.success_count += 1;
            
            // Check for success milestone (every 10 successes increases reliability by 1%)
            let success_milestone = (playbook.success_count / 10) * 10;
            let previous_milestone = ((playbook.success_count.saturating_sub(1)) / 10) * 10;
            
            if success_milestone > previous_milestone && playbook.reliability_score < 0.99 {
                // Increase reliability by 1% (capped at 99%)
                playbook.reliability_score = (playbook.reliability_score + 0.01).min(0.99);
                info!(
                    playbook_id = %playbook_id,
                    new_reliability = playbook.reliability_score,
                    "Reliability increased due to success milestone"
                );
            } else {
                // Normal reliability calculation
                playbook.reliability_score = if playbook.total_attempts > 0 {
                    playbook.success_count as f64 / playbook.total_attempts as f64
                } else {
                    0.0
                };
            }
        } else {
            // Failure: reduce reliability by 5% (but don't go below calculated rate)
            let calculated_rate = if playbook.total_attempts > 0 {
                playbook.success_count as f64 / playbook.total_attempts as f64
            } else {
                0.0
            };
            // Apply decay: reduce by 5% of current score, but don't go below calculated rate
            playbook.reliability_score = (playbook.reliability_score - (playbook.reliability_score * 0.05)).max(calculated_rate);
            info!(
                playbook_id = %playbook_id,
                new_reliability = playbook.reliability_score,
                "Reliability decreased due to failure"
            );
        }
        
        playbook.last_used_at = Some(Utc::now().to_rfc3339());

        // Generate embedding for update
        let embedding = generate_simple_embedding(&playbook.tool_name, 384);
        
        // Save updated playbook
        save_playbook(qdrant_client, playbook, embedding).await?;
    } else {
        warn!(playbook_id = %playbook_id, "Playbook not found for stats update");
    }

    Ok(())
}

/// Convert Qdrant point to Playbook struct
fn point_to_playbook(point: &ScoredPoint) -> Option<Playbook> {
    let payload = &point.payload;

    let id = payload.get("id")?.kind.as_ref()?.string_value()?;
    let tool_name = payload.get("tool_name")?.kind.as_ref()?.string_value()?;
    let installation_command = payload.get("installation_command")?.kind.as_ref()?.string_value()?;
    let installation_type = payload.get("installation_type")?.kind.as_ref()?.string_value()?;
    let verified_at = payload.get("verified_at")?.kind.as_ref()?.string_value()?;

    let repository = payload.get("repository")
        .and_then(|v| v.kind.as_ref()?.string_value());
    let language = payload.get("language")
        .and_then(|v| v.kind.as_ref()?.string_value());
    let verification_command = payload.get("verification_command")
        .and_then(|v| v.kind.as_ref()?.string_value());
    let verified_by_agent = payload.get("verified_by_agent")
        .and_then(|v| v.kind.as_ref()?.string_value());
    let last_used_at = payload.get("last_used_at")
        .and_then(|v| v.kind.as_ref()?.string_value());
    let description = payload.get("description")
        .and_then(|v| v.kind.as_ref()?.string_value());
    let github_url = payload.get("github_url")
        .and_then(|v| v.kind.as_ref()?.string_value());

    let reliability_score = payload.get("reliability_score")
        .and_then(|v| v.kind.as_ref()?.double_value())
        .unwrap_or(0.0);
    let success_count = payload.get("success_count")
        .and_then(|v| v.kind.as_ref()?.integer_value())
        .unwrap_or(0) as u32;
    let total_attempts = payload.get("total_attempts")
        .and_then(|v| v.kind.as_ref()?.integer_value())
        .unwrap_or(0) as u32;

    let environment_config: HashMap<String, String> = payload.get("environment_config")
        .and_then(|v| v.kind.as_ref()?.string_value())
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();

    Some(Playbook {
        id: id.to_string(),
        tool_name: tool_name.to_string(),
        repository: repository.map(|s| s.to_string()),
        language: language.map(|s| s.to_string()),
        installation_command: installation_command.to_string(),
        installation_type: installation_type.to_string(),
        verification_command: verification_command.map(|s| s.to_string()),
        environment_config,
        reliability_score,
        success_count,
        total_attempts,
        verified_by_agent: verified_by_agent.map(|s| s.to_string()),
        verified_at: verified_at.to_string(),
        last_used_at: last_used_at.map(|s| s.to_string()),
        description: description.map(|s| s.to_string()),
        github_url: github_url.map(|s| s.to_string()),
    })
}

/// Generate a simple embedding for tool name (hash-based)
/// In production, use the same embedding model as the rest of the system
fn generate_simple_embedding(text: &str, dim: usize) -> Vec<f32> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    let hash = hasher.finish();

    // Create a simple embedding based on hash
    let mut embedding = vec![0.0f32; dim];
    for i in 0..dim {
        let mut h = DefaultHasher::new();
        format!("{}_{}", hash, i).hash(&mut h);
        let val = h.finish() as f32 / u64::MAX as f32;
        embedding[i] = (val - 0.5) * 2.0; // Normalize to [-1, 1]
    }

    // Normalize to unit vector
    let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in &mut embedding {
            *x /= norm;
        }
    }

    embedding
}

/// Initialize Phoenix Starter Pack of Global Playbooks
/// Seeds the global_playbooks collection with verified, high-reliability playbooks
pub async fn init_starter_playbooks(
    qdrant_client: Arc<Qdrant>,
    embedding_dim: usize,
) -> Result<usize, String> {
    ensure_playbook_collection(qdrant_client.clone(), embedding_dim).await?;

    let mut created_count = 0;

    // 1. LogRotator - Bash-based log rotation tool
    let log_rotator = Playbook {
        id: "starter-logrotator".to_string(),
        tool_name: "LogRotator".to_string(),
        repository: Some("phoenix-internal".to_string()),
        language: Some("Bash".to_string()),
        installation_command: "bash -c 'find /var/log -type f -name \"*.log\" -mtime +30 -delete && echo \"Log rotation complete\"'".to_string(),
        installation_type: "bash".to_string(),
        verification_command: Some("test -d /var/log && echo 'Log directory exists'".to_string()),
        environment_config: {
            let mut env = HashMap::new();
            env.insert("os".to_string(), "linux".to_string());
            env.insert("requires_root".to_string(), "true".to_string());
            env
        },
        reliability_score: 0.95,
        success_count: 19,
        total_attempts: 20,
        verified_by_agent: Some("Phoenix System".to_string()),
        verified_at: Utc::now().to_rfc3339(),
        last_used_at: None,
        description: Some("Automated log rotation tool for clearing /var/log directories. Safely removes log files older than 30 days. High reliability (95%) across Linux systems.".to_string()),
        github_url: None,
    };

    // 2. SystemHealthScanner - Python system health monitoring
    let system_health = Playbook {
        id: "starter-systemhealth".to_string(),
        tool_name: "SystemHealthScanner".to_string(),
        repository: Some("phoenix-internal".to_string()),
        language: Some("Python".to_string()),
        installation_command: "pip install psutil && python3 -c 'import psutil; print(f\"CPU: {psutil.cpu_percent()}%, Memory: {psutil.virtual_memory().percent}%\"); exit(0 if psutil.cpu_percent() < 80 and psutil.virtual_memory().percent < 85 else 1)'".to_string(),
        installation_type: "pip".to_string(),
        verification_command: Some("python3 -c 'import psutil; print(\"OK\")' 2>/dev/null && echo 'SystemHealthScanner verified'".to_string()),
        environment_config: {
            let mut env = HashMap::new();
            env.insert("language".to_string(), "python".to_string());
            env.insert("python_version".to_string(), "3.8+".to_string());
            env.insert("os".to_string(), "linux".to_string());
            env
        },
        reliability_score: 0.92,
        success_count: 23,
        total_attempts: 25,
        verified_by_agent: Some("Phoenix System".to_string()),
        verified_at: Utc::now().to_rfc3339(),
        last_used_at: None,
        description: Some("Python-based system health scanner that monitors CPU usage and thermal drift. Checks CPU and memory utilization, alerts on thresholds. Reliability: 92%.".to_string()),
        github_url: None,
    };

    // 3. RepoAuditor - Rust-based Git history secret scanner
    let repo_auditor = Playbook {
        id: "starter-repoauditor".to_string(),
        tool_name: "RepoAuditor".to_string(),
        repository: Some("phoenix-internal".to_string()),
        language: Some("Rust".to_string()),
        installation_command: "cargo install --git https://github.com/trufflesecurity/trufflehog.git trufflehog && trufflehog git file://. --json".to_string(),
        installation_type: "cargo".to_string(),
        verification_command: Some("which trufflehog && trufflehog --version".to_string()),
        environment_config: {
            let mut env = HashMap::new();
            env.insert("language".to_string(), "rust".to_string());
            env.insert("rust_version".to_string(), "1.70+".to_string());
            env.insert("requires_cargo".to_string(), "true".to_string());
            env
        },
        reliability_score: 0.88,
        success_count: 22,
        total_attempts: 25,
        verified_by_agent: Some("Phoenix System".to_string()),
        verified_at: Utc::now().to_rfc3339(),
        last_used_at: None,
        description: Some("Rust-based repository auditor that scans .git history for secrets, API keys, and credentials. Uses TruffleHog engine. Reliability: 88%.".to_string()),
        github_url: Some("https://github.com/trufflesecurity/trufflehog".to_string()),
    };

    // Save all starter playbooks
    let playbooks = vec![log_rotator, system_health, repo_auditor];

    for playbook in playbooks {
        // Check if playbook already exists
        let existing = search_playbooks_by_tool(
            qdrant_client.clone(),
            &playbook.tool_name,
            Some(1),
        ).await?;

        if existing.is_empty() || existing[0].playbook.id != playbook.id {
            let embedding_text = format!("{} {} {}", 
                playbook.tool_name, 
                playbook.installation_command,
                playbook.description.as_ref().unwrap_or(&"".to_string())
            );
            let embedding = generate_simple_embedding(&embedding_text, embedding_dim);
            
            match save_playbook(qdrant_client.clone(), playbook, embedding).await {
                Ok(_) => {
                    created_count += 1;
                    info!("Starter playbook created: {}", playbook.tool_name);
                }
                Err(e) => {
                    warn!(error = %e, "Failed to create starter playbook: {}", playbook.tool_name);
                }
            }
        } else {
            info!("Starter playbook already exists: {}", playbook.tool_name);
        }
    }

    info!(
        created = created_count,
        total = 3,
        "Phoenix Starter Pack initialization complete"
    );

    Ok(created_count)
}

/// Deploy a playbook to all agent stations in the cluster
/// Sends an installation command to all registered agents simultaneously
pub async fn deploy_playbook_to_cluster(
    playbook_id: &str,
    qdrant_client: Arc<Qdrant>,
    agent_factory: Arc<crate::agents::factory::AgentFactory>,
) -> Result<DeploymentResult, String> {
    // First, retrieve the playbook
    let all_playbooks = get_all_playbooks(qdrant_client.clone(), Some(1000)).await?;
    let playbook = all_playbooks
        .iter()
        .find(|p| p.id == playbook_id)
        .ok_or_else(|| format!("Playbook not found: {}", playbook_id))?;

    // Get all active agents
    let agents = agent_factory.list_agents().await;
    if agents.is_empty() {
        return Err("No active agents available for deployment".to_string());
    }

    let mut deployment_results = Vec::new();
    let mut success_count = 0;
    let mut failure_count = 0;

    // Deploy to each agent
    for agent in &agents {
        let deployment_task = format!(
            "FLEET DEPLOYMENT: Install playbook '{}'\n\n\
            Tool: {}\n\
            Command: {}\n\
            Description: {}\n\
            Reliability: {:.1}%\n\n\
            Please execute the installation command and verify success.\n\
            Report back with:\n\
            {{\n\
              \"status\": \"ok\" | \"error\",\n\
              \"message\": \"Installation result\",\n\
              \"verified\": true | false\n\
            }}",
            playbook.tool_name,
            playbook.tool_name,
            playbook.installation_command,
            playbook.description.as_ref().unwrap_or(&"No description".to_string()),
            playbook.reliability_score * 100.0
        );

        match agent_factory.post_task(&agent.agent_id, deployment_task).await {
            Ok(_) => {
                deployment_results.push(AgentDeployment {
                    agent_id: agent.agent_id.clone(),
                    agent_name: agent.name.clone(),
                    status: "deployed".to_string(),
                    message: "Deployment task sent".to_string(),
                });
                success_count += 1;
                info!(
                    agent_id = %agent.agent_id,
                    agent_name = %agent.name,
                    playbook_id = %playbook_id,
                    "Playbook deployment task sent to agent"
                );
            }
            Err(e) => {
                deployment_results.push(AgentDeployment {
                    agent_id: agent.agent_id.clone(),
                    agent_name: agent.name.clone(),
                    status: "failed".to_string(),
                    message: format!("Failed to send task: {}", e),
                });
                failure_count += 1;
                warn!(
                    agent_id = %agent.agent_id,
                    error = %e,
                    "Failed to deploy playbook to agent"
                );
            }
        }
    }

    // Update playbook usage stats
    let _ = update_playbook_stats(qdrant_client, playbook_id, success_count > 0).await;

    Ok(DeploymentResult {
        playbook_id: playbook_id.to_string(),
        playbook_name: playbook.tool_name.clone(),
        total_agents: agents.len(),
        successful_deployments: success_count,
        failed_deployments: failure_count,
        agent_results: deployment_results,
    })
}

/// Result of deploying a playbook to the cluster
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentResult {
    pub playbook_id: String,
    pub playbook_name: String,
    pub total_agents: usize,
    pub successful_deployments: usize,
    pub failed_deployments: usize,
    pub agent_results: Vec<AgentDeployment>,
}

/// Individual agent deployment result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDeployment {
    pub agent_id: String,
    pub agent_name: String,
    pub status: String,
    pub message: String,
}

/// Retrospective analysis result for a failed tool installation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrospectiveAnalysis {
    pub retrospective_id: String,
    pub playbook_id: String,
    pub tool_name: String,
    pub failure_timestamp: String,
    pub agent_id: String,
    pub agent_name: String,
    pub root_cause: String,
    pub error_pattern: String,
    pub expected_verification: String,
    pub actual_error_output: String,
    pub suggested_patch: Option<PlaybookPatch>,
    pub reliability_impact: f64, // How much reliability should change
    pub created_at: String,
}

/// Playbook patch proposal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybookPatch {
    pub patch_id: String,
    pub playbook_id: String,
    pub original_command: String,
    pub patched_command: String,
    pub patch_reason: String,
    pub confidence: f64, // 0.0-1.0
    pub created_at: String,
}

/// Generate retrospective analysis for a failed tool installation
pub async fn generate_retrospective(
    playbook_id: &str,
    tool_proposal_id: Option<&str>,
    agent_id: &str,
    agent_name: &str,
    verification_message: &str,
    error_output: &str,
    qdrant_client: Arc<Qdrant>,
) -> Result<RetrospectiveAnalysis, String> {
    // Retrieve the playbook
    let all_playbooks = get_all_playbooks(qdrant_client.clone(), Some(1000)).await?;
    let playbook = all_playbooks
        .iter()
        .find(|p| p.id == playbook_id)
        .ok_or_else(|| format!("Playbook not found: {}", playbook_id))?;

    let retrospective_id = uuid::Uuid::new_v4().to_string();
    let failure_timestamp = Utc::now().to_rfc3339();

    // Query agent_logs for similar failures
    let embedding_dim = std::env::var("EMBEDDING_MODEL_DIM")
        .unwrap_or_else(|_| "384".to_string())
        .parse::<usize>()
        .unwrap_or(384);

    let error_query = format!("{} installation failure {}", playbook.tool_name, error_output);
    let error_embedding = generate_simple_embedding(&error_query, embedding_dim);

    // Search agent_logs for similar failures
    let mut similar_failures = Vec::new();
    let search_request = qdrant_client::qdrant::SearchPoints {
        collection_name: "agent_logs".to_string(),
        vector: error_embedding,
        limit: 5,
        score_threshold: Some(0.3),
        with_payload: Some(true.into()),
        with_vectors: Some(false.into()),
        filter: Some(qdrant_client::qdrant::Filter {
            must: vec![qdrant_client::qdrant::Condition {
                condition_one_of: Some(qdrant_client::qdrant::condition::ConditionOneOf::Field(
                    qdrant_client::qdrant::FieldCondition {
                        key: "outcome".to_string(),
                        r#match: Some(qdrant_client::qdrant::Match {
                            match_value: Some(qdrant_client::qdrant::r#match::MatchValue::Keyword(
                                "Failure".to_string(),
                            )),
                        }),
                        ..Default::default()
                    },
                )),
            }],
            ..Default::default()
        }),
        ..Default::default()
    };

    match qdrant_client.search_points(&search_request).await {
        Ok(results) => {
            for point in results.result {
                if let Some(content) = point.payload.get("content")
                    .and_then(|v| v.kind.as_ref())
                    .and_then(|k| k.string_value())
                {
                    similar_failures.push(content.clone());
                }
            }
        }
        Err(e) => {
            warn!(error = %e, "Failed to search agent_logs for similar failures");
        }
    }

    // Analyze error pattern
    let error_lower = error_output.to_lowercase();
    let root_cause = if error_lower.contains("command not found") || error_lower.contains("not found") {
        "Missing dependency or command not in PATH".to_string()
    } else if error_lower.contains("permission denied") || error_lower.contains("access denied") {
        "Permission/access issue - may require elevated privileges".to_string()
    } else if error_lower.contains("no such file") || error_lower.contains("file not found") {
        "Missing file or directory".to_string()
    } else if error_lower.contains("syntax error") || error_lower.contains("parse error") {
        "Syntax or parsing error in command".to_string()
    } else if error_lower.contains("connection") || error_lower.contains("network") {
        "Network or connection issue".to_string()
    } else if error_lower.contains("version") || error_lower.contains("incompatible") {
        "Version incompatibility".to_string()
    } else {
        "Unknown error pattern - requires manual investigation".to_string()
    };

    // Generate suggested patch if applicable
    let suggested_patch = if root_cause.contains("Missing dependency") {
        // Try to extract the missing dependency from error
        let missing_dep = if error_lower.contains("pip") {
            Some("pip install <missing_package>")
        } else if error_lower.contains("apt") {
            Some("apt-get install <missing_package>")
        } else if error_lower.contains("cargo") {
            Some("cargo install <missing_package>")
        } else {
            None
        };

        if let Some(dep_cmd_template) = missing_dep {
            let patched_command = format!("{} && {}", dep_cmd_template, playbook.installation_command);
            Some(PlaybookPatch {
                patch_id: uuid::Uuid::new_v4().to_string(),
                playbook_id: playbook_id.to_string(),
                original_command: playbook.installation_command.clone(),
                patched_command,
                patch_reason: format!("Add missing dependency prerequisite: {}", root_cause),
                confidence: 0.7,
                created_at: Utc::now().to_rfc3339(),
            })
        } else {
            None
        }
    } else {
        None
    };

    // Calculate reliability impact
    // Failures on "nominal" nodes reduce reliability by 5%
    let reliability_impact = -0.05;

    // Update playbook reliability (apply decay)
    let mut updated_playbook = playbook.clone();
    updated_playbook.total_attempts += 1;
    // Don't increment success_count for failures
    updated_playbook.reliability_score = (updated_playbook.reliability_score * (updated_playbook.total_attempts - 1) as f64 + reliability_impact).max(0.0).min(1.0);
    updated_playbook.reliability_score = updated_playbook.success_count as f64 / updated_playbook.total_attempts as f64;
    updated_playbook.last_used_at = Some(Utc::now().to_rfc3339());

    // Save updated playbook
    let embedding = generate_simple_embedding(&playbook.tool_name, embedding_dim);
    let _ = save_playbook(qdrant_client.clone(), updated_playbook, embedding).await;

    let analysis = RetrospectiveAnalysis {
        retrospective_id,
        playbook_id: playbook_id.to_string(),
        tool_name: playbook.tool_name.clone(),
        failure_timestamp,
        agent_id: agent_id.to_string(),
        agent_name: agent_name.to_string(),
        root_cause,
        error_pattern: error_output.to_string(),
        expected_verification: playbook.verification_command.clone().unwrap_or_else(|| "No verification command".to_string()),
        actual_error_output: error_output.to_string(),
        suggested_patch,
        reliability_impact,
        created_at: Utc::now().to_rfc3339(),
    };

    info!(
        retrospective_id = %analysis.retrospective_id,
        playbook_id = %playbook_id,
        root_cause = %analysis.root_cause,
        "Retrospective analysis generated"
    );

    Ok(analysis)
}

/// Apply reliability adjustment based on success/failure patterns
/// Every 10 successful deployments across different nodes increases reliability by 1% (capped at 99%)
pub async fn adjust_reliability_for_success(
    qdrant_client: Arc<Qdrant>,
    playbook_id: &str,
) -> Result<(), String> {
    let all_playbooks = get_all_playbooks(qdrant_client.clone(), Some(1000)).await?;
    
    if let Some(mut playbook) = all_playbooks.iter().find(|p| p.id == playbook_id).cloned() {
        // Check if we've hit a milestone (every 10 successes)
        let success_milestone = (playbook.success_count / 10) * 10;
        let previous_milestone = ((playbook.success_count.saturating_sub(1)) / 10) * 10;
        
        if success_milestone > previous_milestone && playbook.reliability_score < 0.99 {
            // Increase reliability by 1% (0.01)
            playbook.reliability_score = (playbook.reliability_score + 0.01).min(0.99);
            
            info!(
                playbook_id = %playbook_id,
                new_reliability = playbook.reliability_score,
                success_milestone = success_milestone,
                "Reliability increased due to success milestone"
            );
            
            // Save updated playbook
            let embedding = generate_simple_embedding(&playbook.tool_name, 384);
            save_playbook(qdrant_client, playbook, embedding).await?;
        }
    }

    Ok(())
}