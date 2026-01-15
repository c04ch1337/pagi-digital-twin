//! Audit Report Archiving and Trend Analysis
//!
//! This module provides tools for archiving Phoenix Auditor reports to Qdrant
//! and performing trend analysis across historical audit data.

use chrono::Utc;
use qdrant_client::{
    qdrant::{
        CreateCollection, Distance, PointStruct, ScrollPoints, SearchPoints, Value, VectorParams,
        VectorsConfig, vectors_config::Config, Filter, FieldCondition, Match, HnswConfigDiff,
        Condition,
    },
    Qdrant,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{error, info, warn};

/// Audit report structure matching the Phoenix Auditor output format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditReport {
    pub status: String,
    pub executive_pulse: String,
    pub rising_action: Vec<String>,
    pub climax: String,
    pub resolutions: Vec<String>,
    pub evidence: Vec<String>,
    pub next_steps: Vec<String>,
    #[serde(default)]
    pub tool_discoveries: Vec<serde_json::Value>,
}

/// Historical audit report with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalAuditReport {
    pub id: String,
    pub timestamp: String,
    pub report: AuditReport,
    pub affected_paths: Vec<String>,
    pub severity: String,
    #[serde(default)]
    pub source_node: String,
}

/// Trend analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendAnalysis {
    pub path: String,
    pub change_count: usize,
    pub first_seen: String,
    pub last_seen: String,
    pub severity_escalation: bool,
    pub recurring_climax: bool,
    pub related_reports: Vec<String>,
}

/// Archive an audit report to Qdrant
/// Creates its own Qdrant client connection from environment variables
/// 
/// # Arguments
/// * `report_json` - The audit report JSON string
/// * `source_node` - Optional node ID that generated this audit report (defaults to NODE_ID env var)
pub async fn archive_audit_report(report_json: &str, source_node: Option<&str>) -> Result<String, String> {
    // Create Qdrant client from environment
    let qdrant_url = std::env::var("QDRANT_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:6334".to_string());
    let qdrant_api_key = std::env::var("QDRANT_API_KEY").ok();
    
    let qdrant_client = Arc::new(
        Qdrant::from_url(&qdrant_url)
            .with_api_key(qdrant_api_key)
            .build()
            .map_err(|e| format!("Failed to create Qdrant client: {}", e))?,
    );
    // Parse the audit report
    let report: AuditReport = serde_json::from_str(report_json)
        .map_err(|e| format!("Failed to parse audit report JSON: {}", e))?;

    // Extract affected paths from evidence
    let affected_paths = report.evidence.clone();

    // Determine severity based on status and climax
    let severity = if report.status == "error" || report.status == "blocked" {
        "CRITICAL"
    } else if !report.climax.is_empty() {
        "HIGH"
    } else if !report.rising_action.is_empty() {
        "MEDIUM"
    } else {
        "LOW"
    };

    // Create text for embedding (combine rising_action and climax)
    let text_for_embedding = format!(
        "{}\n{}",
        report.rising_action.join("\n"),
        report.climax
    );

    // Generate embedding
    let embedding_dim = std::env::var("EMBEDDING_MODEL_DIM")
        .unwrap_or_else(|_| "384".to_string())
        .parse::<usize>()
        .unwrap_or(384);

    // Import the generate_dense_vector function from phoenix_routes
    // For now, we'll use a simple approach: call the embedding generation
    // We need to access the embedding model - let's use the same approach as phoenix_routes
    let embedding = generate_embedding_for_audit(&text_for_embedding, embedding_dim).await;

    // Ensure collection exists
    let collection_name = "audit_history";
    ensure_audit_collection(qdrant_client.clone(), collection_name, embedding_dim).await?;

    // Create point with metadata
    let point_id = uuid::Uuid::new_v4().to_string();
    let timestamp = Utc::now().to_rfc3339();

    // Get source node ID (from parameter, env var, or default)
    let node_id = source_node
        .map(|s| s.to_string())
        .or_else(|| std::env::var("NODE_ID").ok())
        .unwrap_or_else(|| "unknown".to_string());

    let mut payload: HashMap<String, Value> = HashMap::new();
    payload.insert("timestamp".to_string(), qdrant_string_value(timestamp.clone()));
    payload.insert("status".to_string(), qdrant_string_value(report.status.clone()));
    payload.insert("executive_pulse".to_string(), qdrant_string_value(report.executive_pulse.clone()));
    payload.insert("climax".to_string(), qdrant_string_value(report.climax.clone()));
    payload.insert("severity".to_string(), qdrant_string_value(severity.to_string()));
    payload.insert("source_node".to_string(), qdrant_string_value(node_id.clone()));
    
    // Store affected paths as JSON array
    payload.insert(
        "affected_paths".to_string(),
        qdrant_string_value(serde_json::to_string(&affected_paths).unwrap_or_default()),
    );

    // Store full report as JSON
    payload.insert(
        "report_json".to_string(),
        qdrant_string_value(report_json.to_string()),
    );

    // Store rising_action as JSON array
    payload.insert(
        "rising_action".to_string(),
        qdrant_string_value(serde_json::to_string(&report.rising_action).unwrap_or_default()),
    );

    // Create point
    let point = PointStruct {
        id: Some(qdrant_client::qdrant::PointId {
            point_id_options: Some(qdrant_client::qdrant::point_id::PointIdOptions::Uuid(
                point_id.clone(),
            )),
        }),
        vectors: Some(qdrant_client::qdrant::Vectors {
            vectors_options: Some(qdrant_client::qdrant::vectors::VectorsOptions::Vector(
                qdrant_client::qdrant::Vector { data: embedding },
            )),
        }),
        payload,
    };

    // Upsert point
    match qdrant_client
        .upsert_points(qdrant_client::qdrant::UpsertPoints {
            collection_name: collection_name.to_string(),
            points: vec![point],
            ..Default::default()
        })
        .await
    {
        Ok(_) => {
            info!(
                point_id = %point_id,
                severity = %severity,
                "Audit report archived successfully"
            );
            Ok(json!({
                "status": "success",
                "point_id": point_id,
                "timestamp": timestamp,
                "severity": severity,
                "affected_paths_count": affected_paths.len()
            })
            .to_string())
        }
        Err(e) => {
            error!(error = %e, "Failed to archive audit report to Qdrant");
            Err(format!("Failed to archive audit report: {}", e))
        }
    }
}

/// Search historical audit reports for a specific path
/// Creates its own Qdrant client connection from environment variables
/// 
/// # Arguments
/// * `path` - The path to search for in audit history
/// * `days` - Optional number of days to look back
/// * `source_node` - Optional node ID to filter by (None = search across all nodes)
pub async fn search_audit_history(
    path: &str,
    days: Option<u32>,
    source_node: Option<&str>,
) -> Result<Vec<HistoricalAuditReport>, String> {
    // Create Qdrant client from environment
    let qdrant_url = std::env::var("QDRANT_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:6334".to_string());
    let qdrant_api_key = std::env::var("QDRANT_API_KEY").ok();
    
    let qdrant_client = Arc::new(
        Qdrant::from_url(&qdrant_url)
            .with_api_key(qdrant_api_key)
            .build()
            .map_err(|e| format!("Failed to create Qdrant client: {}", e))?,
    );
    let collection_name = "audit_history";

    // Build filter for path and time range
    let mut filter_conditions = vec![];

    // Filter by source node if specified
    if let Some(node_id) = source_node {
        filter_conditions.push(Condition {
            condition_one_of: Some(qdrant_client::qdrant::condition::ConditionOneOf::Field(
                FieldCondition {
                    key: "source_node".to_string(),
                    r#match: Some(Match {
                        match_value: Some(qdrant_client::qdrant::r#match::MatchValue::Keyword(
                            node_id.to_string(),
                        )),
                    }),
                    range: None,
                    geo_bounding_box: None,
                    geo_radius: None,
                    values_count: None,
                },
            )),
        });
    }

    // Filter by path (search in affected_paths)
    filter_conditions.push(Condition {
        condition_one_of: Some(qdrant_client::qdrant::condition::ConditionOneOf::Field(
            FieldCondition {
                key: "affected_paths".to_string(),
                r#match: Some(Match {
                    match_value: Some(qdrant_client::qdrant::r#match::MatchValue::Keyword(
                        path.to_string(),
                    )),
                }),
                range: None,
                geo_bounding_box: None,
                geo_radius: None,
                values_count: None,
            },
        )),
    });

    // Filter by time range if specified
    if let Some(days) = days {
        let cutoff_time = Utc::now() - chrono::Duration::days(days as i64);
        let cutoff_rfc3339 = cutoff_time.to_rfc3339();

        filter_conditions.push(Condition {
            condition_one_of: Some(qdrant_client::qdrant::condition::ConditionOneOf::Field(
                FieldCondition {
                    key: "timestamp".to_string(),
                    r#match: None,
                    range: Some(qdrant_client::qdrant::Range {
                        lt: None,
                        gt: Some(cutoff_rfc3339),
                        gte: None,
                        lte: None,
                    }),
                    geo_bounding_box: None,
                    geo_radius: None,
                    values_count: None,
                },
            )),
        });
    }

    let filter = Filter {
        must: filter_conditions,
        must_not: vec![],
        should: vec![],
        min_should: None,
    };

    // Scroll points matching the filter
    let scroll_request = ScrollPoints {
        collection_name: collection_name.to_string(),
        filter: Some(filter),
        limit: Some(100), // Limit to last 100 reports
        offset: None,
        with_payload: Some(true.into()),
        with_vectors: Some(false.into()),
        ..Default::default()
    };

    match qdrant_client.scroll(&scroll_request).await {
        Ok(scroll_result) => {
            let mut reports = Vec::new();

            for point in scroll_result.result {
                let point_id = extract_point_id_from_point(&point);
                let timestamp = extract_string_from_payload(&point.payload, "timestamp")
                    .unwrap_or_else(|| Utc::now().to_rfc3339());
                let severity = extract_string_from_payload(&point.payload, "severity")
                    .unwrap_or_else(|| "UNKNOWN".to_string());
                let source_node = extract_string_from_payload(&point.payload, "source_node")
                    .unwrap_or_else(|| "unknown".to_string());
                let report_json = extract_string_from_payload(&point.payload, "report_json")
                    .unwrap_or_default();
                let affected_paths_json = extract_string_from_payload(&point.payload, "affected_paths")
                    .unwrap_or_default();

                let affected_paths: Vec<String> = serde_json::from_str(&affected_paths_json)
                    .unwrap_or_default();

                let report: AuditReport = serde_json::from_str(&report_json)
                    .unwrap_or_else(|_| AuditReport {
                        status: "unknown".to_string(),
                        executive_pulse: "Unknown".to_string(),
                        rising_action: vec![],
                        climax: "Unknown".to_string(),
                        resolutions: vec![],
                        evidence: affected_paths.clone(),
                        next_steps: vec![],
                        tool_discoveries: vec![],
                    });

                reports.push(HistoricalAuditReport {
                    id: point_id,
                    timestamp,
                    report,
                    affected_paths,
                    severity,
                    source_node,
                });
            }

            // Sort by timestamp (newest first)
            reports.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

            Ok(reports)
        }
        Err(e) => {
            error!(error = %e, path = %path, "Failed to search audit history");
            Err(format!("Failed to search audit history: {}", e))
        }
    }
}

/// Perform trend analysis for a specific path
/// Creates its own Qdrant client connection from environment variables
pub async fn analyze_audit_trends(
    path: &str,
    days: Option<u32>,
    source_node: Option<&str>,
) -> Result<TrendAnalysis, String> {
    let historical_reports = search_audit_history(path, days, source_node).await?;

    if historical_reports.is_empty() {
        return Ok(TrendAnalysis {
            path: path.to_string(),
            change_count: 0,
            first_seen: String::new(),
            last_seen: String::new(),
            severity_escalation: false,
            recurring_climax: false,
            related_reports: vec![],
        });
    }

    let change_count = historical_reports.len();
    let first_seen = historical_reports
        .last()
        .map(|r| r.timestamp.clone())
        .unwrap_or_default();
    let last_seen = historical_reports
        .first()
        .map(|r| r.timestamp.clone())
        .unwrap_or_default();

    // Check for severity escalation (if latest report is more severe than earlier ones)
    let latest_severity = historical_reports
        .first()
        .map(|r| severity_to_number(&r.severity))
        .unwrap_or(0);
    let earliest_severity = historical_reports
        .last()
        .map(|r| severity_to_number(&r.severity))
        .unwrap_or(0);
    let severity_escalation = latest_severity > earliest_severity;

    // Check for recurring climax (same climax in multiple reports)
    let climaxes: Vec<String> = historical_reports
        .iter()
        .map(|r| r.report.climax.clone())
        .collect();
    let unique_climaxes: std::collections::HashSet<String> = climaxes.iter().cloned().collect();
    let recurring_climax = unique_climaxes.len() < climaxes.len() && change_count >= 3;

    let related_reports: Vec<String> = historical_reports
        .iter()
        .map(|r| r.id.clone())
        .collect();

    Ok(TrendAnalysis {
        path: path.to_string(),
        change_count,
        first_seen,
        last_seen,
        severity_escalation,
        recurring_climax,
        related_reports,
    })
}

// Helper functions

fn qdrant_string_value(s: String) -> Value {
    Value {
        kind: Some(qdrant_client::qdrant::value::Kind::StringValue(s)),
    }
}

fn extract_point_id_from_point(point: &qdrant_client::qdrant::PointStruct) -> String {
    match &point.id {
        Some(id) => match id {
            qdrant_client::qdrant::PointId {
                point_id_options: Some(opt),
            } => match opt {
                qdrant_client::qdrant::point_id::PointIdOptions::Num(num) => num.to_string(),
                qdrant_client::qdrant::point_id::PointIdOptions::Uuid(uuid) => uuid.clone(),
            },
            _ => "unknown".to_string(),
        },
        None => "unknown".to_string(),
    }
}

fn extract_string_from_payload(
    payload: &HashMap<String, Value>,
    key: &str,
) -> Option<String> {
    payload.get(key).and_then(|v| match &v.kind {
        Some(qdrant_client::qdrant::value::Kind::StringValue(s)) => Some(s.clone()),
        _ => None,
    })
}

fn severity_to_number(severity: &str) -> u8 {
    match severity {
        "CRITICAL" => 4,
        "HIGH" => 3,
        "MEDIUM" => 2,
        "LOW" => 1,
        _ => 0,
    }
}

async fn ensure_audit_collection(
    qdrant_client: Arc<Qdrant>,
    collection_name: &str,
    embedding_dim: usize,
) -> Result<(), String> {
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
            "Creating audit_history Qdrant collection"
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
            .map_err(|e| format!("Failed to create audit_history collection: {}", e))?;

        info!(collection = %collection_name, "audit_history collection created");
    }

    Ok(())
}

/// Generate embedding for audit report text
/// Uses fastembed model (same as phoenix_routes)
async fn generate_embedding_for_audit(text: &str, expected_dim: usize) -> Vec<f32> {
    use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};
    use std::sync::{Arc, Mutex};
    use std::sync::OnceLock;
    
    // Global embedding model (same pattern as phoenix_routes)
    static EMBEDDING_MODEL: OnceLock<Option<Arc<Mutex<TextEmbedding>>>> = OnceLock::new();
    
    // Initialize model if needed
    let model_opt = EMBEDDING_MODEL.get_or_init(|| {
        let model_name = std::env::var("EMBEDDING_MODEL_NAME")
            .unwrap_or_else(|_| "all-MiniLM-L6-v2".to_string());
        
        let model_type: EmbeddingModel = match model_name.as_str() {
            "all-MiniLM-L6-v2" | "sentence-transformers/all-MiniLM-L6-v2" => {
                EmbeddingModel::AllMiniLML6V2
            }
            "BAAI/bge-small-en-v1.5" => EmbeddingModel::BGESmallENV15,
            "BAAI/bge-base-en-v1.5" => EmbeddingModel::BGEBaseENV15,
            _ => EmbeddingModel::AllMiniLML6V2,
        };
        
        let init_options = TextInitOptions::new(model_type).with_show_download_progress(false);
        
        match TextEmbedding::try_new(init_options) {
            Ok(model) => {
                info!("Embedding model initialized for audit archiver");
                Some(Arc::new(Mutex::new(model)))
            }
            Err(e) => {
                warn!(error = %e, "Failed to initialize embedding model, will use hash fallback");
                None
            }
        }
    });
    
    // Try to use the model if available
    if let Some(model_arc) = model_opt {
    
    // Truncate text if too long (same limit as phoenix_routes)
    const MAX_QUERY_CHARS: usize = 1000;
    let truncated_text = if text.len() > MAX_QUERY_CHARS {
        &text[..MAX_QUERY_CHARS]
    } else {
        text
    };
    
        // Generate embedding (blocking operation, so we use spawn_blocking)
        match tokio::task::spawn_blocking({
            let model_arc = model_arc.clone();
            let text = truncated_text.to_string();
            move || {
                let model = model_arc.lock().unwrap();
                model.embed(vec![text.as_str()], None)
            }
        }).await {
            Ok(Ok(embeddings)) => {
                if let Some(embedding) = embeddings.first() {
                    if embedding.len() == expected_dim {
                        return embedding.clone();
                    } else {
                        warn!(
                            expected_dim = expected_dim,
                            actual_dim = embedding.len(),
                            "Embedding dimension mismatch, using hash fallback"
                        );
                    }
                }
            }
            Ok(Err(e)) => {
                warn!(error = %e, "Embedding generation failed, using hash fallback");
            }
            Err(e) => {
                warn!(error = %e, "Task join error during embedding, using hash fallback");
            }
        }
    }
    
    // Fallback to hash-based embedding
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    
    let mut embedding = vec![0.0f32; expected_dim];
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    let hash = hasher.finish();
    
    // Distribute hash across dimensions
    for i in 0..expected_dim {
        let hash_val = (hash.wrapping_mul((i as u64).wrapping_add(1))) % 1000;
        embedding[i] = (hash_val as f32) / 1000.0;
    }
    
    // Normalize
    let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for val in &mut embedding {
            *val /= norm;
        }
    }
    
    embedding
}
