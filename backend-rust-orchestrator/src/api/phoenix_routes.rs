//! Phoenix API Routes - SSE Broadcaster and REST Endpoints
//!
//! This module provides:
//! 1. SSE stream for PhoenixEvent variants (ConsensusVote, QuarantineAlert, MemoryTransfer)
//! 2. REST endpoints for consensus status, voting, and memory statistics

use axum::{
    extract::{Path, State},
    response::{sse::{Event, KeepAlive, Sse}, Response},
    routing::{get, post},
    Json, Router,
};
use axum::http::{header, StatusCode};
use chrono::Utc;
use futures_core::stream::Stream;
use serde::{Deserialize, Serialize};
use std::{
    convert::Infallible,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::broadcast;
use tracing::{error, info, warn};

use crate::bus::{GlobalMessageBus, PhoenixEvent};
use crate::network::consensus::PhoenixConsensus;
use crate::network::memory_exchange::PhoenixMemoryExchangeServiceImpl;
use crate::api::reports;
use crate::api::feedback_storage::{FeedbackStorage, FeedbackEntry};
use crate::foundry::optimizer;
use crate::security::PrivacyFilter;
use qdrant_client::{
    qdrant::{ScrollPoints, Filter, Condition, FieldCondition, Match, Value, PointStruct, SearchPoints, SparseVector, SparseIndices},
    Qdrant,
};
use std::collections::{HashMap, HashSet, BinaryHeap};
use std::cmp::Reverse;
use std::sync::OnceLock;
use std::path::PathBuf;
use axum::extract::Query;

/// App state for Phoenix routes
#[derive(Clone)]
pub struct PhoenixAppState {
    pub message_bus: Arc<GlobalMessageBus>,
    pub consensus: Arc<PhoenixConsensus>,
    pub memory_exchange: Arc<PhoenixMemoryExchangeServiceImpl>,
    pub node_id: String,
    pub agents_repo_path: std::path::PathBuf,
    pub qdrant_client: Arc<Qdrant>,
    pub feedback_storage: Arc<FeedbackStorage>,
}

/// SSE stream handler for Phoenix events
pub async fn phoenix_sse_stream(
    State(state): State<PhoenixAppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let mut receiver = state.message_bus.subscribe();
    
    let stream = async_stream::stream! {
        loop {
            match receiver.recv().await {
                Ok(event) => {
                    // Only stream specific Phoenix events
                    let should_stream = matches!(
                        event,
                        PhoenixEvent::ConsensusVote { .. }
                            | PhoenixEvent::ConsensusResult { .. }
                            | PhoenixEvent::QuarantineAlert { .. }
                            | PhoenixEvent::MemoryTransfer { .. }
                    );
                    
                    if should_stream {
                        match serde_json::to_string(&event) {
                            Ok(json) => {
                                let event_type = match event {
                                    PhoenixEvent::ConsensusVote { .. } => "consensus_vote",
                                    PhoenixEvent::ConsensusResult { .. } => "consensus_result",
                                    PhoenixEvent::QuarantineAlert { .. } => "quarantine_alert",
                                    PhoenixEvent::MemoryTransfer { .. } => "memory_transfer",
                                    _ => "phoenix_event",
                                };
                                yield Ok(Event::default().event(event_type).data(json));
                            }
                            Err(e) => {
                                error!(error = %e, "Failed to serialize Phoenix event");
                            }
                        }
                    }
                }
                Err(broadcast::error::RecvError::Closed) => {
                    warn!("Phoenix event stream closed");
                    break;
                }
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    warn!(skipped = skipped, "Phoenix event stream lagged");
                }
            }
        }
    };

    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(10)).text("keep-alive"))
}

/// Consensus status response
#[derive(Debug, Serialize)]
pub struct ConsensusStatusResponse {
    pub commit_hash: String,
    pub vote_count: usize,
    pub approval_percentage: f64,
    pub average_score: f64,
    pub approved: bool,
    pub status: String, // "pending", "approved", "rejected", "timeout"
}

/// Get consensus status for a commit
pub async fn get_consensus_status(
    State(state): State<PhoenixAppState>,
    Path(commit_hash): Path<String>,
) -> Result<Json<ConsensusStatusResponse>, axum::http::StatusCode> {
    // Access the consensus service's active sessions
    let sessions = state.consensus.get_active_sessions().await;
    
    if let Some(session) = sessions.get(&commit_hash) {
        let total_votes = session.votes.len();
        let approved_votes = session.votes.iter().filter(|v| v.approved).count();
        let approval_percentage = if total_votes > 0 {
            (approved_votes as f64 / total_votes as f64) * 100.0
        } else {
            0.0
        };
        let average_score = if total_votes > 0 {
            session.votes.iter().map(|v| v.compliance_score).sum::<f64>() / total_votes as f64
        } else {
            0.0
        };
        
        let config = state.consensus.get_config().await;
        let approved = average_score >= config.min_average_score 
            && approval_percentage >= config.min_approval_percentage;
        
        let status = if approved {
            "approved"
        } else if total_votes == 0 {
            "pending"
        } else {
            "rejected"
        };
        
        Ok(Json(ConsensusStatusResponse {
            commit_hash: commit_hash.clone(),
            vote_count: total_votes,
            approval_percentage,
            average_score,
            approved,
            status: status.to_string(),
        }))
    } else {
        // Check if it's in quarantine
        let mesh_quarantine = state.consensus.get_mesh_quarantine().await;
        if mesh_quarantine.contains_key(&commit_hash) {
            Ok(Json(ConsensusStatusResponse {
                commit_hash: commit_hash.clone(),
                vote_count: 0,
                approval_percentage: 0.0,
                average_score: 0.0,
                approved: false,
                status: "rejected".to_string(),
            }))
        } else {
            Ok(Json(ConsensusStatusResponse {
                commit_hash: commit_hash.clone(),
                vote_count: 0,
                approval_percentage: 0.0,
                average_score: 0.0,
                approved: false,
                status: "pending".to_string(),
            }))
        }
    }
}

/// Manual vote request
#[derive(Debug, Deserialize)]
pub struct ManualVoteRequest {
    pub commit_hash: String,
    pub approved: bool,
    pub compliance_score: Option<f64>,
}

/// Manual vote response
#[derive(Debug, Serialize)]
pub struct ManualVoteResponse {
    pub success: bool,
    pub message: String,
}

/// Submit a manual human vote
pub async fn post_consensus_vote(
    State(state): State<PhoenixAppState>,
    Json(request): Json<ManualVoteRequest>,
) -> Result<Json<ManualVoteResponse>, axum::http::StatusCode> {
    let compliance_score = request.compliance_score.unwrap_or(if request.approved { 80.0 } else { 30.0 });
    
    // Publish a manual vote event
    let vote_event = PhoenixEvent::ConsensusVote {
        commit_hash: request.commit_hash.clone(),
        voting_node: state.node_id.clone(),
        compliance_score,
        approved: request.approved,
        timestamp: Utc::now().to_rfc3339(),
    };
    
    state.message_bus.publish(vote_event);
    
    info!(
        commit_hash = %request.commit_hash,
        approved = request.approved,
        compliance_score = compliance_score,
        "Manual consensus vote submitted"
    );
    
    Ok(Json(ManualVoteResponse {
        success: true,
        message: format!("Vote recorded for commit {}", request.commit_hash),
    }))
}

/// Memory statistics response
#[derive(Debug, Serialize)]
pub struct MemoryStatsResponse {
    pub bytes_transferred_24h: u64,
    pub fragments_exchanged_24h: usize,
    pub active_transfers: usize,
    pub total_nodes: usize,
}

/// Get memory exchange statistics
pub async fn get_memory_stats(
    State(state): State<PhoenixAppState>,
) -> Result<Json<MemoryStatsResponse>, axum::http::StatusCode> {
    // Get statistics from memory exchange service
    let stats = state.memory_exchange.get_statistics().await;
    
    // Get verified peers count
    let total_nodes = state.memory_exchange.get_verified_peers_count().await;
    
    Ok(Json(MemoryStatsResponse {
        bytes_transferred_24h: stats.bytes_transferred_24h,
        fragments_exchanged_24h: stats.fragments_exchanged_24h,
        active_transfers: stats.active_transfers,
        total_nodes,
    }))
}

/// Topic heat map response
#[derive(Debug, Serialize)]
pub struct TopicHeatMapResponse {
    pub topic_frequencies: std::collections::HashMap<String, usize>,
    pub node_volumes: std::collections::HashMap<String, usize>,
}

/// Get topic heat map data
pub async fn get_topic_heat_map(
    State(state): State<PhoenixAppState>,
) -> Result<Json<TopicHeatMapResponse>, axum::http::StatusCode> {
    let topic_frequencies = state.memory_exchange.get_topic_frequencies().await;
    let node_volumes = state.memory_exchange.get_node_volumes().await;
    
    Ok(Json(TopicHeatMapResponse {
        topic_frequencies,
        node_volumes,
    }))
}

/// Vote detail response
#[derive(Debug, Serialize)]
pub struct VoteDetail {
    pub node_id: String,
    pub compliance_score: f64,
    pub approved: bool,
    pub timestamp: String,
}

/// Votes response
#[derive(Debug, Serialize)]
pub struct VotesResponse {
    pub votes: Vec<VoteDetail>,
}

/// Get detailed votes for a commit
pub async fn get_consensus_votes(
    State(state): State<PhoenixAppState>,
    Path(commit_hash): Path<String>,
) -> Result<Json<VotesResponse>, axum::http::StatusCode> {
    if let Some(votes) = state.consensus.get_votes_for_commit(&commit_hash).await {
        let vote_details: Vec<VoteDetail> = votes
            .into_iter()
            .map(|v| VoteDetail {
                node_id: v.node_id,
                compliance_score: v.compliance_score,
                approved: v.approved,
                timestamp: v.timestamp,
            })
            .collect();
        
        Ok(Json(VotesResponse {
            votes: vote_details,
        }))
    } else {
        // Return empty votes if no session found
        Ok(Json(VotesResponse {
            votes: Vec::new(),
        }))
    }
}

/// Strategic override request
#[derive(Debug, Deserialize)]
pub struct StrategicOverrideRequest {
    pub commit_hash: String,
    pub rationale: String,
}

/// Strategic override response
#[derive(Debug, Serialize)]
pub struct StrategicOverrideResponse {
    pub success: bool,
    pub message: String,
}

/// Perform strategic override
pub async fn post_strategic_override(
    State(state): State<PhoenixAppState>,
    Json(request): Json<StrategicOverrideRequest>,
) -> Result<Json<StrategicOverrideResponse>, axum::http::StatusCode> {
    match state.consensus.strategic_override(
        request.commit_hash.clone(),
        request.rationale.clone(),
    ).await {
        Ok(_) => {
            info!(
                commit_hash = %request.commit_hash,
                "Strategic override completed successfully"
            );
            Ok(Json(StrategicOverrideResponse {
                success: true,
                message: format!("Strategic override applied to commit {}", request.commit_hash),
            }))
        }
        Err(e) => {
            error!(
                commit_hash = %request.commit_hash,
                error = %e,
                "Strategic override failed"
            );
            Ok(Json(StrategicOverrideResponse {
                success: false,
                message: format!("Override failed: {}", e),
            }))
        }
    }
}

/// Get latest governance report
pub async fn get_governance_report(
    State(state): State<PhoenixAppState>,
) -> Result<Response<String>, StatusCode> {
    match reports::generate_governance_report(
        &state.agents_repo_path,
        &state.consensus,
        &state.memory_exchange,
    ).await {
        Ok(report) => {
            let markdown = reports::generate_markdown_report(&report);
            
            // Return as Markdown with appropriate content type
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/markdown; charset=utf-8")
                .header(header::CONTENT_DISPOSITION, "attachment; filename=\"phoenix-governance-report.md\"")
                .body(markdown)
                .unwrap())
        }
        Err(e) => {
            error!(error = %e, "Failed to generate governance report");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get latest governance report as JSON
pub async fn get_governance_report_json(
    State(state): State<PhoenixAppState>,
) -> Result<Json<reports::GovernanceReport>, axum::http::StatusCode> {
    match reports::generate_governance_report(
        &state.agents_repo_path,
        &state.consensus,
        &state.memory_exchange,
    ).await {
        Ok(report) => Ok(Json(report)),
        Err(e) => {
            error!(error = %e, "Failed to generate governance report");
            Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Prune topic request
#[derive(Debug, Deserialize)]
pub struct PruneTopicRequest {
    pub topic: String,
}

/// Prune topic response
#[derive(Debug, Serialize)]
pub struct PruneTopicResponse {
    pub success: bool,
    pub message: String,
    pub deleted_count: usize,
}

/// Prune a topic from memory collections
pub async fn post_prune_topic(
    State(state): State<PhoenixAppState>,
    Json(request): Json<PruneTopicRequest>,
) -> Result<Json<PruneTopicResponse>, axum::http::StatusCode> {
    info!(topic = %request.topic, "Prune topic request received");
    
    // Safety check: require recent snapshot
    if !state.memory_exchange.has_recent_snapshot().await {
        warn!(
            topic = %request.topic,
            "Prune request rejected: no recent snapshot"
        );
        return Ok(Json(PruneTopicResponse {
            success: false,
            message: "Pruning requires a snapshot taken within the last 60 minutes. Please create a snapshot first.".to_string(),
            deleted_count: 0,
        }));
    }
    
    match state.memory_exchange.prune_topic(&request.topic).await {
        Ok(deleted_count) => {
            info!(
                topic = %request.topic,
                deleted = deleted_count,
                "Topic pruned successfully"
            );
            Ok(Json(PruneTopicResponse {
                success: true,
                message: format!("Topic '{}' pruned successfully", request.topic),
                deleted_count,
            }))
        }
        Err(e) => {
            error!(
                topic = %request.topic,
                error = %e,
                "Failed to prune topic"
            );
            Ok(Json(PruneTopicResponse {
                success: false,
                message: format!("Failed to prune topic: {}", e),
                deleted_count: 0,
            }))
        }
    }
}

/// Snapshot request response
#[derive(Debug, Serialize)]
pub struct SnapshotResponse {
    pub success: bool,
    pub message: String,
    pub snapshot_paths: Vec<String>,
    pub timestamp: String,
}

/// Create mesh-wide memory snapshots
pub async fn post_create_snapshot(
    State(state): State<PhoenixAppState>,
) -> Result<Json<SnapshotResponse>, axum::http::StatusCode> {
    info!("Mesh snapshot request received");
    
    match state.memory_exchange.create_mesh_snapshot().await {
        Ok(snapshot_paths) => {
            info!(
                snapshot_count = snapshot_paths.len(),
                "Mesh snapshot created successfully"
            );
            Ok(Json(SnapshotResponse {
                success: true,
                message: format!("Created {} snapshot(s) successfully", snapshot_paths.len()),
                snapshot_paths,
                timestamp: chrono::Utc::now().to_rfc3339(),
            }))
        }
        Err(e) => {
            error!(error = %e, "Failed to create mesh snapshot");
            Ok(Json(SnapshotResponse {
                success: false,
                message: format!("Failed to create snapshot: {}", e),
                snapshot_paths: Vec::new(),
                timestamp: chrono::Utc::now().to_rfc3339(),
            }))
        }
    }
}

/// Get snapshot status
#[derive(Debug, Serialize)]
pub struct SnapshotStatusResponse {
    pub has_recent_snapshot: bool,
    pub last_snapshot_time: Option<String>,
}

/// Get snapshot status
pub async fn get_snapshot_status(
    State(state): State<PhoenixAppState>,
) -> Result<Json<SnapshotStatusResponse>, axum::http::StatusCode> {
    let has_recent = state.memory_exchange.has_recent_snapshot().await;
    let last_snapshot_time = state.memory_exchange.get_last_snapshot_time().await
        .and_then(|time| {
            time.duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|d| chrono::DateTime::<chrono::Utc>::from_timestamp(d.as_secs() as i64, 0))
                .flatten()
                .map(|dt| dt.to_rfc3339())
        });
    
    Ok(Json(SnapshotStatusResponse {
        has_recent_snapshot: has_recent,
        last_snapshot_time,
    }))
}

/// Snapshot info response
#[derive(Debug, Serialize)]
pub struct SnapshotInfoResponse {
    pub snapshot_id: String,
    pub collection_name: String,
    pub creation_time: String,
    pub size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compliance_score: Option<f64>,
    pub is_recommended: bool,
    pub is_blessed: bool,
}

/// List snapshots response
#[derive(Debug, Serialize)]
pub struct ListSnapshotsResponse {
    pub snapshots: Vec<SnapshotInfoResponse>,
}

/// List all available snapshots
pub async fn get_snapshots(
    State(state): State<PhoenixAppState>,
) -> Result<Json<ListSnapshotsResponse>, axum::http::StatusCode> {
    match state.memory_exchange.list_snapshots().await {
        Ok(snapshots) => {
            let snapshot_responses: Vec<SnapshotInfoResponse> = snapshots
                .into_iter()
                .map(|s| SnapshotInfoResponse {
                    snapshot_id: s.snapshot_id,
                    collection_name: s.collection_name,
                    creation_time: s.creation_time,
                    size: s.size,
                    compliance_score: s.compliance_score,
                    is_recommended: s.is_recommended,
                    is_blessed: s.is_blessed,
                })
                .collect();
            
            Ok(Json(ListSnapshotsResponse {
                snapshots: snapshot_responses,
            }))
        }
        Err(e) => {
            error!(error = %e, "Failed to list snapshots");
            Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Restore snapshot request
#[derive(Debug, Deserialize)]
pub struct RestoreSnapshotRequest {
    pub snapshot_id: String,
    pub collection_name: String,
}

/// Restore snapshot response
#[derive(Debug, Serialize)]
pub struct RestoreSnapshotResponse {
    pub success: bool,
    pub message: String,
}

/// Restore from a snapshot
pub async fn post_restore_snapshot(
    State(state): State<PhoenixAppState>,
    Json(request): Json<RestoreSnapshotRequest>,
) -> Result<Json<RestoreSnapshotResponse>, axum::http::StatusCode> {
    info!(
        snapshot_id = %request.snapshot_id,
        collection = %request.collection_name,
        "Snapshot restore request received"
    );

    // Check if already in maintenance mode
    if state.memory_exchange.is_maintenance_mode().await {
        return Ok(Json(RestoreSnapshotResponse {
            success: false,
            message: "System is already in maintenance mode. Please wait for current operation to complete.".to_string(),
        }));
    }

    match state.memory_exchange.restore_from_snapshot(
        &request.snapshot_id,
        &request.collection_name,
    ).await {
        Ok(_) => {
            info!(
                snapshot_id = %request.snapshot_id,
                collection = %request.collection_name,
                "Snapshot restore completed successfully"
            );
            Ok(Json(RestoreSnapshotResponse {
                success: true,
                message: format!("Snapshot {} restored successfully for collection {}", 
                    request.snapshot_id, request.collection_name),
            }))
        }
        Err(e) => {
            error!(
                snapshot_id = %request.snapshot_id,
                collection = %request.collection_name,
                error = %e,
                "Failed to restore snapshot"
            );
            Ok(Json(RestoreSnapshotResponse {
                success: false,
                message: format!("Failed to restore snapshot: {}", e),
            }))
        }
    }
}

/// Maintenance mode status response
#[derive(Debug, Serialize)]
pub struct MaintenanceModeStatusResponse {
    pub enabled: bool,
}

/// Get maintenance mode status
pub async fn get_maintenance_mode_status(
    State(state): State<PhoenixAppState>,
) -> Result<Json<MaintenanceModeStatusResponse>, axum::http::StatusCode> {
    let enabled = state.memory_exchange.is_maintenance_mode().await;
    Ok(Json(MaintenanceModeStatusResponse { enabled }))
}

/// Get draft rules from optimizer
pub async fn get_optimizer_drafts(
    State(state): State<PhoenixAppState>,
) -> Result<Json<optimizer::DraftRulesResponse>, axum::http::StatusCode> {
    // Fetch the latest governance report
    let report = match reports::generate_governance_report(&state.agents_repo_path).await {
        Ok(r) => r,
        Err(e) => {
            error!(error = %e, "Failed to generate governance report");
            return Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Generate draft rules
    match optimizer::generate_draft_rules(&report).await {
        Ok(drafts) => {
            Ok(Json(optimizer::DraftRulesResponse {
                total_recommendations_analyzed: report.strategic_recommendations
                    .as_ref()
                    .map(|r| r.len())
                    .unwrap_or(0),
                drafts,
            }))
        }
        Err(e) => {
            error!(error = %e, "Failed to generate draft rules");
            Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Apply draft rule request
#[derive(Debug, Deserialize)]
pub struct ApplyRuleRequest {
    pub rule_id: String,
}

/// Apply draft rule response
#[derive(Debug, Serialize)]
pub struct ApplyRuleResponse {
    pub success: bool,
    pub message: String,
}

/// Apply a draft rule
pub async fn apply_optimizer_rule(
    State(state): State<PhoenixAppState>,
    Json(request): Json<ApplyRuleRequest>,
) -> Result<Json<ApplyRuleResponse>, axum::http::StatusCode> {
    // Fetch the latest governance report
    let report = match reports::generate_governance_report(&state.agents_repo_path).await {
        Ok(r) => r,
        Err(e) => {
            error!(error = %e, "Failed to generate governance report");
            return Ok(Json(ApplyRuleResponse {
                success: false,
                message: format!("Failed to load governance report: {}", e),
            }));
        }
    };

    // Generate draft rules and find the one to apply
    let drafts = match optimizer::generate_draft_rules(&report).await {
        Ok(d) => d,
        Err(e) => {
            error!(error = %e, "Failed to generate draft rules");
            return Ok(Json(ApplyRuleResponse {
                success: false,
                message: format!("Failed to generate draft rules: {}", e),
            }));
        }
    };

    let rule = match drafts.iter().find(|r| r.id == request.rule_id) {
        Some(r) => r,
        None => {
            return Ok(Json(ApplyRuleResponse {
                success: false,
                message: format!("Rule {} not found", request.rule_id),
            }));
        }
    };

    // Apply the rule
    match optimizer::apply_draft_rule(rule, &state.agents_repo_path).await {
        Ok(message) => {
            // Broadcast UpdateConfig event to mesh peers
            let rule_type_str = match &rule.rule_type {
                optimizer::RuleType::PythonRegex { .. } => "python_regex",
                optimizer::RuleType::RustFilter { .. } => "rust_filter",
                optimizer::RuleType::ConfigUpdate { .. } => "config_update",
            };

            let config_data = serde_json::to_string(rule)
                .unwrap_or_else(|_| "{}".to_string());

            let update_event = PhoenixEvent::UpdateConfig {
                rule_id: rule.id.clone(),
                rule_type: rule_type_str.to_string(),
                config_data,
                applied_by: state.node_id.clone(),
                timestamp: Utc::now().to_rfc3339(),
            };

            state.message_bus.publish(update_event);
            info!(rule_id = %rule.id, "Draft rule applied and broadcast to mesh");

            Ok(Json(ApplyRuleResponse {
                success: true,
                message,
            }))
        }
        Err(e) => {
            error!(error = %e, rule_id = %rule.id, "Failed to apply draft rule");
            Ok(Json(ApplyRuleResponse {
                success: false,
                message: format!("Failed to apply rule: {}", e),
            }))
        }
    }
}

/// Memory query request
#[derive(Debug, Deserialize)]
pub struct MemoryQueryRequest {
    pub query: String,
    pub namespace: Option<String>,
    pub top_k: Option<u32>,
    /// Bias parameter: -1.0 (strict keyword) to 1.0 (strict semantic)
    /// 0.0 = balanced (default)
    pub bias: Option<f64>,
    /// Enable Deep Verify with Cross-Encoder re-ranking for top 5 results
    /// Default: false (for performance)
    pub deep_verify: Option<bool>,
}

/// Memory query result
#[derive(Debug, Serialize)]
pub struct MemoryQueryResult {
    pub id: String,
    pub content: String,
    pub namespace: String,
    pub similarity: f64,
    pub timestamp: Option<String>,
    pub twin_id: Option<String>,
    /// Cross-Encoder score from Deep Verify stage (if enabled)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cross_encoder_score: Option<f64>,
    /// Verification confidence status: "High Confidence" if score > 0.8, "Medium" if > 0.5, "Low" otherwise
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification_status: Option<String>,
    /// Best-scoring snippet for long documents (chunked verification)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
}

/// Memory query response
#[derive(Debug, Serialize)]
pub struct MemoryQueryResponse {
    pub results: Vec<MemoryQueryResult>,
    pub total: usize,
}

/// Search result with ranking information for RRF
#[derive(Debug, Clone)]
struct RankedResult {
    point_id: String,
    score: f64,
    rank: usize,
    namespace: String,
    content: String,
    timestamp: Option<String>,
    twin_id: Option<String>,
    /// Cross-Encoder score from Deep Verify (if applied)
    cross_encoder_score: Option<f64>,
    /// Verification confidence status
    verification_status: Option<String>,
    /// Best-scoring snippet for long documents
    snippet: Option<String>,
}

/// Generate sparse vector (BM25-like term frequency) from text
fn generate_sparse_vector(text: &str) -> SparseVector {
    use std::collections::HashMap;
    
    // Simple tokenization (split on whitespace and punctuation)
    let tokens: Vec<String> = text
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();
    
    // Count term frequencies
    let mut term_freq: HashMap<String, f32> = HashMap::new();
    for token in &tokens {
        *term_freq.entry(token.clone()).or_insert(0.0) += 1.0;
    }
    
    // Convert to sparse vector format (indices and values)
    // For simplicity, we'll use a hash-based index
    // In production, you'd maintain a vocabulary mapping
    let mut indices = Vec::new();
    let mut values = Vec::new();
    
    for (token, freq) in term_freq {
        // Simple hash-based index (modulo to keep it reasonable)
        let hash = token.as_bytes().iter().fold(0u64, |acc, &b| acc.wrapping_mul(31).wrapping_add(b as u64));
        let index = (hash % 10000) as u32; // Limit to 10k dimensions
        
        // Apply TF-IDF-like weighting (sqrt of frequency for sublinear scaling)
        let value = freq.sqrt();
        
        indices.push(index);
        values.push(value);
    }
    
    SparseVector {
        indices: Some(SparseIndices { data: indices }),
        values,
    }
}

/// Global embedding model instance (lazy initialization)
/// Using std::sync::Mutex because fastembed operations are blocking
static EMBEDDING_MODEL: OnceLock<Arc<std::sync::Mutex<fastembed::EmbeddingModel>>> = OnceLock::new();

/// Initialize the embedding model (called once on first use)
fn init_embedding_model() -> Result<Arc<std::sync::Mutex<fastembed::EmbeddingModel>>, String> {
    use fastembed::{EmbeddingModel, InitOptions};
    
    // Get model name from environment or use default
    let model_name = std::env::var("EMBEDDING_MODEL_NAME")
        .unwrap_or_else(|_| "all-MiniLM-L6-v2".to_string());
    
    // Map model name to fastembed model type
    let model_type = match model_name.as_str() {
        "all-MiniLM-L6-v2" => fastembed::EmbeddingModel::AllMiniLmL6V2,
        "BAAI/bge-small-en-v1.5" => fastembed::EmbeddingModel::BgeSmallEnV15,
        "BAAI/bge-base-en-v1.5" => fastembed::EmbeddingModel::BgeBaseEnV15,
        _ => {
            warn!(
                model = %model_name,
                "Unknown model name, defaulting to all-MiniLM-L6-v2"
            );
            fastembed::EmbeddingModel::AllMiniLmL6V2
        }
    };
    
    let init_options = InitOptions {
        show_download_progress: false,
        ..Default::default()
    };
    
    match EmbeddingModel::try_new(model_type, init_options) {
            Ok(model) => {
            info!(
                model = %model_name,
                "Embedding model initialized successfully"
            );
            Ok(Arc::new(std::sync::Mutex::new(model)))
        }
        Err(e) => {
            error!(
                model = %model_name,
                error = %e,
                "Failed to initialize embedding model, falling back to hash-based embeddings"
            );
            Err(format!("Failed to initialize embedding model: {}", e))
        }
    }
}

/// Get or initialize the embedding model
fn get_embedding_model() -> Result<Arc<std::sync::Mutex<fastembed::EmbeddingModel>>, String> {
    EMBEDDING_MODEL.get_or_try_init(init_embedding_model).cloned()
}

/// Cross-Encoder model and tokenizer wrapper
struct CrossEncoderModel {
    session: ort::Session,
    tokenizer: tokenizers::Tokenizer,
}

/// Global Cross-Encoder model instance (lazy initialization)
static CROSS_ENCODER_MODEL: OnceLock<Arc<std::sync::Mutex<CrossEncoderModel>>> = OnceLock::new();

/// Initialize the Cross-Encoder model (called once on first use)
fn init_cross_encoder_model() -> Result<Arc<std::sync::Mutex<CrossEncoderModel>>, String> {
    use ort::{Session, SessionBuilder, Value};
    use tokenizers::Tokenizer;
    
    // Get model name from environment or use default
    let model_name = std::env::var("CROSS_ENCODER_MODEL_NAME")
        .unwrap_or_else(|_| "cross-encoder/ms-marco-MiniLM-L-6-v2".to_string());
    
    info!(
        model = %model_name,
        "Initializing Cross-Encoder model"
    );
    
    // Try to load model from local cache or download from HuggingFace
    // For now, we'll use a local path or download mechanism
    // The model should be in ONNX format
    let model_path = get_cross_encoder_model_path(&model_name)?;
    
    // Initialize ONNX session
    let session = SessionBuilder::new()
        .map_err(|e| format!("Failed to create ONNX session builder: {}", e))?
        .with_execution_providers([ort::ExecutionProvider::CPU(Default::default())])
        .map_err(|e| format!("Failed to set execution provider: {}", e))?
        .commit_from_file(&model_path)
        .map_err(|e| format!("Failed to load ONNX model from {}: {}", model_path.display(), e))?;
    
    // Load tokenizer (try to find tokenizer.json in the same directory as model)
    let tokenizer_path = model_path.parent()
        .ok_or_else(|| "Invalid model path".to_string())?
        .join("tokenizer.json");
    
    let tokenizer = if tokenizer_path.exists() {
        Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| format!("Failed to load tokenizer from {}: {}", tokenizer_path.display(), e))?
    } else {
        // Fallback: try to download or use a default tokenizer
        // For ms-marco-MiniLM-L-6-v2, we can use the AutoTokenizer from HuggingFace
        // For now, we'll create a basic tokenizer or download it
        warn!(
            path = %tokenizer_path.display(),
            "Tokenizer file not found, attempting to download or use default"
        );
        
        // Try to download tokenizer from HuggingFace
        download_cross_encoder_tokenizer(&model_name, tokenizer_path.parent().unwrap())?;
        
        Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| format!("Failed to load downloaded tokenizer: {}", e))?
    };
    
    info!(
        model = %model_name,
        "Cross-Encoder model initialized successfully"
    );
    
    Ok(Arc::new(std::sync::Mutex::new(CrossEncoderModel {
        session,
        tokenizer,
    })))
}

/// Get or initialize the Cross-Encoder model
fn get_cross_encoder_model() -> Result<Arc<std::sync::Mutex<CrossEncoderModel>>, String> {
    CROSS_ENCODER_MODEL.get_or_try_init(init_cross_encoder_model).cloned()
}

/// Get the path to the Cross-Encoder ONNX model
/// Tries local cache first, then downloads from HuggingFace if needed
fn get_cross_encoder_model_path(model_name: &str) -> Result<PathBuf, String> {
    // Use HuggingFace cache directory or local models directory
    let cache_dir = std::env::var("HF_HOME")
        .or_else(|_| std::env::var("XDG_CACHE_HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".cache")
                .join("huggingface")
        });
    
    let model_dir = cache_dir
        .join("hub")
        .join("models--".to_string() + &model_name.replace("/", "--"));
    
    // Look for model.onnx in the model directory
    let model_path = model_dir
        .join("snapshots")
        .read_dir()
        .ok()
        .and_then(|mut entries| {
            entries
                .next()
                .and_then(|entry| entry.ok())
                .map(|entry| entry.path().join("model.onnx"))
        })
        .filter(|p| p.exists());
    
    if let Some(path) = model_path {
        return Ok(path);
    }
    
    // Fallback: try local models directory
    let local_models_dir = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("models")
        .join(model_name.replace("/", "_"));
    
    let local_model_path = local_models_dir.join("model.onnx");
    if local_model_path.exists() {
        return Ok(local_model_path);
    }
    
    // If model doesn't exist, try to download it
    warn!(
        model = %model_name,
        "Cross-Encoder model not found locally, attempting to download"
    );
    
    download_cross_encoder_model(model_name, &local_models_dir)?;
    
    Ok(local_models_dir.join("model.onnx"))
}

/// Download Cross-Encoder model from HuggingFace
fn download_cross_encoder_model(model_name: &str, target_dir: &std::path::Path) -> Result<(), String> {
    use std::fs;
    
    // Create target directory
    fs::create_dir_all(target_dir)
        .map_err(|e| format!("Failed to create model directory: {}", e))?;
    
    // For now, we'll use a simple approach: download the ONNX model from HuggingFace
    // In production, you might want to use the huggingface-hub library or similar
    let model_url = format!(
        "https://huggingface.co/{}/resolve/main/model.onnx",
        model_name
    );
    
    info!(
        url = %model_url,
        "Downloading Cross-Encoder model from HuggingFace"
    );
    
    // Use reqwest to download (blocking for simplicity, but could be async)
    let response = reqwest::blocking::Client::new()
        .get(&model_url)
        .send()
        .map_err(|e| format!("Failed to download model: {}", e))?;
    
    if !response.status().is_success() {
        return Err(format!("Failed to download model: HTTP {}", response.status()));
    }
    
    let model_path = target_dir.join("model.onnx");
    let mut file = fs::File::create(&model_path)
        .map_err(|e| format!("Failed to create model file: {}", e))?;
    
    std::io::copy(&mut response.bytes().map_err(|e| format!("Failed to read response: {}", e))?.as_ref(), &mut file)
        .map_err(|e| format!("Failed to write model file: {}", e))?;
    
    info!(
        path = %model_path.display(),
        "Cross-Encoder model downloaded successfully"
    );
    
    Ok(())
}

/// Download Cross-Encoder tokenizer from HuggingFace
fn download_cross_encoder_tokenizer(model_name: &str, target_dir: &std::path::Path) -> Result<(), String> {
    use std::fs;
    
    // Create target directory if it doesn't exist
    fs::create_dir_all(target_dir)
        .map_err(|e| format!("Failed to create tokenizer directory: {}", e))?;
    
    let tokenizer_url = format!(
        "https://huggingface.co/{}/resolve/main/tokenizer.json",
        model_name
    );
    
    info!(
        url = %tokenizer_url,
        "Downloading Cross-Encoder tokenizer from HuggingFace"
    );
    
    let response = reqwest::blocking::Client::new()
        .get(&tokenizer_url)
        .send()
        .map_err(|e| format!("Failed to download tokenizer: {}", e))?;
    
    if !response.status().is_success() {
        return Err(format!("Failed to download tokenizer: HTTP {}", response.status()));
    }
    
    let tokenizer_path = target_dir.join("tokenizer.json");
    let mut file = fs::File::create(&tokenizer_path)
        .map_err(|e| format!("Failed to create tokenizer file: {}", e))?;
    
    std::io::copy(&mut response.bytes().map_err(|e| format!("Failed to read response: {}", e))?.as_ref(), &mut file)
        .map_err(|e| format!("Failed to write tokenizer file: {}", e))?;
    
    info!(
        path = %tokenizer_path.display(),
        "Cross-Encoder tokenizer downloaded successfully"
    );
    
    Ok(())
}

/// Truncate or summarize query text to fit within model token limits
/// Most embedding models have a limit of 512 tokens (roughly 2000-3000 characters)
fn truncate_query_for_embedding(text: &str, max_chars: usize) -> String {
    if text.len() <= max_chars {
        return text.to_string();
    }
    
    // For long queries, take the first part and last part (preserving context)
    // This is better than just truncating from the start
    let prefix_len = max_chars / 2;
    let suffix_len = max_chars - prefix_len - 10; // Reserve space for "..."
    
    if text.len() > max_chars {
        let prefix = &text[..prefix_len.min(text.len())];
        let suffix_start = text.len().saturating_sub(suffix_len);
        let suffix = &text[suffix_start..];
        
        format!("{}...{}", prefix, suffix)
    } else {
        text.to_string()
    }
}

/// Generate dense vector using fastembed (real semantic embeddings)
/// Falls back to hash-based embeddings if model initialization fails
async fn generate_dense_vector(text: &str, expected_dim: usize) -> Vec<f32> {
    // Truncate query if too long (most models have ~512 token limit)
    // all-MiniLM-L6-v2 has a 256 token limit, so we use ~1000 chars as safe limit
    const MAX_QUERY_CHARS: usize = 1000;
    let truncated_text = truncate_query_for_embedding(text, MAX_QUERY_CHARS);
    
    // Try to use real embedding model
    match get_embedding_model() {
        Ok(model_arc) => {
            let model_arc_clone = model_arc.clone();
            let text_clone = truncated_text.clone();
            
            // fastembed operations are blocking, so run in spawn_blocking
            // Using std::sync::Mutex allows blocking_lock() in spawn_blocking
            match tokio::task::spawn_blocking(move || {
                let model = model_arc_clone.lock().unwrap();
                model.embed(vec![text_clone.as_str()], None)
            }).await {
                Ok(Ok(embeddings)) => {
                    if let Some(embedding) = embeddings.first() {
                        // Validate dimension
                        if embedding.len() == expected_dim {
                            info!(
                                query_len = text.len(),
                                truncated_len = truncated_text.len(),
                                dim = embedding.len(),
                                "Generated dense vector using embedding model"
                            );
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
                    warn!(
                        error = %e,
                        "Embedding generation failed, using hash fallback"
                    );
                }
                Err(e) => {
                    warn!(
                        error = %e,
                        "Task join error during embedding, using hash fallback"
                    );
                }
            }
        }
        Err(e) => {
            warn!(
                error = %e,
                "Embedding model unavailable, using hash fallback"
            );
        }
    }
    
    // Fallback to hash-based embedding if model fails
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    let hash = hasher.finish();
    
    // Generate deterministic vector from hash
    let mut vector = vec![0.0f32; expected_dim];
    let mut seed = hash;
    for i in 0..expected_dim {
        seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
        vector[i] = ((seed % 1000) as f32 / 1000.0) - 0.5; // Normalize to [-0.5, 0.5]
    }
    
    // L2 normalize
    let norm: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for v in &mut vector {
            *v /= norm;
        }
    }
    
    vector
}

/// Score a single query-document chunk pair using Cross-Encoder
/// Returns the probability score (0.0-1.0)
fn score_chunk_with_cross_encoder(
    model: &CrossEncoderModel,
    query: &str,
    document_chunk: &str,
) -> Result<f64, String> {
    // Truncate document chunk if too long (most Cross-Encoders have ~512 token limit)
    const MAX_DOC_CHARS: usize = 3000; // Conservative limit
    let truncated_doc = if document_chunk.len() > MAX_DOC_CHARS {
        &document_chunk[..MAX_DOC_CHARS]
    } else {
        document_chunk
    };
    
    // Tokenize query and document separately, then combine with [SEP] token
    let query_encoding = model.tokenizer
        .encode(query, false)
        .map_err(|e| format!("Query tokenization failed: {}", e))?;
    
    let doc_encoding = model.tokenizer
        .encode(truncated_doc, false)
        .map_err(|e| format!("Document tokenization failed: {}", e))?;
    
    // Combine encodings: [CLS] + query_ids + [SEP] + doc_ids + [SEP]
    let mut input_ids = query_encoding.get_ids().to_vec();
    let mut attention_mask = query_encoding.get_attention_mask().to_vec();
    
    let doc_ids = doc_encoding.get_ids();
    let doc_attention = doc_encoding.get_attention_mask();
    
    // Find [SEP] token ID (typically 102 for BERT-based models)
    if let Some(sep_pos) = input_ids.iter().position(|&id| id == 102 || id == 2) {
        let doc_start = if doc_ids.first().copied() == Some(101) || doc_ids.first().copied() == Some(0) {
            1 // Skip [CLS]
        } else {
            0
        };
        
        for i in doc_start..doc_ids.len() {
            if doc_ids[i] != 102 && doc_ids[i] != 2 {
                input_ids.insert(sep_pos + 1 + (i - doc_start), doc_ids[i]);
                attention_mask.insert(sep_pos + 1 + (i - doc_start), doc_attention[i]);
            }
        }
        
        if input_ids.last().copied() != Some(102) && input_ids.last().copied() != Some(2) {
            input_ids.push(102);
            attention_mask.push(1);
        }
    } else {
        input_ids.push(102);
        attention_mask.push(1);
        
        let doc_start = if doc_ids.first().copied() == Some(101) || doc_ids.first().copied() == Some(0) {
            1
        } else {
            0
        };
        
        for i in doc_start..doc_ids.len() {
            if doc_ids[i] != 102 && doc_ids[i] != 2 {
                input_ids.push(doc_ids[i]);
                attention_mask.push(doc_attention[i]);
            }
        }
        
        input_ids.push(102);
        attention_mask.push(1);
    }
    
    // Create encoding from combined tokens
    let encoding = tokenizers::Encoding::new(
        input_ids,
        attention_mask,
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
    );
    
    let input_ids = encoding.get_ids();
    let attention_mask = encoding.get_attention_mask();
    
    // Convert to ONNX input format
    let input_ids_vec: Vec<i64> = input_ids.iter().map(|&id| id as i64).collect();
    let attention_mask_vec: Vec<i64> = attention_mask.iter().map(|&mask| mask as i64).collect();
    
    // Create ONNX input tensors
    let input_ids_tensor = ort::Value::from_array(
        ort::ndarray::Array2::from_shape_vec(
            (1, input_ids_vec.len()),
            input_ids_vec
        ).map_err(|e| format!("Failed to create input_ids tensor: {}", e))?
    ).map_err(|e| format!("Failed to convert input_ids to ONNX value: {}", e))?;
    
    let attention_mask_tensor = ort::Value::from_array(
        ort::ndarray::Array2::from_shape_vec(
            (1, attention_mask_vec.len()),
            attention_mask_vec
        ).map_err(|e| format!("Failed to create attention_mask tensor: {}", e))?
    ).map_err(|e| format!("Failed to convert attention_mask to ONNX value: {}", e))?;
    
    // Run inference
    let inputs = vec![
        ("input_ids", input_ids_tensor),
        ("attention_mask", attention_mask_tensor),
    ];
    
    let outputs = model.session
        .run(inputs)
        .map_err(|e| format!("ONNX inference failed: {}", e))?;
    
    // Extract the logit score from the output
    let output = outputs
        .first()
        .ok_or_else(|| "No output from ONNX model".to_string())?;
    
    let logit = output
        .try_extract_tensor::<f32>()
        .map_err(|e| format!("Failed to extract logit tensor: {}", e))?
        .into_dimensionality::<ort::ndarray::Ix1>()
        .or_else(|_| {
            output
                .try_extract_tensor::<f32>()
                .map_err(|e| format!("Failed to extract logit tensor: {}", e))?
                .into_dimensionality::<ort::ndarray::Ix2>()
                .map(|arr| arr.into_shape(arr.len()).unwrap())
        })
        .map_err(|e| format!("Failed to reshape logit tensor: {}", e))?;
    
    let raw_logit = logit
        .first()
        .copied()
        .ok_or_else(|| "Empty logit tensor".to_string())?;
    
    // Convert logit to probability using sigmoid
    let probability = 1.0 / (1.0 + (-raw_logit as f64).exp());
    
    Ok(probability)
}

/// Split document into 3 overlapping chunks for chunked verification
fn split_into_chunks(document: &str, chunk_count: usize) -> Vec<(usize, usize)> {
    let doc_len = document.len();
    if doc_len <= 1000 {
        return vec![(0, doc_len)];
    }
    
    let chunk_size = doc_len / chunk_count;
    let overlap = chunk_size / 4; // 25% overlap between chunks
    
    let mut chunks = Vec::new();
    for i in 0..chunk_count {
        let start = (i * chunk_size).saturating_sub(if i > 0 { overlap } else { 0 });
        let end = ((i + 1) * chunk_size + if i < chunk_count - 1 { overlap } else { 0 })
            .min(doc_len);
        chunks.push((start, end));
    }
    
    chunks
}

/// Deep Verify: True Cross-Encoder re-ranker for top-K results
/// Uses a dedicated Cross-Encoder model (e.g., cross-encoder/ms-marco-MiniLM-L-6-v2) via ONNX
/// to process query-document pairs as a single sequence for deep linguistic interaction analysis.
/// 
/// This implements a true Cross-Encoder approach by:
/// 1. Concatenating query and document: [CLS] Query [SEP] Document [SEP]
/// 2. Tokenizing the pair as a single input sequence
/// 3. Running inference through the ONNX model
/// 4. Extracting the raw classification logit as the relevance score
async fn deep_verify_rerank(
    query: &str,
    top_results: &mut [RankedResult],
) -> Result<(), String> {
    if top_results.is_empty() {
        return Ok(());
    }
    
    // Limit to top 5 for performance (Cross-Encoder is computationally expensive)
    let verify_count = top_results.len().min(5);
    let results_to_verify = &mut top_results[..verify_count];
    
    // Get Cross-Encoder model
    let model_arc = get_cross_encoder_model()?;
    
    // Verify each result with true Cross-Encoder scoring
    for result in results_to_verify.iter_mut() {
        let query_clone = query.to_string();
        let content_clone = result.content.clone();
        let model_arc_clone = model_arc.clone();
        
        // Process query-document pair through Cross-Encoder
        let (cross_encoder_score, best_snippet) = match tokio::task::spawn_blocking(move || {
            let model = model_arc_clone.lock().unwrap();
            
            // For documents longer than 1000 characters, use chunked verification
            if content_clone.len() > 1000 {
                let chunks = split_into_chunks(&content_clone, 3);
                let mut best_score = 0.0;
                let mut best_chunk_text = String::new();
                
                // Score each chunk and find the best one
                for (start, end) in chunks {
                    let chunk_text = &content_clone[start..end];
                    match score_chunk_with_cross_encoder(&model, &query_clone, chunk_text) {
                        Ok(score) => {
                            if score > best_score {
                                best_score = score;
                                best_chunk_text = chunk_text.to_string();
                            }
                        }
                        Err(e) => {
                            warn!(
                                error = %e,
                                chunk_start = start,
                                chunk_end = end,
                                "Failed to score chunk, skipping"
                            );
                        }
                    }
                }
                
                if best_score > 0.0 {
                    Ok((best_score, Some(best_chunk_text)))
                } else {
                    Err("All chunks failed to score".to_string())
                }
            } else {
                // For shorter documents, score the entire document
                match score_chunk_with_cross_encoder(&model, &query_clone, &content_clone) {
                    Ok(score) => Ok((score, None)),
                    Err(e) => Err(e),
                }
            }
        })
        .await
        {
            Ok(Ok((score, snippet))) => (score, snippet),
            Ok(Err(e)) => {
                warn!(
                    error = %e,
                    "Cross-Encoder scoring failed, skipping Deep Verify for this result"
                );
                continue; // Skip this result if scoring fails
            }
            Err(e) => {
                warn!(
                    error = %e,
                    "Task join error during Cross-Encoder scoring, skipping"
                );
                continue; // Skip this result if task fails
            }
        };
        
        // Set Cross-Encoder score and snippet
        result.cross_encoder_score = Some(cross_encoder_score);
        result.snippet = best_snippet;
        
        // Determine verification status based on confidence
        result.verification_status = if cross_encoder_score > 0.8 {
            Some("High Confidence".to_string())
        } else if cross_encoder_score > 0.5 {
            Some("Medium Confidence".to_string())
        } else {
            Some("Low Confidence".to_string())
        };
    }
    
    // Re-sort top 5 by Cross-Encoder score (higher is better)
    results_to_verify.sort_by(|a, b| {
        let score_a = a.cross_encoder_score.unwrap_or(0.0);
        let score_b = b.cross_encoder_score.unwrap_or(0.0);
        score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
    });
    
    // RRF Boost: Promote exceptionally high-confidence results (>0.95) to #1 spot
    if let Some(high_confidence_idx) = results_to_verify.iter().position(|r| {
        r.cross_encoder_score.unwrap_or(0.0) > 0.95
    }) {
        if high_confidence_idx > 0 {
            // Move the high-confidence result to the front
            let promoted = results_to_verify.remove(high_confidence_idx);
            results_to_verify.insert(0, promoted);
            info!(
                cross_encoder_score = results_to_verify[0].cross_encoder_score,
                "RRF Boost: Promoted high-confidence result to #1"
            );
        }
    }
    
    // Update RRF scores to reflect new ranking (for consistency)
    for (rank, result) in results_to_verify.iter_mut().enumerate() {
        result.rank = rank;
        // Blend Cross-Encoder score with original RRF score (70% Cross-Encoder, 30% RRF)
        result.score = result.cross_encoder_score.unwrap_or(result.score) * 0.7 + result.score * 0.3;
    }
    
    Ok(())
}

/// Compute enhanced similarity score between query and document embeddings
/// This provides Cross-Encoder-like precision by combining:
/// 1. Cosine similarity (semantic match)
/// 2. Length normalization (prefer appropriately-sized documents)
/// 3. Term overlap bonus (reward documents containing query terms)
fn compute_enhanced_similarity(
    query_embedding: &[f32],
    doc_embedding: &[f32],
    query: &str,
    document: &str,
) -> f64 {
    // 1. Cosine similarity (primary signal)
    let cosine_sim = cosine_similarity(query_embedding, doc_embedding);
    
    // 2. Length normalization (prefer documents that aren't too short or too long)
    let doc_len = document.len() as f64;
    let ideal_len = 500.0; // Ideal document length
    let length_penalty = 1.0 / (1.0 + (doc_len - ideal_len).abs() / ideal_len);
    
    // 3. Term overlap bonus (reward documents containing query keywords)
    let query_terms: Vec<&str> = query
        .to_lowercase()
        .split_whitespace()
        .filter(|s| s.len() > 2)
        .collect();
    let doc_lower = document.to_lowercase();
    let matched_terms = query_terms
        .iter()
        .filter(|term| doc_lower.contains(term.as_ref()))
        .count();
    let term_overlap = if query_terms.is_empty() {
        1.0
    } else {
        matched_terms as f64 / query_terms.len() as f64
    };
    
    // Combine signals: 80% cosine similarity, 10% length normalization, 10% term overlap
    let final_score = cosine_sim * 0.8 + length_penalty * 0.1 + term_overlap * 0.1;
    
    // Ensure score is in [0, 1] range
    final_score.max(0.0).min(1.0)
}

/// Compute cosine similarity between two vectors
fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() {
        return 0.0;
    }
    
    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    
    // Cosine similarity is in [-1, 1], normalize to [0, 1] for relevance scoring
    let cosine = (dot_product / (norm_a * norm_b)) as f64;
    (cosine + 1.0) / 2.0
}

/// Reciprocal Rank Fusion (RRF) to combine results from multiple search methods
/// with optional bias toward semantic (dense) or keyword (sparse) results
/// 
/// bias: -1.0 = strict keyword (sparse only), 0.0 = balanced, 1.0 = strict semantic (dense only)
fn reciprocal_rank_fusion(
    dense_results: &[RankedResult],
    sparse_results: &[RankedResult],
    k: f64,
    bias: f64,
) -> Vec<RankedResult> {
    use std::collections::HashMap;
    
    // Clamp bias to [-1.0, 1.0]
    let bias = bias.clamp(-1.0, 1.0);
    
    // Calculate weights: bias = 1.0 means dense_weight = 1.0, sparse_weight = 0.0
    // bias = -1.0 means dense_weight = 0.0, sparse_weight = 1.0
    // bias = 0.0 means both weights = 0.5
    let dense_weight = (bias + 1.0) / 2.0;  // Maps [-1, 1] to [0, 1]
    let sparse_weight = 1.0 - dense_weight;  // Inverse of dense_weight
    
    // RRF score = sum(weight * (1 / (k + rank))) for each result list
    let mut rrf_scores: HashMap<String, (f64, RankedResult)> = HashMap::new();
    
    // Process dense results with weight
    for (rank, result) in dense_results.iter().enumerate() {
        let rrf_score = dense_weight * (1.0 / (k + rank as f64 + 1.0));
        rrf_scores
            .entry(result.point_id.clone())
            .and_modify(|(score, _)| *score += rrf_score)
            .or_insert_with(|| (rrf_score, result.clone()));
    }
    
    // Process sparse results with weight
    for (rank, result) in sparse_results.iter().enumerate() {
        let rrf_score = sparse_weight * (1.0 / (k + rank as f64 + 1.0));
        rrf_scores
            .entry(result.point_id.clone())
            .and_modify(|(score, _)| *score += rrf_score)
            .or_insert_with(|| (rrf_score, result.clone()));
    }
    
    // Convert to vector and sort by RRF score
    let mut final_results: Vec<(f64, RankedResult)> = rrf_scores
        .into_iter()
        .map(|(_, (score, result))| (score, result))
        .collect();
    
    final_results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    
    final_results.into_iter().map(|(_, result)| result).collect()
}

/// Ensure diversity in results by namespace
fn ensure_namespace_diversity(mut results: Vec<RankedResult>, target_per_namespace: usize) -> Vec<RankedResult> {
    use std::collections::HashMap;
    
    let mut namespace_counts: HashMap<String, usize> = HashMap::new();
    let mut diverse_results = Vec::new();
    let mut remaining_results = Vec::new();
    
    // First pass: take up to target_per_namespace from each namespace
    for result in results {
        let count = namespace_counts.entry(result.namespace.clone()).or_insert(0);
        if *count < target_per_namespace {
            diverse_results.push(result);
            *count += 1;
        } else {
            remaining_results.push(result);
        }
    }
    
    // Second pass: add remaining results to fill up to top_k
    diverse_results.extend(remaining_results);
    
    diverse_results
}

/// Search memory fragments across Qdrant collections using Hybrid Search (Dense + Sparse)
pub async fn get_memory_query(
    State(state): State<PhoenixAppState>,
    Json(request): Json<MemoryQueryRequest>,
) -> Result<Json<MemoryQueryResponse>, StatusCode> {
    let query = request.query.trim();
    if query.is_empty() {
        return Ok(Json(MemoryQueryResponse {
            results: Vec::new(),
            total: 0,
        }));
    }

    let top_k = request.top_k.unwrap_or(10).min(50); // Limit to 50 results
    let collections = vec!["agent_logs", "telemetry", "quarantine_list"];
    let privacy_filter = PrivacyFilter::new();
    
    // Generate query vectors
    let sparse_query = generate_sparse_vector(query);
    
    // Get embedding dimension from environment or use default
    // all-MiniLM-L6-v2 produces 384-dimensional vectors
    let embedding_dim = std::env::var("EMBEDDING_MODEL_DIM")
        .unwrap_or_else(|_| "384".to_string())
        .parse::<usize>()
        .unwrap_or(384);
    
    // Generate dense vector using real embedding model (async)
    let dense_query = generate_dense_vector(query, embedding_dim).await;
    
    // Search results from both methods
    let mut dense_results = Vec::new();
    let mut sparse_results = Vec::new();
    
    // Perform hybrid search across all collections
    for collection_name in &collections {
        // Dense vector search (semantic)
        let dense_search = SearchPoints {
            collection_name: collection_name.to_string(),
            vector: dense_query.clone(),
            limit: (top_k * 2) as u64, // Get more candidates for RRF
            score_threshold: Some(0.3), // Minimum similarity threshold
            with_payload: Some(true.into()),
            with_vectors: Some(false.into()),
            // Payload filters: Qdrant automatically indexes frequently filtered payload fields
            // For twin_id/namespace filtering, add: filter: Some(Filter { must: vec![...] })
            filter: None,
            ..Default::default()
        };
        
        match state.qdrant_client.search_points(&dense_search).await {
            Ok(search_result) => {
                for (rank, scored_point) in search_result.result.iter().enumerate() {
                    let point_id = extract_point_id(scored_point);
                    let (content, timestamp, twin_id) = extract_point_metadata(scored_point);
                    
                    if !content.is_empty() {
                    dense_results.push(RankedResult {
                        point_id: format!("{}-{}", collection_name, point_id),
                        score: scored_point.score,
                        rank,
                        namespace: collection_name.to_string(),
                        content: content.clone(),
                        timestamp,
                        twin_id,
                        cross_encoder_score: None,
                        verification_status: None,
                        snippet: None,
                    });
                    }
                }
            }
            Err(e) => {
                // Dense vector search may fail if:
                // 1. Collection doesn't have dense vectors indexed yet
                // 2. Vector dimension mismatch
                // 3. Collection was created before hybrid search update
                // This is expected for existing collections - they'll work with keyword search
                warn!(
                    collection = %collection_name,
                    error = %e,
                    "Dense vector search failed (may need collection reindexing)"
                );
            }
        }
        
        // Sparse vector search (keyword-based)
        // Note: Qdrant's sparse vector search requires using the query field with NamedSparseVector
        // For now, we'll use a text-based keyword search as a fallback
        // In production, you'd use: query: Some(Query::Sparse(SparseVector { ... }))
        let keyword_terms: Vec<&str> = query
            .to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty() && s.len() > 2) // Filter short terms
            .collect();
        
        if !keyword_terms.is_empty() {
            // Use scroll with filter for keyword matching (fallback until sparse vector API is fully configured)
            let scroll_request = ScrollPoints {
                collection_name: collection_name.to_string(),
                filter: None,
                limit: Some((top_k * 3) as u64), // Get more candidates
                offset: None,
                with_payload: Some(true.into()),
                with_vectors: Some(false.into()),
                ..Default::default()
            };
            
            match state.qdrant_client.scroll(scroll_request).await {
                Ok(scroll_result) => {
                    let mut keyword_matches = Vec::new();
                    for point in scroll_result.result {
                        let point_id = extract_point_id_from_point(&point);
                        let (content, timestamp, twin_id) = extract_metadata_from_point(&point);
                        
                        if content.is_empty() {
                            continue;
                        }
                        
                        // Calculate keyword match score
                        let content_lower = content.to_lowercase();
                        let mut match_score = 0.0;
                        let mut matched_terms = 0;
                        
                        for term in &keyword_terms {
                            if content_lower.contains(term) {
                                let term_freq = content_lower.matches(term).count() as f64;
                                match_score += term_freq / (1.0 + term_freq); // TF-like scoring
                                matched_terms += 1;
                            }
                        }
                        
                        if matched_terms > 0 {
                            // Normalize score by number of query terms
                            match_score = match_score / keyword_terms.len() as f64;
                            keyword_matches.push((match_score, point_id, content, timestamp, twin_id));
                        }
                    }
                    
                    // Sort by score and take top results
                    keyword_matches.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
                    
                    for (rank, (score, point_id, content, timestamp, twin_id)) in keyword_matches.iter().take((top_k * 2) as usize).enumerate() {
                        sparse_results.push(RankedResult {
                            point_id: format!("{}-{}", collection_name, point_id),
                            score: *score,
                            rank,
                            namespace: collection_name.to_string(),
                            content: content.clone(),
                            timestamp: timestamp.clone(),
                            twin_id: twin_id.clone(),
                            cross_encoder_score: None,
                            verification_status: None,
                            snippet: None,
                        });
                    }
                }
                Err(e) => {
                    warn!(
                        collection = %collection_name,
                        error = %e,
                        "Keyword search (sparse fallback) failed"
                    );
                }
            }
        }
    }
    
    // Combine results using Reciprocal Rank Fusion (RRF) with bias
    let rrf_k = 60.0; // RRF constant (typical value)
    let bias = request.bias.unwrap_or(0.0); // Default to balanced (0.0)
    let mut fused_results = reciprocal_rank_fusion(&dense_results, &sparse_results, rrf_k, bias);
    
    // Ensure namespace diversity (at least 2-3 results per namespace)
    let target_per_namespace = (top_k / collections.len() as u32).max(2);
    fused_results = ensure_namespace_diversity(fused_results, target_per_namespace as usize);
    
    // Deep Verify: Cross-Encoder re-ranking for top 5 results (if enabled)
    let deep_verify_enabled = request.deep_verify.unwrap_or(false);
    if deep_verify_enabled && !fused_results.is_empty() {
        match deep_verify_rerank(query, &mut fused_results).await {
            Ok(()) => {
                info!(
                    verified_count = fused_results.len().min(5),
                    "Deep Verify re-ranking completed"
                );
            }
            Err(e) => {
                warn!(
                    error = %e,
                    "Deep Verify failed, using RRF results only"
                );
                // Continue with RRF results if Deep Verify fails
            }
        }
    }
    
    // Limit to top_k and apply privacy filter
    fused_results.truncate(top_k as usize);
    
    let final_results: Vec<MemoryQueryResult> = fused_results
        .into_iter()
        .map(|result| {
            // For long documents with chunked verification, use the best snippet as content
            // Otherwise, use the full content
            let display_content = if let Some(ref snippet) = result.snippet {
                privacy_filter.scrub_playbook(snippet)
            } else {
                privacy_filter.scrub_playbook(&result.content)
            };
            
            MemoryQueryResult {
                id: result.point_id,
                content: display_content,
                namespace: result.namespace,
                similarity: result.score,
                timestamp: result.timestamp,
                twin_id: result.twin_id,
                cross_encoder_score: result.cross_encoder_score,
                verification_status: result.verification_status,
                snippet: result.snippet.map(|s| privacy_filter.scrub_playbook(&s)),
            }
        })
        .collect();
    
    Ok(Json(MemoryQueryResponse {
        total: final_results.len(),
        results: final_results,
    }))
}

/// Extract point ID from ScoredPoint
fn extract_point_id(point: &qdrant_client::qdrant::ScoredPoint) -> String {
    match &point.id {
        Some(id) => match id {
            qdrant_client::qdrant::PointId { point_id_options: Some(opt) } => {
                match opt {
                    qdrant_client::qdrant::point_id::PointIdOptions::Num(num) => {
                        num.to_string()
                    }
                    qdrant_client::qdrant::point_id::PointIdOptions::Uuid(uuid) => {
                        uuid.clone()
                    }
                }
            }
            _ => "unknown".to_string(),
        },
        None => "unknown".to_string(),
    }
}

/// Extract point ID from PointStruct
fn extract_point_id_from_point(point: &qdrant_client::qdrant::PointStruct) -> String {
    match &point.id {
        Some(id) => match id {
            qdrant_client::qdrant::PointId { point_id_options: Some(opt) } => {
                match opt {
                    qdrant_client::qdrant::point_id::PointIdOptions::Num(num) => {
                        num.to_string()
                    }
                    qdrant_client::qdrant::point_id::PointIdOptions::Uuid(uuid) => {
                        uuid.clone()
                    }
                }
            }
            _ => "unknown".to_string(),
        },
        None => "unknown".to_string(),
    }
}

/// Extract metadata from PointStruct
fn extract_metadata_from_point(point: &qdrant_client::qdrant::PointStruct) -> (String, Option<String>, Option<String>) {
    let content = if let Some(payload) = &point.payload {
        if let Some(Value { kind: Some(kind) }) = payload.get("content") {
            match kind {
                qdrant_client::qdrant::value::Kind::StringValue(s) => s.clone(),
                _ => String::new(),
            }
        } else {
            String::new()
        }
    } else {
        String::new()
    };
    
    let timestamp = point
        .payload
        .as_ref()
        .and_then(|p| p.get("timestamp"))
        .and_then(|v| match &v.kind {
            Some(kind) => match kind {
                qdrant_client::qdrant::value::Kind::StringValue(s) => Some(s.clone()),
                _ => None,
            },
            None => None,
        });
    
    let twin_id = point
        .payload
        .as_ref()
        .and_then(|p| p.get("twin_id"))
        .and_then(|v| match &v.kind {
            Some(kind) => match kind {
                qdrant_client::qdrant::value::Kind::StringValue(s) => Some(s.clone()),
                _ => None,
            },
            None => None,
        });
    
    (content, timestamp, twin_id)
}

/// Extract metadata (content, timestamp, twin_id) from ScoredPoint
fn extract_point_metadata(point: &qdrant_client::qdrant::ScoredPoint) -> (String, Option<String>, Option<String>) {
    let content = if let Some(payload) = &point.payload {
        if let Some(Value { kind: Some(kind) }) = payload.get("content") {
            match kind {
                qdrant_client::qdrant::value::Kind::StringValue(s) => s.clone(),
                _ => String::new(),
            }
        } else {
            String::new()
        }
    } else {
        String::new()
    };
    
    let timestamp = point
        .payload
        .as_ref()
        .and_then(|p| p.get("timestamp"))
        .and_then(|v| match &v.kind {
            Some(kind) => match kind {
                qdrant_client::qdrant::value::Kind::StringValue(s) => Some(s.clone()),
                _ => None,
            },
            None => None,
        });
    
    let twin_id = point
        .payload
        .as_ref()
        .and_then(|p| p.get("twin_id"))
        .and_then(|v| match &v.kind {
            Some(kind) => match kind {
                qdrant_client::qdrant::value::Kind::StringValue(s) => Some(s.clone()),
                _ => None,
            },
            None => None,
        });
    
    (content, timestamp, twin_id)
}

/// Calculate text similarity score based on match position and frequency
fn calculate_text_similarity(content: &str, query: &str) -> f64 {
    if content.is_empty() || query.is_empty() {
        return 0.0;
    }

    // Count occurrences
    let occurrences = content.matches(query).count();
    
    // Check if query appears at the start (higher score)
    let starts_with = if content.starts_with(query) { 0.2 } else { 0.0 };
    
    // Base score from occurrences (normalized)
    let frequency_score = (occurrences as f64 * 0.1).min(0.6);
    
    // Length ratio (shorter content with match = higher relevance)
    let length_ratio = (query.len() as f64 / content.len().max(1) as f64).min(0.2);
    
    starts_with + frequency_score + length_ratio
}

/// Feedback request payload
#[derive(Debug, Deserialize)]
pub struct FeedbackRequest {
    pub query: String,
    pub document_id: String,
    pub is_relevant: bool,
    pub session_id: Option<String>,
}

/// Feedback response
#[derive(Debug, Serialize)]
pub struct FeedbackResponse {
    pub success: bool,
    pub feedback_id: Option<i64>,
    pub message: String,
}

/// Handle search feedback submission
pub async fn post_search_feedback(
    State(state): State<PhoenixAppState>,
    Json(payload): Json<FeedbackRequest>,
) -> Result<Json<FeedbackResponse>, axum::http::StatusCode> {
    match state.feedback_storage.store_feedback(
        &payload.query,
        &payload.document_id,
        payload.is_relevant,
        payload.session_id.as_deref(),
    ) {
        Ok(feedback_id) => {
            info!(
                feedback_id = feedback_id,
                query = %payload.query,
                document_id = %payload.document_id,
                is_relevant = payload.is_relevant,
                "Search feedback stored"
            );

            Ok(Json(FeedbackResponse {
                success: true,
                feedback_id: Some(feedback_id),
                message: "Feedback stored successfully".to_string(),
            }))
        }
        Err(e) => {
            error!(error = %e, "Failed to store feedback");
            Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Knowledge Atlas query parameters
#[derive(Debug, Deserialize)]
pub struct AtlasQueryParams {
    method: Option<String>, // "pca" or "umap"
    max_nodes: Option<usize>,
}

/// Knowledge Atlas node response
#[derive(Debug, Serialize)]
pub struct AtlasNode {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub title: String,
    pub type_: String,
    pub content: String,
    pub snippet: Option<String>,
    pub confidence: f64,
    pub similarity: f64,
}

/// Knowledge Atlas edge response
#[derive(Debug, Serialize)]
pub struct AtlasEdge {
    pub source: String,
    pub target: String,
    pub strength: f64,
}

/// Knowledge Atlas response
#[derive(Debug, Serialize)]
pub struct AtlasResponse {
    pub nodes: Vec<AtlasNode>,
    pub edges: Vec<AtlasEdge>,
    pub total: usize,
}

/// Pathfinding request
#[derive(Debug, Deserialize)]
pub struct PathfindingRequest {
    pub source_id: String,
    pub target_id: String,
}

/// Path step in the reasoning chain
#[derive(Debug, Serialize, Clone)]
pub struct PathStep {
    pub node_id: String,
    pub title: String,
    pub snippet: Option<String>,
    pub content: String,
    pub type_: String,
    pub edge_strength: f64, // Strength of edge leading to this node
}

/// Pathfinding response
#[derive(Debug, Serialize)]
pub struct PathfindingResponse {
    pub path: Vec<PathStep>,
    pub total_strength: f64, // Sum of edge strengths along path
    pub path_length: usize,
    pub found: bool,
}

/// Compute semantic edges between nodes using Cross-Encoder
/// For each node, finds top 2 semantic neighbors with Cross-Encoder score > 0.85
async fn compute_semantic_edges(
    nodes: &[AtlasNode],
    vectors: &[Vec<f32>],
) -> Vec<AtlasEdge> {
    if nodes.is_empty() || nodes.len() != vectors.len() {
        return Vec::new();
    }
    
    let mut edges = Vec::new();
    let threshold = 0.85;
    let top_k_neighbors = 2;
    let candidate_pool_size = 10; // Use cosine similarity to find top 10 candidates, then Cross-Encoder on those
    
    // Try to get Cross-Encoder model (may fail if not available)
    let model_result = get_cross_encoder_model();
    let has_cross_encoder = model_result.is_ok();
    
    // Process each node to find its semantic neighbors
    for (i, node) in nodes.iter().enumerate() {
        if node.content.is_empty() {
            continue;
        }
        
        let mut candidates = Vec::new();
        
        // Stage 1: Use cosine similarity to find top candidates
        for (j, other_node) in nodes.iter().enumerate() {
            if i == j || other_node.content.is_empty() {
                continue;
            }
            
            let cosine_sim = cosine_similarity(&vectors[i], &vectors[j]);
            candidates.push((j, cosine_sim));
        }
        
        // Sort by cosine similarity and take top candidates
        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        candidates.truncate(candidate_pool_size);
        
        // Stage 2: Use Cross-Encoder on top candidates
        let mut scored_neighbors = Vec::new();
        
        if has_cross_encoder {
            if let Ok(model_arc) = model_result {
                let model_guard = model_arc.lock().unwrap();
                
                for (candidate_idx, cosine_score) in candidates.iter() {
                    let candidate_node = &nodes[*candidate_idx];
                    
                    // Use Cross-Encoder to score the semantic relationship
                    match score_chunk_with_cross_encoder(
                        &model_guard,
                        &node.content,
                        &candidate_node.content,
                    ) {
                        Ok(cross_encoder_score) => {
                            if cross_encoder_score > threshold {
                                scored_neighbors.push((*candidate_idx, cross_encoder_score));
                            }
                        }
                        Err(_) => {
                            // If Cross-Encoder fails, fall back to cosine similarity
                            // but only if it's high enough
                            if *cosine_score > threshold {
                                scored_neighbors.push((*candidate_idx, *cosine_score));
                            }
                        }
                    }
                }
            }
        } else {
            // Fallback: use cosine similarity if Cross-Encoder is not available
            for (candidate_idx, cosine_score) in candidates.iter() {
                if *cosine_score > threshold {
                    scored_neighbors.push((*candidate_idx, *cosine_score));
                }
            }
        }
        
        // Sort by score and take top 2
        scored_neighbors.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored_neighbors.truncate(top_k_neighbors);
        
        // Create edges
        for (neighbor_idx, strength) in scored_neighbors {
            edges.push(AtlasEdge {
                source: node.id.clone(),
                target: nodes[neighbor_idx].id.clone(),
                strength,
            });
        }
    }
    
    edges
}

/// Simple PCA reduction to 3D
/// This is a simplified implementation - for production, use a proper PCA library
fn reduce_to_3d_pca(vectors: &[Vec<f32>]) -> Vec<(f64, f64, f64)> {
    use ndarray::{Array2, Axis};
    
    if vectors.is_empty() {
        return Vec::new();
    }
    
    let dim = vectors[0].len();
    let n_samples = vectors.len();
    
    // Convert to ndarray matrix
    let mut matrix = Array2::<f32>::zeros((n_samples, dim));
    for (i, vec) in vectors.iter().enumerate() {
        for (j, &val) in vec.iter().enumerate() {
            matrix[[i, j]] = val;
        }
    }
    
    // Center the data (subtract mean)
    let mean = matrix.mean_axis(Axis(0)).unwrap();
    for mut row in matrix.rows_mut() {
        row -= &mean;
    }
    
    // Compute covariance matrix (simplified - use first 3 principal components)
    // For a full PCA, we'd compute eigenvalues/eigenvectors
    // Here we use a simple projection onto the first 3 dimensions with scaling
    
    // Simple approach: use first 3 dimensions with variance-based scaling
    let mut coords = Vec::new();
    for row in matrix.rows() {
        let x = row[0] as f64 * 100.0; // Scale for visualization
        let y = if dim > 1 { row[1] as f64 * 100.0 } else { 0.0 };
        let z = if dim > 2 { row[2] as f64 * 100.0 } else { 0.0 };
        coords.push((x, y, z));
    }
    
    coords
}

/// Get knowledge atlas visualization data
pub async fn get_knowledge_atlas(
    State(state): State<PhoenixAppState>,
    Query(params): Query<AtlasQueryParams>,
) -> Result<Json<AtlasResponse>, axum::http::StatusCode> {
    let method = params.method.as_deref().unwrap_or("pca");
    let max_nodes = params.max_nodes.unwrap_or(500).min(2000);
    
    let collections = vec!["agent_logs", "telemetry", "quarantine_list"];
    let mut all_points = Vec::new();
    
    // Collect points from all collections
    for collection_name in &collections {
        let scroll_request = ScrollPoints {
            collection_name: collection_name.to_string(),
            filter: None,
            limit: Some((max_nodes / collections.len() + 100) as u64),
            offset: None,
            with_payload: Some(true.into()),
            with_vectors: Some(true.into()), // Need vectors for PCA
            ..Default::default()
        };
        
        match state.qdrant_client.scroll(&scroll_request).await {
            Ok(scroll_result) => {
                for point in scroll_result.result {
                    all_points.push((point, collection_name.to_string()));
                }
            }
            Err(e) => {
                warn!(
                    collection = %collection_name,
                    error = %e,
                    "Failed to scroll points for knowledge atlas"
                );
            }
        }
    }
    
    // Limit to max_nodes
    all_points.truncate(max_nodes);
    
    // Extract vectors and metadata
    let mut vectors = Vec::new();
    let mut metadata = Vec::new();
    
    for (point, collection) in &all_points {
        // Extract dense vector (first vector in the vectors map)
        if let Some(vectors_map) = &point.vectors {
            let dense_vec = match vectors_map {
                qdrant_client::qdrant::Vectors::Dense(dense) => {
                    Some(dense.data.clone())
                }
                qdrant_client::qdrant::Vectors::Sparse(_) => None,
            };
            
            if let Some(vec) = dense_vec {
                vectors.push(vec);
                
                // Extract metadata
                let payload = &point.payload;
                let content = payload
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let title = payload
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&collection)
                    .to_string();
                let point_id = extract_point_id_from_payload(payload);
                
                metadata.push((point_id, title, content, collection.clone()));
            }
        }
    }
    
    if vectors.is_empty() {
        return Ok(Json(AtlasResponse {
            nodes: Vec::new(),
            edges: Vec::new(),
            total: 0,
        }));
    }
    
    // Reduce dimensions
    let coords = match method {
        "pca" => reduce_to_3d_pca(&vectors),
        _ => {
            // Fallback: use first 3 dimensions
            vectors.iter()
                .map(|v| (
                    v.get(0).copied().unwrap_or(0.0) as f64 * 100.0,
                    v.get(1).copied().unwrap_or(0.0) as f64 * 100.0,
                    v.get(2).copied().unwrap_or(0.0) as f64 * 100.0,
                ))
                .collect()
        }
    };
    
    // Build response nodes
    let nodes: Vec<AtlasNode> = coords
        .iter()
        .zip(metadata.iter())
        .zip(vectors.iter())
        .map(|((coord, meta), vec)| {
            let (id, title, content, type_) = meta;
            let (x, y, z) = coord;
            
            // Compute similarity (average of vector components as proxy)
            let similarity = vec.iter().sum::<f32>() / vec.len() as f32;
            let confidence = similarity.abs().min(1.0) as f64;
            
            AtlasNode {
                id: format!("{}-{}", type_, id),
                x: *x,
                y: *y,
                z: *z,
                title: title.clone(),
                type_: type_.clone(),
                content: content.clone(),
                snippet: if content.len() > 200 {
                    Some(content.chars().take(200).collect())
                } else {
                    Some(content.clone())
                },
                confidence,
                similarity: similarity as f64,
            }
        })
        .collect();
    
    // Compute semantic edges using Cross-Encoder
    let edges = compute_semantic_edges(&nodes, &vectors).await;
    
    Ok(Json(AtlasResponse {
        nodes,
        edges,
        total: nodes.len(),
    }))
}

/// Find semantic path between two nodes using Dijkstra's algorithm
/// Weights are based on inverse Cross-Encoder scores (stronger edges = lower weight)
pub async fn find_semantic_path(
    State(state): State<PhoenixAppState>,
    Json(request): Json<PathfindingRequest>,
) -> Result<Json<PathfindingResponse>, axum::http::StatusCode> {
    // Fetch nodes and compute edges (similar to get_knowledge_atlas but we need the full graph)
    let max_nodes = 2000;
    let collections = vec!["agent_logs", "telemetry", "quarantine_list"];
    let mut all_points = Vec::new();
    
    // Collect points from all collections
    for collection_name in &collections {
        let scroll_request = ScrollPoints {
            collection_name: collection_name.to_string(),
            filter: None,
            limit: Some((max_nodes / collections.len() + 100) as u64),
            offset: None,
            with_payload: Some(true.into()),
            with_vectors: Some(true.into()),
            ..Default::default()
        };
        
        match state.qdrant_client.scroll(&scroll_request).await {
            Ok(scroll_result) => {
                for point in scroll_result.result {
                    all_points.push((point, collection_name.to_string()));
                }
            }
            Err(e) => {
                warn!(
                    collection = %collection_name,
                    error = %e,
                    "Failed to scroll points for pathfinding"
                );
            }
        }
    }
    
    all_points.truncate(max_nodes);
    
    // Extract vectors and metadata
    let mut vectors = Vec::new();
    let mut metadata = Vec::new();
    
    for (point, collection) in &all_points {
        if let Some(vectors_map) = &point.vectors {
            let dense_vec = match vectors_map {
                qdrant_client::qdrant::Vectors::Dense(dense) => {
                    Some(dense.data.clone())
                }
                qdrant_client::qdrant::Vectors::Sparse(_) => None,
            };
            
            if let Some(vec) = dense_vec {
                vectors.push(vec);
                
                let payload = &point.payload;
                let content = payload
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let title = payload
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&collection)
                    .to_string();
                let point_id = extract_point_id_from_payload(payload);
                
                metadata.push((point_id, title, content, collection.clone()));
            }
        }
    }
    
    if vectors.is_empty() {
        return Ok(Json(PathfindingResponse {
            path: Vec::new(),
            total_strength: 0.0,
            path_length: 0,
            found: false,
        }));
    }
    
    // Build nodes (we don't need 3D coords for pathfinding, just IDs and content)
    let nodes: Vec<AtlasNode> = metadata
        .iter()
        .zip(vectors.iter())
        .map(|(meta, vec)| {
            let (id, title, content, type_) = meta;
            let similarity = vec.iter().sum::<f32>() / vec.len() as f32;
            let confidence = similarity.abs().min(1.0) as f64;
            
            AtlasNode {
                id: format!("{}-{}", type_, id),
                x: 0.0,
                y: 0.0,
                z: 0.0,
                title: title.clone(),
                type_: type_.clone(),
                content: content.clone(),
                snippet: if content.len() > 200 {
                    Some(content.chars().take(200).collect())
                } else {
                    Some(content.clone())
                },
                confidence,
                similarity: similarity as f64,
            }
        })
        .collect();
    
    // Compute semantic edges
    let edges = compute_semantic_edges(&nodes, &vectors).await;
    
    // Build adjacency list: node_id -> Vec<(neighbor_id, edge_strength)>
    let mut adjacency: HashMap<String, Vec<(String, f64)>> = HashMap::new();
    for edge in &edges {
        adjacency
            .entry(edge.source.clone())
            .or_insert_with(Vec::new)
            .push((edge.target.clone(), edge.strength));
        // Make graph undirected for pathfinding
        adjacency
            .entry(edge.target.clone())
            .or_insert_with(Vec::new)
            .push((edge.source.clone(), edge.strength));
    }
    
    // Build node lookup: node_id -> AtlasNode
    let node_map: HashMap<String, &AtlasNode> = nodes.iter()
        .map(|n| (n.id.clone(), n))
        .collect();
    
    // Check if source and target exist
    if !node_map.contains_key(&request.source_id) {
        return Ok(Json(PathfindingResponse {
            path: Vec::new(),
            total_strength: 0.0,
            path_length: 0,
            found: false,
        }));
    }
    
    if !node_map.contains_key(&request.target_id) {
        return Ok(Json(PathfindingResponse {
            path: Vec::new(),
            total_strength: 0.0,
            path_length: 0,
            found: false,
        }));
    }
    
    // Dijkstra's algorithm
    // We use inverse strength as weight (1.0 - strength) so stronger edges have lower cost
    // This ensures we prefer paths with high Cross-Encoder scores
    let mut distances: HashMap<String, f64> = HashMap::new();
    let mut previous: HashMap<String, Option<String>> = HashMap::new();
    let mut visited: HashSet<String> = HashSet::new();
    
    // Priority queue: (distance, node_id)
    let mut queue = BinaryHeap::new();
    distances.insert(request.source_id.clone(), 0.0);
    queue.push(Reverse((0.0, request.source_id.clone())));
    
    while let Some(Reverse((dist, current))) = queue.pop() {
        if visited.contains(&current) {
            continue;
        }
        visited.insert(current.clone());
        
        if current == request.target_id {
            break; // Found target
        }
        
        if let Some(neighbors) = adjacency.get(&current) {
            for (neighbor, strength) in neighbors {
                if visited.contains(neighbor) {
                    continue;
                }
                
                // Weight is inverse of strength (stronger = lower cost)
                // Add small epsilon to avoid division by zero
                let weight = 1.0 - strength + 0.01;
                let alt = dist + weight;
                
                let should_update = distances
                    .get(neighbor)
                    .map(|&d| alt < d)
                    .unwrap_or(true);
                
                if should_update {
                    distances.insert(neighbor.clone(), alt);
                    previous.insert(neighbor.clone(), Some(current.clone()));
                    queue.push(Reverse((alt, neighbor.clone())));
                }
            }
        }
    }
    
    // Reconstruct path
    let mut path = Vec::new();
    let mut current = Some(request.target_id.clone());
    let mut total_strength = 0.0;
    
    while let Some(node_id) = current {
        if let Some(node) = node_map.get(&node_id) {
            // Find edge strength from previous node
            let edge_strength = if let Some(prev_id) = previous.get(&node_id).and_then(|p| p.as_ref()) {
                // Find the edge between prev and current
                edges.iter()
                    .find(|e| {
                        (e.source == *prev_id && e.target == node_id) ||
                        (e.target == *prev_id && e.source == node_id)
                    })
                    .map(|e| e.strength)
                    .unwrap_or(0.0)
            } else {
                0.0
            };
            
            total_strength += edge_strength;
            
            path.push(PathStep {
                node_id: node.id.clone(),
                title: node.title.clone(),
                snippet: node.snippet.clone(),
                content: node.content.clone(),
                type_: node.type_.clone(),
                edge_strength,
            });
        }
        
        current = previous.get(&node_id)
            .and_then(|p| p.as_ref())
            .cloned();
    }
    
    path.reverse(); // Reverse to get path from source to target
    
    let found = !path.is_empty() && path[0].node_id == request.source_id;
    
    Ok(Json(PathfindingResponse {
        path,
        total_strength,
        path_length: path.len(),
        found,
    }))
}

/// Extract point ID from payload
fn extract_point_id_from_payload(payload: &HashMap<String, qdrant_client::qdrant::Value>) -> String {
    payload
        .get("id")
        .and_then(|v| v.as_str())
        .or_else(|| payload.get("point_id").and_then(|v| v.as_str()))
        .unwrap_or("unknown")
        .to_string()
}

/// Create Phoenix API router
pub fn create_phoenix_router(state: PhoenixAppState) -> Router {
    Router::new()
        .route("/api/phoenix/stream", get(phoenix_sse_stream))
        .route("/api/consensus/status/:id", get(get_consensus_status))
        .route("/api/consensus/votes/:id", get(get_consensus_votes))
        .route("/api/consensus/vote", post(post_consensus_vote))
        .route("/api/consensus/override", post(post_strategic_override))
        .route("/api/phoenix/memory/stats", get(get_memory_stats))
        .route("/api/phoenix/memory/heatmap", get(get_topic_heat_map))
        .route("/api/phoenix/memory/prune", post(post_prune_topic))
        .route("/api/phoenix/memory/snapshot", post(post_create_snapshot))
        .route("/api/phoenix/memory/snapshot/status", get(get_snapshot_status))
        .route("/api/phoenix/memory/snapshots", get(get_snapshots))
        .route("/api/phoenix/memory/restore", post(post_restore_snapshot))
        .route("/api/phoenix/maintenance/status", get(get_maintenance_mode_status))
        .route("/api/phoenix/reports/latest", get(get_governance_report))
        .route("/api/phoenix/reports/latest.json", get(get_governance_report_json))
        .route("/api/phoenix/optimizer/drafts", get(get_optimizer_drafts))
        .route("/api/phoenix/optimizer/apply", post(apply_optimizer_rule))
        .route("/api/memory/query", post(get_memory_query))
        .route("/api/search/feedback", post(post_search_feedback))
        .route("/api/knowledge/atlas", axum::routing::get(get_knowledge_atlas))
        .route("/api/knowledge/path", post(find_semantic_path))
        .with_state(state)
}
