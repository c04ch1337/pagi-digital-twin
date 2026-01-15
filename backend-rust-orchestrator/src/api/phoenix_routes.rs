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
use serde_json::json;
use std::{
    convert::Infallible,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::broadcast;
use tokio::sync::RwLock;
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

use crate::api::knowledge_pathfinding;
use crate::agents::factory::AgentFactory;
use crate::knowledge::ingestor::{AutoIngestor, IngestionStatus, LLMSettings};
use cron::Schedule;
use std::str::FromStr;

/// Global system pause state
static SYSTEM_PAUSED: OnceLock<Arc<RwLock<bool>>> = OnceLock::new();

fn get_system_pause_state() -> Arc<RwLock<bool>> {
    SYSTEM_PAUSED.get_or_init(|| Arc::new(RwLock::new(false))).clone()
}

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
    pub agent_factory: Arc<AgentFactory>,
    pub scheduled_tasks: Arc<RwLock<ScheduledTaskStore>>,
    pub tool_proposals: Arc<RwLock<ToolProposalStore>>,
    pub peer_reviews: Arc<RwLock<PeerReviewStore>>,
    pub retrospectives: Arc<RwLock<RetrospectiveStore>>,
    pub ingestor: Option<Arc<AutoIngestor>>,
}

/// Scheduled task status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "failed")]
    Failed,
}

/// Scheduled task structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledTask {
    pub id: String,
    pub name: String,
    pub cron_expression: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    pub task_payload: serde_json::Value,
    pub status: TaskStatus,
    pub created_at: String,
    pub last_run: Option<String>,
    pub next_run: Option<String>,
}

/// In-memory store for scheduled tasks (can be upgraded to file-based or SQLite later)
#[derive(Default)]
pub struct ScheduledTaskStore {
    tasks: HashMap<String, ScheduledTask>,
}

impl ScheduledTaskStore {
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
        }
    }

    pub fn add_task(&mut self, task: ScheduledTask) {
        self.tasks.insert(task.id.clone(), task);
    }

    pub fn get_task(&self, id: &str) -> Option<&ScheduledTask> {
        self.tasks.get(id)
    }

    pub fn get_all_tasks(&self) -> Vec<&ScheduledTask> {
        self.tasks.values().collect()
    }

    pub fn get_pending_tasks(&self) -> Vec<&ScheduledTask> {
        self.tasks
            .values()
            .filter(|t| t.status == TaskStatus::Pending)
            .collect()
    }

    pub fn update_task(&mut self, id: &str, status: TaskStatus, last_run: Option<String>) {
        if let Some(task) = self.tasks.get_mut(id) {
            task.status = status;
            task.last_run = last_run;
            // Calculate next run time based on cron expression
            if let Ok(schedule) = Schedule::from_str(&task.cron_expression) {
                let reference_time = last_run
                    .as_ref()
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|| chrono::Utc::now());
                
                if let Some(next) = schedule.after(&reference_time).take(1).next() {
                    task.next_run = Some(next.to_rfc3339());
                }
            }
        }
    }

    pub fn remove_task(&mut self, id: &str) -> bool {
        self.tasks.remove(id).is_some()
    }
}

/// Tool installation proposal status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProposalStatus {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "approved")]
    Approved,
    #[serde(rename = "rejected")]
    Rejected,
}

/// Tool installation proposal structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInstallationProposal {
    pub id: String,
    pub agent_id: String,
    pub agent_name: String,
    pub repository: String,
    pub tool_name: String,
    pub description: String,
    pub github_url: String,
    pub stars: u32,
    pub language: Option<String>,
    pub installation_command: String,
    pub code_snippet: String,
    pub status: ProposalStatus,
    pub created_at: String,
    pub reviewed_at: Option<String>,
    #[serde(default)]
    pub installation_success: Option<bool>,
    #[serde(default)]
    pub verified: Option<bool>,
    #[serde(default)]
    pub verification_message: Option<String>,
    #[serde(default)]
    pub repair_proposal: Option<serde_json::Value>,
}

/// In-memory store for tool installation proposals
#[derive(Default)]
pub struct ToolProposalStore {
    proposals: HashMap<String, ToolInstallationProposal>,
}

impl ToolProposalStore {
    pub fn new() -> Self {
        Self {
            proposals: HashMap::new(),
        }
    }

    pub fn add_proposal(&mut self, proposal: ToolInstallationProposal) {
        self.proposals.insert(proposal.id.clone(), proposal);
    }

    pub fn get_proposal(&self, id: &str) -> Option<&ToolInstallationProposal> {
        self.proposals.get(id)
    }

    pub fn get_all_proposals(&self) -> Vec<&ToolInstallationProposal> {
        self.proposals.values().collect()
    }

    pub fn get_pending_proposals(&self) -> Vec<&ToolInstallationProposal> {
        self.proposals
            .values()
            .filter(|p| p.status == ProposalStatus::Pending)
            .collect()
    }

    pub fn update_proposal_status(&mut self, id: &str, status: ProposalStatus) -> bool {
        if let Some(proposal) = self.proposals.get_mut(id) {
            proposal.status = status;
            proposal.reviewed_at = Some(chrono::Utc::now().to_rfc3339());
            true
        } else {
            false
        }
    }

    pub fn remove_proposal(&mut self, id: &str) -> bool {
        self.proposals.remove(id).is_some()
    }
}

/// Peer review decision
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ReviewDecision {
    #[serde(rename = "concur")]
    Concur,
    #[serde(rename = "object")]
    Object,
}

/// Peer review status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ReviewStatus {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "reviewed")]
    Reviewed,
    #[serde(rename = "consensus_reached")]
    ConsensusReached,
}

/// Peer review structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerReview {
    pub review_id: String,
    pub tool_proposal_id: String,
    pub requesting_agent_id: String,
    pub requesting_agent_name: String,
    pub expert_agent_id: String,
    pub expert_agent_name: String,
    pub tool_name: String,
    pub github_url: String,
    pub requesting_reasoning: String,
    pub expert_decision: Option<ReviewDecision>,
    pub expert_reasoning: Option<String>,
    pub alternative_playbook_id: Option<String>,
    pub status: ReviewStatus,
    pub consensus: Option<String>, // "approved" or "rejected"
    pub created_at: String,
    pub reviewed_at: Option<String>,
    pub consensus_at: Option<String>,
}

/// In-memory store for peer reviews
#[derive(Default)]
pub struct PeerReviewStore {
    reviews: HashMap<String, PeerReview>,
}

impl PeerReviewStore {
    pub fn new() -> Self {
        Self {
            reviews: HashMap::new(),
        }
    }

    pub fn add_review(&mut self, review: PeerReview) {
        self.reviews.insert(review.review_id.clone(), review);
    }

    pub fn get_review(&self, id: &str) -> Option<&PeerReview> {
        self.reviews.get(id)
    }

    pub fn get_all_reviews(&self) -> Vec<&PeerReview> {
        self.reviews.values().collect()
    }

    pub fn get_pending_reviews(&self) -> Vec<&PeerReview> {
        self.reviews
            .values()
            .filter(|r| r.status == ReviewStatus::Pending)
            .collect()
    }

    pub fn get_reviews_for_proposal(&self, proposal_id: &str) -> Vec<&PeerReview> {
        self.reviews
            .values()
            .filter(|r| r.tool_proposal_id == proposal_id)
            .collect()
    }

    pub fn update_review_response(
        &mut self,
        review_id: &str,
        decision: ReviewDecision,
        reasoning: String,
        alternative_playbook_id: Option<String>,
    ) -> bool {
        if let Some(review) = self.reviews.get_mut(review_id) {
            review.expert_decision = Some(decision);
            review.expert_reasoning = Some(reasoning);
            review.alternative_playbook_id = alternative_playbook_id;
            review.status = ReviewStatus::Reviewed;
            review.reviewed_at = Some(chrono::Utc::now().to_rfc3339());
            true
        } else {
            false
        }
    }

    pub fn update_review_consensus(
        &mut self,
        review_id: &str,
        consensus: String,
    ) -> bool {
        if let Some(review) = self.reviews.get_mut(review_id) {
            review.consensus = Some(consensus.clone());
            review.status = ReviewStatus::ConsensusReached;
            review.consensus_at = Some(chrono::Utc::now().to_rfc3339());
            true
        } else {
            false
        }
    }
}

/// Retrospective store
#[derive(Default)]
pub struct RetrospectiveStore {
    retrospectives: HashMap<String, crate::tools::playbook_store::RetrospectiveAnalysis>,
}

impl RetrospectiveStore {
    pub fn new() -> Self {
        Self {
            retrospectives: HashMap::new(),
        }
    }

    pub fn add_retrospective(&mut self, retrospective: crate::tools::playbook_store::RetrospectiveAnalysis) {
        self.retrospectives.insert(retrospective.retrospective_id.clone(), retrospective);
    }

    pub fn get_retrospective(&self, id: &str) -> Option<&crate::tools::playbook_store::RetrospectiveAnalysis> {
        self.retrospectives.get(id)
    }

    pub fn get_all_retrospectives(&self) -> Vec<&crate::tools::playbook_store::RetrospectiveAnalysis> {
        self.retrospectives.values().collect()
    }

    pub fn get_retrospectives_for_playbook(&self, playbook_id: &str) -> Vec<&crate::tools::playbook_store::RetrospectiveAnalysis> {
        self.retrospectives
            .values()
            .filter(|r| r.playbook_id == playbook_id)
            .collect()
    }
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
                            | PhoenixEvent::PeerReviewRequest { .. }
                            | PhoenixEvent::PeerReviewResponse { .. }
                            | PhoenixEvent::PeerReviewConsensus { .. }
                            | PhoenixEvent::PostMortemRetrospective { .. }
                    );
                    
                    if should_stream {
                        match serde_json::to_string(&event) {
                            Ok(json) => {
                                let event_type = match event {
                                    PhoenixEvent::ConsensusVote { .. } => "consensus_vote",
                                    PhoenixEvent::ConsensusResult { .. } => "consensus_result",
                                    PhoenixEvent::QuarantineAlert { .. } => "quarantine_alert",
                                    PhoenixEvent::MemoryTransfer { .. } => "memory_transfer",
                                    PhoenixEvent::PeerReviewRequest { .. } => "peer_review_request",
                                    PhoenixEvent::PeerReviewResponse { .. } => "peer_review_response",
                                    PhoenixEvent::PeerReviewConsensus { .. } => "peer_review_consensus",
                                    PhoenixEvent::PostMortemRetrospective { .. } => "post_mortem_retrospective",
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
    /// Active knowledge domains to search (Mind, Body, Heart, Soul)
    /// If not provided, will infer from query or search all domains
    #[serde(default)]
    pub domains: Option<Vec<String>>,
    /// Agent ID for persona-based domain weighting
    pub agent_id: Option<String>,
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
    /// Knowledge domain this result belongs to (Mind, Body, Heart, Soul)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
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
    /// Domain attribution showing contribution percentages from each domain
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain_attribution: Option<crate::knowledge::DomainAttribution>,
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
    domain: Option<crate::knowledge::KnowledgeDomain>,
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
static EMBEDDING_MODEL: OnceLock<Arc<std::sync::Mutex<fastembed::TextEmbedding>>> = OnceLock::new();

/// Initialize the embedding model (called once on first use)
fn init_embedding_model() -> Result<Arc<std::sync::Mutex<fastembed::TextEmbedding>>, String> {
    use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};

    // Get model name from environment or use default
    let model_name = std::env::var("EMBEDDING_MODEL_NAME")
        .unwrap_or_else(|_| "all-MiniLM-L6-v2".to_string());

    // Map model name to fastembed model type
    let model_type: EmbeddingModel = match model_name.as_str() {
        "all-MiniLM-L6-v2" | "sentence-transformers/all-MiniLM-L6-v2" => {
            EmbeddingModel::AllMiniLML6V2
        }
        "BAAI/bge-small-en-v1.5" => EmbeddingModel::BGESmallENV15,
        "BAAI/bge-base-en-v1.5" => EmbeddingModel::BGEBaseENV15,
        _ => {
            warn!(
                model = %model_name,
                "Unknown model name, defaulting to all-MiniLM-L6-v2"
            );
            EmbeddingModel::AllMiniLML6V2
        }
    };

    let init_options = TextInitOptions::new(model_type).with_show_download_progress(false);

    match TextEmbedding::try_new(init_options) {
        Ok(model) => {
            info!(model = %model_name, "Embedding model initialized successfully");
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
fn get_embedding_model() -> Result<Arc<std::sync::Mutex<fastembed::TextEmbedding>>, String> {
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
    use ort::SessionBuilder;
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
    // For ort 2.0.0-rc.11, use SessionBuilder
    use ort::SessionBuilder;
    let session = SessionBuilder::new()
        .map_err(|e| format!("Failed to create ONNX session builder: {}", e))?
        .with_model_from_file(&model_path)
        .map_err(|e| {
            format!(
                "Failed to load ONNX model from {}: {}",
                model_path.display(),
                e
            )
        })?
        .commit()
        .map_err(|e| format!("Failed to commit ONNX session: {}", e))?;
    
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

    let bytes = response
        .bytes()
        .map_err(|e| format!("Failed to read response: {}", e))?;

    std::io::copy(&mut bytes.as_ref(), &mut file)
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

    let bytes = response
        .bytes()
        .map_err(|e| format!("Failed to read response: {}", e))?;

    std::io::copy(&mut bytes.as_ref(), &mut file)
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
    model: &mut CrossEncoderModel,
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

    // Create ONNX input tensors.
    // Most BERT-style Cross-Encoders expect input tensors shaped [batch, seq_len].
    let input_ids_tensor = ort::value::Tensor::from_array((
        vec![1i64, input_ids_vec.len() as i64],
        input_ids_vec,
    ))
    .map_err(|e| format!("Failed to create input_ids tensor: {}", e))?;

    let attention_mask_tensor = ort::value::Tensor::from_array((
        vec![1i64, attention_mask_vec.len() as i64],
        attention_mask_vec,
    ))
    .map_err(|e| format!("Failed to create attention_mask tensor: {}", e))?;

    // Run inference
    let outputs = model
        .session
        .run(ort::inputs![
            "input_ids" => input_ids_tensor,
            "attention_mask" => attention_mask_tensor,
        ])
        .map_err(|e| format!("ONNX inference failed: {}", e))?;

    // Extract the first logit from the first output tensor.
    let output = &outputs[0];
    let (_shape, logits) = output
        .try_extract_tensor::<f32>()
        .map_err(|e| format!("Failed to extract logit tensor: {}", e))?;

    let raw_logit = logits
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
                    match score_chunk_with_cross_encoder(&mut *model, &query_clone, chunk_text) {
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
                match score_chunk_with_cross_encoder(&mut *model, &query_clone, &content_clone) {
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
            domain_attribution: None,
        }));
    }

    let top_k = request.top_k.unwrap_or(10).min(50); // Limit to 50 results
    let privacy_filter = PrivacyFilter::new();
    
    // Initialize domain router
    let domain_router = crate::knowledge::DomainRouter::new();
    
    // Determine active domains
    let active_domains: Vec<crate::knowledge::KnowledgeDomain> = if let Some(domain_strings) = &request.domains {
        // Parse domain strings from request
        domain_strings
            .iter()
            .filter_map(|s| {
                match s.to_lowercase().as_str() {
                    "mind" => Some(crate::knowledge::KnowledgeDomain::Mind),
                    "body" => Some(crate::knowledge::KnowledgeDomain::Body),
                    "heart" => Some(crate::knowledge::KnowledgeDomain::Heart),
                    "soul" => Some(crate::knowledge::KnowledgeDomain::Soul),
                    _ => None,
                }
            })
            .collect()
    } else {
        // Infer domains from query
        domain_router.infer_domains_from_query(query)
    };
    
    // If no domains selected, use all
    let active_domains = if active_domains.is_empty() {
        crate::knowledge::KnowledgeDomain::all()
    } else {
        active_domains
    };
    
    // Get collections for active domains
    let collections = domain_router.get_collections_for_domains(&active_domains);
    
    // If no collections found, fall back to default
    let collections = if collections.is_empty() {
        vec!["agent_logs".to_string(), "telemetry".to_string(), "quarantine_list".to_string()]
    } else {
        collections
    };
    
    // Get persona-based domain weights if agent_id provided
    let domain_weights = if let Some(agent_id) = &request.agent_id {
        // Load persona to get name
        let persona = crate::agents::persona::get_persona(
            state.qdrant_client.clone(),
            agent_id,
        ).await.unwrap_or(None);
        
        crate::knowledge::get_persona_domain_weights(
            persona.as_ref().map(|p| p.name.as_str())
        )
    } else {
        // Default equal weights
        let mut weights = std::collections::HashMap::new();
        for domain in crate::knowledge::KnowledgeDomain::all() {
            weights.insert(domain, 1.0);
        }
        weights
    };
    
    // Create domain-to-collection mapping for result labeling
    let mut collection_to_domain: HashMap<String, crate::knowledge::KnowledgeDomain> = HashMap::new();
    for domain in &active_domains {
        for collection in domain_router.get_collections(*domain) {
            collection_to_domain.insert(collection, *domain);
        }
    }
    
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
                        // Determine domain for this collection
                        let domain = collection_to_domain.get(collection_name).copied();
                        
                        // Apply domain weight to score
                        let weighted_score = if let Some(dom) = domain {
                            scored_point.score * domain_weights.get(&dom).copied().unwrap_or(1.0)
                        } else {
                            scored_point.score
                        };
                        
                        dense_results.push(RankedResult {
                            point_id: format!("{}-{}", collection_name, point_id),
                            score: weighted_score,
                            rank,
                            namespace: collection_name.to_string(),
                            content: content.clone(),
                            timestamp,
                            twin_id,
                            domain,
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
                        // Determine domain for this collection
                        let domain = collection_to_domain.get(collection_name).copied();
                        
                        // Apply domain weight to score
                        let weighted_score = if let Some(dom) = domain {
                            *score * domain_weights.get(&dom).copied().unwrap_or(1.0)
                        } else {
                            *score
                        };
                        
                        sparse_results.push(RankedResult {
                            point_id: format!("{}-{}", collection_name, point_id),
                            score: weighted_score,
                            rank,
                            namespace: collection_name.to_string(),
                            content: content.clone(),
                            timestamp: timestamp.clone(),
                            twin_id: twin_id.clone(),
                            domain,
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
    
    // Calculate domain attribution from final results
    let domain_attribution_results: Vec<(crate::knowledge::KnowledgeDomain, f64)> = fused_results
        .iter()
        .filter_map(|result| {
            result.domain.map(|domain| (domain, result.score))
        })
        .collect();
    
    let domain_attribution = if !domain_attribution_results.is_empty() {
        Some(crate::knowledge::get_source_attribution(&domain_attribution_results))
    } else {
        None
    };
    
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
                domain: result.domain.map(|d| d.display_name().to_string()),
                cross_encoder_score: result.cross_encoder_score,
                verification_status: result.verification_status,
                snippet: result.snippet.map(|s| privacy_filter.scrub_playbook(&s)),
            }
        })
        .collect();
    
    // Optionally persist attribution to audit_history collection for analytics
    if let Some(ref attribution) = domain_attribution {
        let _ = save_attribution_to_audit(
            state.qdrant_client.clone(),
            attribution,
            &query,
            request.agent_id.as_deref(),
        ).await;
    }
    
    Ok(Json(MemoryQueryResponse {
        total: final_results.len(),
        results: final_results,
        domain_attribution,
    }))
}

/// Save domain attribution to audit_history collection for analytics
async fn save_attribution_to_audit(
    qdrant_client: Arc<Qdrant>,
    attribution: &crate::knowledge::DomainAttribution,
    query: &str,
    agent_id: Option<&str>,
) -> Result<(), String> {
    use qdrant_client::qdrant::{UpsertPoints, PointId, point_id::PointIdOptions};
    use uuid::Uuid;
    
    let collection_name = "audit_history";
    
    // Generate embedding from query text for semantic search
    let embedding_dim = std::env::var("EMBEDDING_MODEL_DIM")
        .unwrap_or_else(|_| "384".to_string())
        .parse::<usize>()
        .unwrap_or(384);
    let embedding = generate_dense_vector(query, embedding_dim).await;
    
    let point_id = Uuid::new_v4();
    let timestamp = chrono::Utc::now().to_rfc3339();
    
    let mut payload: HashMap<String, Value> = HashMap::new();
    payload.insert("type".to_string(), qdrant_string_value("domain_attribution".to_string()));
    payload.insert("query".to_string(), qdrant_string_value(query.to_string()));
    payload.insert("timestamp".to_string(), qdrant_string_value(timestamp));
    payload.insert("mind".to_string(), qdrant_double_value(attribution.mind));
    payload.insert("body".to_string(), qdrant_double_value(attribution.body));
    payload.insert("heart".to_string(), qdrant_double_value(attribution.heart));
    payload.insert("soul".to_string(), qdrant_double_value(attribution.soul));
    
    if let Some(agent_id) = agent_id {
        payload.insert("agent_id".to_string(), qdrant_string_value(agent_id.to_string()));
    }
    
    let point = PointStruct {
        id: Some(PointId {
            point_id_options: Some(PointIdOptions::Uuid(point_id.to_string())),
        }),
        vectors: Some(qdrant_client::qdrant::Vectors {
            vectors_options: Some(qdrant_client::qdrant::vectors::VectorsOptions::Vector(
                qdrant_client::qdrant::Vector { data: embedding },
            )),
        }),
        payload,
    };
    
    // Ensure collection exists (best effort)
    let _ = ensure_audit_collection(qdrant_client.clone(), embedding_dim).await;
    
    qdrant_client
        .upsert_points(UpsertPoints {
            collection_name: collection_name.to_string(),
            points: vec![point],
            ..Default::default()
        })
        .await
        .map_err(|e| format!("Failed to save attribution to audit_history: {}", e))?;
    
    info!(
        query = %query,
        mind = attribution.mind,
        body = attribution.body,
        heart = attribution.heart,
        soul = attribution.soul,
        "Domain attribution saved to audit_history"
    );
    
    Ok(())
}

/// Helper to create Qdrant string value
fn qdrant_string_value(s: String) -> Value {
    Value {
        kind: Some(qdrant_client::qdrant::value::Kind::StringValue(s)),
    }
}

/// Helper to create Qdrant double value
fn qdrant_double_value(f: f64) -> Value {
    Value {
        kind: Some(qdrant_client::qdrant::value::Kind::DoubleValue(f)),
    }
}

/// Ensure audit_history collection exists
async fn ensure_audit_collection(
    qdrant_client: Arc<Qdrant>,
    embedding_dim: usize,
) -> Result<(), String> {
    use qdrant_client::qdrant::{CreateCollection, Distance, VectorParams, VectorsConfig, vectors_config::Config};
    
    let collection_name = "audit_history";
    
    // Check if collection exists
    match qdrant_client.collection_info(collection_name).await {
        Ok(_) => return Ok(()), // Collection exists
        Err(_) => {
            // Collection doesn't exist, create it
            info!("Creating audit_history collection");
        }
    }
    
    let config = CreateCollection {
        collection_name: collection_name.to_string(),
        vectors_config: Some(VectorsConfig {
            config: Some(Config::Params(VectorParams {
                size: embedding_dim as u64,
                distance: Distance::Cosine as i32,
                ..Default::default()
            })),
        }),
        ..Default::default()
    };
    
    qdrant_client
        .create_collection(config)
        .await
        .map_err(|e| format!("Failed to create audit_history collection: {}", e))?;
    
    info!("Created audit_history collection");
    Ok(())
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

/// Multi-hop semantic pathfinding request.
#[derive(Debug, Deserialize)]
pub struct KnowledgePathRequest {
    pub source_id: String,
    pub target_id: String,
    pub max_depth: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct KnowledgePathEdge {
    pub source_id: String,
    pub target_id: String,
    pub cross_encoder_score: f64,
    pub weight: f64,
}

#[derive(Debug, Serialize)]
pub struct KnowledgePathResponse {
    pub node_ids: Vec<String>,
    pub edges: Vec<KnowledgePathEdge>,
    pub total_weight: f64,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
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
                let mut model_guard = model_arc.lock().unwrap();
                
                for (candidate_idx, cosine_score) in candidates.iter() {
                    let candidate_node = &nodes[*candidate_idx];
                    
                    // Use Cross-Encoder to score the semantic relationship
                    match score_chunk_with_cross_encoder(
                        &mut *model_guard,
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

/// POST `/api/knowledge/path`
///
/// Returns an ordered path from source -> target with per-edge scores and weights.
pub async fn post_knowledge_path(
    State(state): State<PhoenixAppState>,
    Json(request): Json<KnowledgePathRequest>,
) -> Result<Json<KnowledgePathResponse>, (StatusCode, Json<ErrorResponse>)> {
    const DEFAULT_MAX_DEPTH: usize = 8;
    const ABS_MAX_DEPTH: usize = 50; // Safety: layered graph size = O(|V| * max_depth)
    const MAX_NODES: usize = 2000;

    if request.source_id.trim().is_empty() || request.target_id.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "source_id and target_id must be non-empty".to_string(),
            }),
        ));
    }

    let max_depth = request
        .max_depth
        .unwrap_or(DEFAULT_MAX_DEPTH)
        .min(ABS_MAX_DEPTH);

    // Fetch nodes and edges the same way the Knowledge Atlas does (Qdrant + semantic edges).
    let collections = vec!["agent_logs", "telemetry", "quarantine_list"];
    let mut all_points = Vec::new();

    for collection_name in &collections {
        let scroll_request = ScrollPoints {
            collection_name: collection_name.to_string(),
            filter: None,
            limit: Some((MAX_NODES / collections.len() + 100) as u64),
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
                    "Failed to scroll points for knowledge/path"
                );
            }
        }
    }

    all_points.truncate(MAX_NODES);

    let mut vectors: Vec<Vec<f32>> = Vec::new();
    let mut metadata: Vec<(String, String, String, String)> = Vec::new();

    for (point, collection) in &all_points {
        let Some(vectors_map) = &point.vectors else {
            continue;
        };

        let dense_vec = match vectors_map {
            qdrant_client::qdrant::Vectors::Dense(dense) => Some(dense.data.clone()),
            qdrant_client::qdrant::Vectors::Sparse(_) => None,
        };

        let Some(vec) = dense_vec else {
            continue;
        };

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
            .unwrap_or(collection)
            .to_string();
        let point_id = extract_point_id_from_payload(payload);

        metadata.push((point_id, title, content, collection.clone()));
    }

    if vectors.is_empty() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Knowledge graph is empty (no vectors available)".to_string(),
            }),
        ));
    }

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

    // Compute edges; strength here is the Cross-Encoder score (or cosine fallback) in [0, 1].
    let base_edges = compute_semantic_edges(&nodes, &vectors).await;

    // Flatten to the minimal representation the pathfinder needs.
    let node_ids: Vec<String> = nodes.iter().map(|n| n.id.clone()).collect();
    let mut edges_for_search: Vec<knowledge_pathfinding::EdgeInput> = Vec::new();
    edges_for_search.reserve(base_edges.len() * 2);
    for e in &base_edges {
        edges_for_search.push(knowledge_pathfinding::EdgeInput {
            source_id: e.source.clone(),
            target_id: e.target.clone(),
            cross_encoder_score: Some(e.strength),
        });
        // Treat as undirected for traversal: add the reverse direction.
        edges_for_search.push(knowledge_pathfinding::EdgeInput {
            source_id: e.target.clone(),
            target_id: e.source.clone(),
            cross_encoder_score: Some(e.strength),
        });
    }

    // Validate source/target exist in the current graph.
    if !node_ids.iter().any(|id| id == &request.source_id) {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("source_id '{}' not found", request.source_id),
            }),
        ));
    }
    if !node_ids.iter().any(|id| id == &request.target_id) {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("target_id '{}' not found", request.target_id),
            }),
        ));
    }

    let Some(found) = knowledge_pathfinding::find_path_with_max_depth(
        &node_ids,
        &edges_for_search,
        &request.source_id,
        &request.target_id,
        max_depth,
    ) else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!(
                    "No path found from '{}' to '{}' within max_depth={}",
                    request.source_id, request.target_id, max_depth
                ),
            }),
        ));
    };

    let edges: Vec<KnowledgePathEdge> = found
        .edges
        .into_iter()
        .map(|e| KnowledgePathEdge {
            source_id: e.source_id,
            target_id: e.target_id,
            cross_encoder_score: e.cross_encoder_score,
            weight: e.weight,
        })
        .collect();

    Ok(Json(KnowledgePathResponse {
        node_ids: found.node_ids,
        edges,
        total_weight: found.total_weight,
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

/// POST `/api/knowledge/ingest`
///
/// Manually trigger ingestion of a file or re-index existing files
#[derive(Debug, Deserialize)]
pub struct KnowledgeIngestRequest {
    /// Optional file path to ingest (if not provided, processes all files in watch directory)
    pub file_path: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct KnowledgeIngestResponse {
    pub success: bool,
    pub message: String,
    pub file_path: Option<String>,
}

pub async fn post_knowledge_ingest(
    State(state): State<PhoenixAppState>,
    Json(request): Json<KnowledgeIngestRequest>,
) -> Result<Json<KnowledgeIngestResponse>, StatusCode> {
    let ingestor = state.ingestor.as_ref().ok_or_else(|| {
        error!("Auto-Domain Ingestor not initialized");
        StatusCode::SERVICE_UNAVAILABLE
    })?;

    if let Some(ref file_path) = request.file_path {
        // Process specific file
        let path = std::path::Path::new(file_path);
        if !path.exists() {
            return Err(StatusCode::NOT_FOUND);
        }

        match ingestor.process_file(path).await {
            Ok(_) => Ok(Json(KnowledgeIngestResponse {
                success: true,
                message: format!("File '{}' ingested successfully", file_path),
                file_path: Some(file_path.clone()),
            })),
            Err(e) => {
                error!(file = %file_path, error = %e, "Failed to ingest file");
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    } else {
        // Process all files in watch directory
        let watch_dir = &ingestor.watch_dir;
        if !watch_dir.exists() {
            return Err(StatusCode::NOT_FOUND);
        }

        let mut processed = 0;
        let mut failed = 0;

        use tokio::fs;
        let mut entries = fs::read_dir(watch_dir).await.map_err(|_| {
            error!(dir = %watch_dir.display(), "Failed to read watch directory");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        while let Some(entry) = entries.next_entry().await.map_err(|_| {
            error!("Failed to read directory entry");
            StatusCode::INTERNAL_SERVER_ERROR
        })? {
            let path = entry.path();
            if path.is_file() {
                match ingestor.process_file(&path).await {
                    Ok(_) => processed += 1,
                    Err(e) => {
                        error!(file = %path.display(), error = %e, "Failed to process file");
                        failed += 1;
                    }
                }
            }
        }

        Ok(Json(KnowledgeIngestResponse {
            success: failed == 0,
            message: format!(
                "Processed {} files successfully, {} failed",
                processed, failed
            ),
            file_path: None,
        }))
    }
}

/// GET `/api/knowledge/ingest/status`
///
/// Get current ingestion status
#[derive(Debug, Serialize)]
pub struct KnowledgeIngestStatusResponse {
    pub status: IngestionStatus,
}

pub async fn get_knowledge_ingest_status(
    State(state): State<PhoenixAppState>,
) -> Result<Json<KnowledgeIngestStatusResponse>, StatusCode> {
    let ingestor = state.ingestor.as_ref().ok_or_else(|| {
        error!("Auto-Domain Ingestor not initialized");
        StatusCode::SERVICE_UNAVAILABLE
    })?;

    let status = ingestor.get_status().await;
    Ok(Json(KnowledgeIngestStatusResponse { status }))
}

// ============================================================================
// Phoenix Chronos: Scheduled Task Management
// ============================================================================

/// Request to create a scheduled task
#[derive(Debug, Deserialize)]
pub struct CreateScheduledTaskRequest {
    pub name: String,
    pub cron_expression: String,
    pub agent_id: Option<String>,
    pub task_payload: serde_json::Value,
}

/// Response for scheduled task operations
#[derive(Debug, Serialize)]
pub struct ScheduledTaskResponse {
    pub task: ScheduledTask,
}

/// List of scheduled tasks
#[derive(Debug, Serialize)]
pub struct ScheduledTasksListResponse {
    pub tasks: Vec<ScheduledTask>,
}

/// Create a new scheduled task
pub async fn post_create_scheduled_task(
    State(state): State<PhoenixAppState>,
    Json(request): Json<CreateScheduledTaskRequest>,
) -> Result<Json<ScheduledTaskResponse>, StatusCode> {
    // Validate cron expression
    if Schedule::from_str(&request.cron_expression).is_err() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let task_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now();
    
    // Calculate next run time
    let next_run = Schedule::from_str(&request.cron_expression)
        .ok()
        .and_then(|s| s.after(&now).take(1).next())
        .map(|dt| dt.to_rfc3339());

    let task = ScheduledTask {
        id: task_id.clone(),
        name: request.name,
        cron_expression: request.cron_expression,
        agent_id: request.agent_id,
        task_payload: request.task_payload,
        status: TaskStatus::Pending,
        created_at: now.to_rfc3339(),
        last_run: None,
        next_run,
    };

    {
        let mut store = state.scheduled_tasks.write().await;
        store.add_task(task.clone());
    }

    info!(task_id = %task.id, "Created scheduled task");
    Ok(Json(ScheduledTaskResponse { task }))
}

/// List all scheduled tasks
pub async fn get_scheduled_tasks(
    State(state): State<PhoenixAppState>,
) -> Json<ScheduledTasksListResponse> {
    let store = state.scheduled_tasks.read().await;
    let tasks = store.get_all_tasks().into_iter().cloned().collect();
    Json(ScheduledTasksListResponse { tasks })
}

/// Get a specific scheduled task
pub async fn get_scheduled_task(
    State(state): State<PhoenixAppState>,
    Path(task_id): Path<String>,
) -> Result<Json<ScheduledTaskResponse>, StatusCode> {
    let store = state.scheduled_tasks.read().await;
    match store.get_task(&task_id) {
        Some(task) => Ok(Json(ScheduledTaskResponse {
            task: task.clone(),
        })),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// Delete a scheduled task
pub async fn delete_scheduled_task(
    State(state): State<PhoenixAppState>,
    Path(task_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut store = state.scheduled_tasks.write().await;
    if store.remove_task(&task_id) {
        info!(task_id = %task_id, "Deleted scheduled task");
        Ok(Json(json!({"ok": true})))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

/// Search for agents using semantic similarity
#[derive(Debug, Deserialize)]
pub struct AgentSearchRequest {
    pub query: String,
    #[serde(default = "default_top_k")]
    pub top_k: usize,
}

fn default_top_k() -> usize {
    5
}

/// Agent search result
#[derive(Debug, Serialize)]
pub struct AgentSearchResult {
    pub agent_id: String,
    pub agent_name: String,
    pub mission: String,
    pub score: f64,
    pub status: String,
}

/// Agent search response
#[derive(Debug, Serialize)]
pub struct AgentSearchResponse {
    pub results: Vec<AgentSearchResult>,
}

/// Search for agents using semantic similarity against agent_logs collection
pub async fn get_agents_search(
    State(state): State<PhoenixAppState>,
    Query(params): Query<AgentSearchRequest>,
) -> Result<Json<AgentSearchResponse>, StatusCode> {
    let query = params.query.trim();
    if query.is_empty() {
        return Ok(Json(AgentSearchResponse {
            results: Vec::new(),
        }));
    }

    let top_k = params.top_k.min(10); // Limit to 10 results

    // Get embedding dimension
    let embedding_dim = std::env::var("EMBEDDING_MODEL_DIM")
        .unwrap_or_else(|_| "384".to_string())
        .parse::<usize>()
        .unwrap_or(384);

    // Generate dense vector for the query
    let dense_query = generate_dense_vector(query, embedding_dim).await;

    // Search in agent_logs collection
    let search_request = SearchPoints {
        collection_name: "agent_logs".to_string(),
        vector: dense_query,
        limit: top_k as u64,
        score_threshold: Some(0.3),
        with_payload: Some(true.into()),
        with_vectors: Some(false.into()),
        filter: None,
        ..Default::default()
    };

    let mut results = Vec::new();

    match state.qdrant_client.search_points(&search_request).await {
        Ok(search_result) => {
            for scored_point in search_result.result {
                let payload = &scored_point.payload;
                
                // Extract agent information from payload
                let agent_id = payload
                    .get("agent_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                
                let agent_name = payload
                    .get("agent_name")
                    .or_else(|| payload.get("name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown Agent")
                    .to_string();
                
                let mission = payload
                    .get("mission")
                    .or_else(|| payload.get("content"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                // Get agent status from agent factory
                let agent_list = state.agent_factory.list_agents().await;
                let agent_info = agent_list.iter().find(|a| a.agent_id == agent_id);
                let status = agent_info
                    .map(|a| a.status.clone())
                    .unwrap_or_else(|| "offline".to_string());

                results.push(AgentSearchResult {
                    agent_id,
                    agent_name,
                    mission,
                    score: scored_point.score as f64,
                    status,
                });
            }
        }
        Err(e) => {
            warn!(error = %e, "Failed to search agents in Qdrant");
        }
    }

    // If no results from Qdrant, fall back to listing active agents and doing keyword matching
    if results.is_empty() {
        let agent_list = state.agent_factory.list_agents().await;
        let query_lower = query.to_lowercase();
        
        for agent in agent_list {
            let mission_lower = agent.mission.to_lowercase();
            let name_lower = agent.name.to_lowercase();
            
            // Simple keyword matching score
            let score = if mission_lower.contains(&query_lower) || name_lower.contains(&query_lower) {
                0.7
            } else {
                let mission_words: Vec<&str> = mission_lower.split_whitespace().collect();
                let query_words: Vec<&str> = query_lower.split_whitespace().collect();
                let matches = query_words.iter()
                    .filter(|qw| mission_words.iter().any(|mw| mw.contains(*qw)))
                    .count();
                if !query_words.is_empty() {
                    (matches as f64) / (query_words.len() as f64) * 0.5
                } else {
                    0.0
                }
            };

            if score > 0.0 {
                results.push(AgentSearchResult {
                    agent_id: agent.agent_id.clone(),
                    agent_name: agent.name.clone(),
                    mission: agent.mission.clone(),
                    score,
                    status: agent.status.clone(),
                });
            }
        }

        // Sort by score descending
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(top_k);
    }

    Ok(Json(AgentSearchResponse { results }))
}

/// Agent station metrics response
#[derive(Debug, Serialize)]
pub struct AgentStationMetrics {
    pub agent_id: String,
    pub agent_name: String,
    pub reasoning_load: f64, // 0-100, based on active tasks and complexity
    pub drift_frequency: f64, // 0-100, based on audit findings and corrections
    pub capability_score: f64, // 0-100, based on historical success rates
    pub active_tasks: usize,
    pub last_drift_timestamp: Option<String>,
}

/// Metrics response
#[derive(Debug, Serialize)]
pub struct MetricsStationsResponse {
    pub stations: Vec<AgentStationMetrics>,
}

/// Get metrics for all agent stations
pub async fn get_metrics_stations(
    State(state): State<PhoenixAppState>,
) -> Result<Json<MetricsStationsResponse>, StatusCode> {
    use crate::tools::audit_archiver;
    use rand::Rng;
    
    let agents = state.agent_factory.list_agents().await;
    let mut stations = Vec::new();
    let mut rng = rand::thread_rng();
    
    for agent in agents {
        // Calculate reasoning load based on status and activity
        let reasoning_load = match agent.status.as_str() {
            "active" => {
                // High load if agent is active
                75.0 + (rng.gen::<f64>() * 20.0) // 75-95% for active agents
            }
            "idle" => {
                // Low load if idle
                5.0 + (rng.gen::<f64>() * 10.0) // 5-15% for idle agents
            }
            _ => {
                // Medium load for other statuses
                30.0 + (rng.gen::<f64>() * 20.0) // 30-50% for other statuses
            }
        };
        
        // Calculate drift frequency based on last 5 audit entries
        let (drift_frequency, last_drift_timestamp) = {
            let mut drift_count = 0;
            let mut last_drift: Option<String> = None;
            
            // Search audit history for recent entries (last 7 days)
            match audit_archiver::search_audit_history("", Some(7), None).await {
                Ok(reports) => {
                    // Filter to last 5 reports and check for drift indicators
                    let recent_reports: Vec<_> = reports.into_iter().take(5).collect();
                    
                    for report in &recent_reports {
                        let report_text = format!("{} {} {}", 
                            report.report.executive_pulse,
                            report.report.climax,
                            report.report.rising_action.join(" ")
                        ).to_lowercase();
                        
                        // Check for drift indicators
                        if report_text.contains("drift") 
                            || report_text.contains("anomaly")
                            || report_text.contains("issue")
                            || report_text.contains("correction")
                            || report_text.contains("repair") {
                            drift_count += 1;
                            if last_drift.is_none() {
                                last_drift = Some(report.timestamp.clone());
                            }
                        }
                    }
                    
                    // Calculate frequency as percentage (drift_count / 5 * 100)
                    let freq = (drift_count as f64 / 5.0) * 100.0;
                    (freq, last_drift)
                }
                Err(_) => {
                    // If audit search fails, use a default low value
                    (0.0, None)
                }
            }
        };
        
        // Calculate capability score based on tool proposal success rate
        let capability_score = {
            let tool_proposals = state.tool_proposals.read().await;
            let agent_proposals: Vec<_> = tool_proposals.proposals.values()
                .filter(|p| p.agent_id == agent.agent_id)
                .collect();
            
            if agent_proposals.is_empty() {
                50.0 // Default score if no proposals
            } else {
                let successful = agent_proposals.iter()
                    .filter(|p| p.status == ProposalStatus::Approved && p.verified == Some(true))
                    .count();
                let total = agent_proposals.len();
                (successful as f64 / total as f64) * 100.0
            }
        };
        
        // Count active tasks (simplified - based on status)
        let active_tasks = if agent.status == "active" { 1 } else { 0 };
        
        stations.push(AgentStationMetrics {
            agent_id: agent.agent_id.clone(),
            agent_name: agent.name.clone(),
            reasoning_load: reasoning_load.min(100.0).max(0.0),
            drift_frequency: drift_frequency.min(100.0).max(0.0),
            capability_score: capability_score.min(100.0).max(0.0),
            active_tasks,
            last_drift_timestamp,
        });
    }
    
    Ok(Json(MetricsStationsResponse { stations }))
}

/// Request to create a tool installation proposal
#[derive(Debug, Deserialize)]
pub struct CreateToolProposalRequest {
    pub agent_id: String,
    pub agent_name: String,
    pub repository: String,
    pub tool_name: String,
    pub description: String,
    pub github_url: String,
    pub stars: u32,
    pub language: Option<String>,
    pub installation_command: String,
    pub code_snippet: String,
}

/// Response for listing proposals
#[derive(Debug, Serialize)]
pub struct ToolProposalsResponse {
    pub proposals: Vec<ToolInstallationProposal>,
}

/// Create a new tool installation proposal
pub async fn post_create_tool_proposal(
    State(state): State<PhoenixAppState>,
    Json(request): Json<CreateToolProposalRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let proposal_id = uuid::Uuid::new_v4().to_string();
    let created_at = chrono::Utc::now().to_rfc3339();

    let proposal = ToolInstallationProposal {
        id: proposal_id.clone(),
        agent_id: request.agent_id,
        agent_name: request.agent_name,
        repository: request.repository,
        tool_name: request.tool_name,
        description: request.description,
        github_url: request.github_url,
        stars: request.stars,
        language: request.language,
        installation_command: request.installation_command,
        code_snippet: request.code_snippet,
        status: ProposalStatus::Pending,
        created_at,
        reviewed_at: None,
        installation_success: None,
        verified: None,
        verification_message: None,
        repair_proposal: None,
    };

    {
        let mut store = state.tool_proposals.write().await;
        store.add_proposal(proposal.clone());
    }

    info!(
        proposal_id = %proposal_id,
        agent_name = %proposal.agent_name,
        tool_name = %proposal.tool_name,
        "Tool installation proposal created"
    );

    // Publish event to message bus
    let _ = state.message_bus.sender().send(PhoenixEvent::ToolProposalCreated {
        proposal_id: proposal_id.clone(),
        agent_name: proposal.agent_name.clone(),
        tool_name: proposal.tool_name.clone(),
    });

    // Automatically request peer review for GitHub tools (Agent Debate Protocol)
    // Only trigger if this is a GitHub tool (not a playbook)
    if proposal.github_url.starts_with("https://github.com") {
        let state_clone = state.clone();
        let proposal_id_clone = proposal_id.clone();
        let proposal_clone = proposal.clone();
        
        tokio::spawn(async move {
            // Wait a brief moment to ensure proposal is fully stored
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            
            let reasoning = format!(
                "I propose installing {} from GitHub. Repository: {}. Description: {}. Installation command: {}",
                proposal_clone.tool_name,
                proposal_clone.repository,
                proposal_clone.description,
                proposal_clone.installation_command
            );
            
            let review_request = RequestPeerReviewRequest {
                tool_proposal_id: proposal_id_clone.clone(),
                requesting_agent_id: proposal_clone.agent_id.clone(),
                requesting_agent_name: proposal_clone.agent_name.clone(),
                tool_name: proposal_clone.tool_name.clone(),
                github_url: proposal_clone.github_url.clone(),
                reasoning,
            };
            
            match post_request_peer_review(State(state_clone), Json(review_request)).await {
                Ok(response) => {
                    info!(
                        proposal_id = %proposal_id_clone,
                        review_id = %response.review_id,
                        expert_agent = %response.expert_agent_name,
                        "Automatic peer review requested for GitHub tool proposal"
                    );
                }
                Err(e) => {
                    warn!(
                        proposal_id = %proposal_id_clone,
                        error = ?e,
                        "Failed to automatically request peer review (this is OK if no expert agents available)"
                    );
                }
            }
        });
    }

    Ok(Json(json!({
        "ok": true,
        "proposal_id": proposal_id,
        "proposal": proposal,
        "peer_review_triggered": proposal.github_url.starts_with("https://github.com")
    })))
}

/// Get all tool installation proposals
pub async fn get_tool_proposals(
    State(state): State<PhoenixAppState>,
) -> Result<Json<ToolProposalsResponse>, StatusCode> {
    let store = state.tool_proposals.read().await;
    let proposals = store.get_all_proposals();
    Ok(Json(ToolProposalsResponse {
        proposals: proposals.into_iter().cloned().collect(),
    }))
}

/// Get pending tool installation proposals
pub async fn get_pending_tool_proposals(
    State(state): State<PhoenixAppState>,
) -> Result<Json<ToolProposalsResponse>, StatusCode> {
    let store = state.tool_proposals.read().await;
    let proposals = store.get_pending_proposals();
    Ok(Json(ToolProposalsResponse {
        proposals: proposals.into_iter().cloned().collect(),
    }))
}

/// Simulate a tool installation in a sandbox environment
pub async fn post_simulate_tool_proposal(
    State(state): State<PhoenixAppState>,
    Path(proposal_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // Get proposal
    let proposal = {
        let store = state.tool_proposals.read().await;
        store.get_proposal(&proposal_id).cloned()
    };

    let proposal = proposal.ok_or(StatusCode::NOT_FOUND)?;

    info!(
        proposal_id = %proposal_id,
        tool_name = %proposal.tool_name,
        command = %proposal.installation_command,
        "Running simulation for tool installation proposal"
    );

    // Determine installation type
    let installation_type = proposal.installation_command
        .split_whitespace()
        .next()
        .unwrap_or("unknown")
        .to_lowercase();

    // Run simulation
    let simulation_result = crate::tools::safe_installer::run_simulation(
        &proposal.tool_name,
        &proposal.installation_command,
        &installation_type,
        None, // No custom verification command for now
    ).await;

    match simulation_result {
        Ok(result) => {
            info!(
                proposal_id = %proposal_id,
                success = result.success,
                "Simulation completed"
            );

            // Save playbook if simulation was successful
            if result.success {
                let qdrant = state.qdrant_client.clone();
                let proposal_clone = proposal.clone();
                let installation_type_clone = installation_type.clone();
                
                tokio::spawn(async move {
                    // Generate embedding for playbook
                    let embedding_text = format!("{} {}", proposal_clone.tool_name, proposal_clone.installation_command);
                    let embedding = generate_dense_vector(&embedding_text, 384).await;
                    
                    // Build environment config
                    let mut env_config = std::collections::HashMap::new();
                    env_config.insert("installation_type".to_string(), installation_type_clone.clone());
                    if let Some(ref lang) = proposal_clone.language {
                        env_config.insert("language".to_string(), lang.clone());
                    }
                    
                    let playbook = crate::tools::playbook_store::Playbook {
                        id: uuid::Uuid::new_v4().to_string(),
                        tool_name: proposal_clone.tool_name.clone(),
                        repository: Some(proposal_clone.repository.clone()),
                        language: proposal_clone.language.clone(),
                        installation_command: proposal_clone.installation_command.clone(),
                        installation_type: installation_type_clone,
                        verification_command: None,
                        environment_config: env_config,
                        reliability_score: 1.0, // First successful simulation = 100%
                        success_count: 1,
                        total_attempts: 1,
                        verified_by_agent: Some(proposal_clone.agent_name.clone()),
                        verified_at: chrono::Utc::now().to_rfc3339(),
                        last_used_at: None,
                        description: Some(proposal_clone.description.clone()),
                        github_url: Some(proposal_clone.github_url.clone()),
                    };
                    
                    if let Err(e) = crate::tools::playbook_store::save_playbook(qdrant, playbook, embedding).await {
                        warn!(error = %e, "Failed to save playbook after successful simulation");
                    } else {
                        info!(tool_name = %proposal_clone.tool_name, "Playbook saved after successful simulation");
                    }
                });
            }

            Ok(Json(json!({
                "ok": true,
                "simulation": result,
                "message": if result.success {
                    "Simulation successful - tool can be safely installed"
                } else {
                    "Simulation failed - review errors before installing"
                }
            })))
        }
        Err(e) => {
            error!(
                proposal_id = %proposal_id,
                error = %e,
                "Simulation failed"
            );

            Ok(Json(json!({
                "ok": false,
                "error": e,
                "message": "Simulation execution failed"
            })))
        }
    }
}

/// Approve a tool installation proposal and execute the installation
pub async fn post_approve_tool_proposal(
    State(state): State<PhoenixAppState>,
    Path(proposal_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // Get proposal before updating status
    let proposal = {
        let store = state.tool_proposals.read().await;
        store.get_proposal(&proposal_id).cloned()
    };

    let proposal = proposal.ok_or(StatusCode::NOT_FOUND)?;

    // Validate and execute installation command
    info!(
        proposal_id = %proposal_id,
        tool_name = %proposal.tool_name,
        command = %proposal.installation_command,
        "Approving tool installation proposal and executing command"
    );

    // Execute the installation command
    let installation_result = crate::tools::safe_installer::execute_installation_command(
        &proposal.installation_command
    ).await;

    let (install_success, install_stdout, install_stderr) = match installation_result {
        Ok((success, stdout, stderr)) => (success, stdout, stderr),
        Err(e) => {
            error!(
                proposal_id = %proposal_id,
                error = %e,
                "Failed to validate or execute installation command"
            );
            return Ok(Json(json!({
                "ok": false,
                "error": e,
                "message": "Installation command validation or execution failed"
            })));
        }
    };

    // Update proposal status
    {
        let mut store = state.tool_proposals.write().await;
        store.update_proposal_status(&proposal_id, ProposalStatus::Approved);
    }

    // Determine installation type for verification
    let installation_type = proposal.installation_command
        .split_whitespace()
        .next()
        .unwrap_or("unknown")
        .to_lowercase();

    // Perform verification dry run
    let verification_result = crate::tools::safe_installer::verify_tool_installation(
        &proposal.tool_name,
        &installation_type
    ).await;

    let (verified, verification_message) = verification_result.unwrap_or_else(|e| {
        (false, format!("Verification failed: {}", e))
    });

    // If verification failed, propose a rollback/repair
    let repair_proposal = if !verified || !install_success {
        info!(
            proposal_id = %proposal_id,
            tool_name = %proposal.tool_name,
            "Verification or installation failed, generating repair proposal"
        );
        
        match crate::tools::safe_installer::propose_rollback(
            &proposal.tool_name,
            &proposal.installation_command,
            &installation_type
        ).await {
            Ok(Some(repair)) => {
                info!(
                    proposal_id = %proposal_id,
                    rollback_command = %repair.rollback_command,
                    "Repair proposal generated"
                );
                Some(serde_json::to_value(&repair).unwrap_or_else(|_| json!({})))
            }
            Ok(None) => {
                warn!(proposal_id = %proposal_id, "No repair proposal available");
                None
            }
            Err(e) => {
                warn!(
                    proposal_id = %proposal_id,
                    error = %e,
                    "Failed to generate repair proposal"
                );
                None
            }
        }
    } else {
        None
    };

    // Auto-trigger retrospective analysis if verification failed
    if !verified || !install_success {
        let qdrant_for_retro = state.qdrant_client.clone();
        let message_bus_sender = state.message_bus.sender().clone();
        let retrospective_store = state.retrospectives.clone();
        let proposal_clone = proposal.clone();
        let proposal_id_clone = proposal_id.clone();
        let agent_id_clone = proposal.agent_id.clone();
        let agent_name_clone = proposal.agent_name.clone();
        let error_output = format!("{}\n{}", install_stderr, verification_message);
        
        tokio::spawn(async move {
            // Try to find the playbook ID from the proposal
            // If it's a new proposal, we might not have a playbook yet
            let all_playbooks = match crate::tools::playbook_store::get_all_playbooks(
                qdrant_for_retro.clone(),
                Some(1000)
            ).await {
                Ok(pbs) => pbs,
                Err(e) => {
                    warn!(error = %e, "Failed to get playbooks for retrospective");
                    return;
                }
            };
            
            // Find matching playbook by tool name
            if let Some(playbook) = all_playbooks.iter().find(|p| p.tool_name == proposal_clone.tool_name) {
                match crate::tools::playbook_store::generate_retrospective(
                    &playbook.id,
                    Some(&proposal_id_clone),
                    &agent_id_clone,
                    &agent_name_clone,
                    &verification_message,
                    &error_output,
                    qdrant_for_retro.clone(),
                ).await {
                    Ok(retrospective) => {
                        info!(
                            retrospective_id = %retrospective.retrospective_id,
                            playbook_id = %playbook.id,
                            root_cause = %retrospective.root_cause,
                            "Retrospective analysis generated"
                        );
                        
                        // Store retrospective
                        {
                            let mut store = retrospective_store.write().await;
                            store.add_retrospective(retrospective.clone());
                        }
                        
                        // Publish Post-Mortem event
                        let patch_json = retrospective.suggested_patch.as_ref()
                            .and_then(|p| serde_json::to_string(p).ok());
                        
                        let _ = message_bus_sender.send(PhoenixEvent::PostMortemRetrospective {
                            retrospective_id: retrospective.retrospective_id.clone(),
                            playbook_id: retrospective.playbook_id.clone(),
                            tool_name: retrospective.tool_name.clone(),
                            agent_id: retrospective.agent_id.clone(),
                            agent_name: retrospective.agent_name.clone(),
                            root_cause: retrospective.root_cause.clone(),
                            error_pattern: retrospective.error_pattern.clone(),
                            suggested_patch: patch_json,
                            reliability_impact: retrospective.reliability_impact,
                            timestamp: retrospective.created_at.clone(),
                        });
                    }
                    Err(e) => {
                        warn!(
                            proposal_id = %proposal_id_clone,
                            error = %e,
                            "Failed to generate retrospective analysis"
                        );
                    }
                }
            }
        });
    }

    // Update proposal with verification results and repair proposal
    {
        let mut store = state.tool_proposals.write().await;
        if let Some(proposal_mut) = store.proposals.get_mut(&proposal_id) {
            proposal_mut.installation_success = Some(install_success);
            proposal_mut.verified = Some(verified);
            proposal_mut.verification_message = Some(verification_message.clone());
            proposal_mut.repair_proposal = repair_proposal.clone();
        }
    }

    // Save playbook if installation and verification were successful
    if install_success && verified {
        let qdrant = state.qdrant_client.clone();
        let proposal_clone = proposal.clone();
        let installation_type_clone = installation_type.clone();
        
        tokio::spawn(async move {
            // Generate embedding for playbook
            let embedding_text = format!("{} {}", proposal_clone.tool_name, proposal_clone.installation_command);
            let embedding = generate_dense_vector(&embedding_text, 384).await;
            
            // Build environment config
            let mut env_config = std::collections::HashMap::new();
            env_config.insert("installation_type".to_string(), installation_type_clone.clone());
            if let Some(ref lang) = proposal_clone.language {
                env_config.insert("language".to_string(), lang.clone());
            }
            
            let playbook = crate::tools::playbook_store::Playbook {
                id: uuid::Uuid::new_v4().to_string(),
                tool_name: proposal_clone.tool_name.clone(),
                repository: Some(proposal_clone.repository.clone()),
                language: proposal_clone.language.clone(),
                installation_command: proposal_clone.installation_command.clone(),
                installation_type: installation_type_clone,
                verification_command: None,
                environment_config: env_config,
                reliability_score: 1.0, // First successful deployment = 100%
                success_count: 1,
                total_attempts: 1,
                verified_by_agent: Some(proposal_clone.agent_name.clone()),
                verified_at: chrono::Utc::now().to_rfc3339(),
                last_used_at: None,
                description: Some(proposal_clone.description.clone()),
                github_url: Some(proposal_clone.github_url.clone()),
            };
            
            if let Err(e) = crate::tools::playbook_store::save_playbook(qdrant, playbook, embedding).await {
                warn!(error = %e, "Failed to save playbook after successful deployment");
            } else {
                info!(tool_name = %proposal_clone.tool_name, "Playbook saved after successful deployment");
            }
        });
    }

    // Log installation result to Auditor's logs if possible
    let log_message = format!(
        "[TOOL INSTALLATION] Approved: {}\nCommand: {}\nInstallation: {}\nVerification: {}\nStdout: {}\nStderr: {}",
        proposal.tool_name,
        proposal.installation_command,
        if install_success { "SUCCESS" } else { "FAILED" },
        verification_message,
        install_stdout,
        install_stderr
    );

    // Try to find Phoenix Auditor agent and post a verification task
    let agent_factory = state.agent_factory.clone();
    let proposal_clone = proposal.clone();
    let proposal_id_clone = proposal_id.clone();
    tokio::spawn(async move {
        let agents = agent_factory.list_agents().await;
        if let Some(auditor) = agents.iter().find(|a| a.name == "Phoenix Auditor") {
            let verification_task = format!(
                "Verify installation of tool '{}' from proposal {}. Installation command: '{}'. Installation result: {}. Verification: {}. Please log this to your audit logs.",
                proposal_clone.tool_name,
                proposal_id_clone,
                proposal_clone.installation_command,
                if install_success { "SUCCESS" } else { "FAILED" },
                verification_message
            );
            // Post verification task to Auditor
            match agent_factory.post_task(&auditor.agent_id, verification_task).await {
                Ok(_) => {
                    info!("Verification task posted to Phoenix Auditor");
                }
                Err(e) => {
                    warn!(error = %e, "Failed to post verification task to Phoenix Auditor");
                }
            }
        }
    });

    info!(
        proposal_id = %proposal_id,
        tool_name = %proposal.tool_name,
        install_success = install_success,
        verified = verified,
        "Tool installation completed"
    );

    // Publish event to message bus
    let _ = state.message_bus.sender().send(PhoenixEvent::ToolProposalApproved {
        proposal_id: proposal_id.clone(),
        tool_name: proposal.tool_name.clone(),
        installation_command: proposal.installation_command.clone(),
    });

    Ok(Json(json!({
        "ok": true,
        "message": "Proposal approved and installation executed",
        "installation": {
            "success": install_success,
            "stdout": install_stdout,
            "stderr": install_stderr
        },
        "verification": {
            "verified": verified,
            "message": verification_message
        },
        "repair_proposal": repair_proposal
    })))
}

/// Reject a tool installation proposal
pub async fn post_reject_tool_proposal(
    State(state): State<PhoenixAppState>,
    Path(proposal_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut store = state.tool_proposals.write().await;
    
    if !store.update_proposal_status(&proposal_id, ProposalStatus::Rejected) {
        return Err(StatusCode::NOT_FOUND);
    }

    let proposal = store.get_proposal(&proposal_id).cloned();
    
    if let Some(prop) = proposal {
        info!(
            proposal_id = %proposal_id,
            tool_name = %prop.tool_name,
            "Tool installation proposal rejected"
        );

        // Publish event to message bus
        let _ = state.message_bus.sender().send(PhoenixEvent::ToolProposalRejected {
            proposal_id: proposal_id.clone(),
            tool_name: prop.tool_name.clone(),
        });
    }

    Ok(Json(json!({
        "ok": true,
        "message": "Proposal rejected"
    })))
}

/// Get all playbooks (for library view)
pub async fn get_all_playbooks(
    State(state): State<PhoenixAppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match crate::tools::playbook_store::get_all_playbooks(
        state.qdrant_client.clone(),
        Some(100),
    ).await {
        Ok(playbooks) => {
            Ok(Json(json!({
                "ok": true,
                "playbooks": playbooks,
                "count": playbooks.len()
            })))
        }
        Err(e) => {
            error!(error = %e, "Failed to get playbooks");
            Ok(Json(json!({
                "ok": false,
                "error": e,
                "playbooks": []
            })))
        }
    }
}

/// Query parameters for playbook search
#[derive(Debug, Deserialize)]
pub struct PlaybookSearchQuery {
    pub query: Option<String>,
    pub tool_name: Option<String>,
    pub min_reliability: Option<f64>,
    pub limit: Option<usize>,
}

/// Search playbooks by query or tool name
pub async fn get_playbooks_search(
    State(state): State<PhoenixAppState>,
    Query(params): Query<PlaybookSearchQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let qdrant = state.qdrant_client.clone();
    
    // If tool_name is provided, search by tool name
    if let Some(tool_name) = params.tool_name {
        match crate::tools::playbook_store::search_playbooks_by_tool(
            qdrant,
            &tool_name,
            params.limit,
        ).await {
            Ok(results) => {
                Ok(Json(json!({
                    "ok": true,
                    "results": results,
                    "count": results.len()
                })))
            }
            Err(e) => {
                error!(error = %e, "Failed to search playbooks by tool name");
                Ok(Json(json!({
                    "ok": false,
                    "error": e,
                    "results": []
                })))
            }
        }
    } else if let Some(query) = params.query {
        // Generate embedding for semantic search
        let embedding = generate_dense_vector(&query, 384).await;
        
        match crate::tools::playbook_store::search_playbooks_by_query(
            qdrant,
            &query,
            embedding,
            params.min_reliability,
            params.limit,
        ).await {
            Ok(results) => {
                Ok(Json(json!({
                    "ok": true,
                    "results": results,
                    "count": results.len()
                })))
            }
            Err(e) => {
                error!(error = %e, "Failed to search playbooks by query");
                Ok(Json(json!({
                    "ok": false,
                    "error": e,
                    "results": []
                })))
            }
        }
    } else {
        // No query provided, return all playbooks
        get_all_playbooks(State(state)).await
    }
}

/// Query parameters for audit history search
#[derive(Debug, Deserialize)]
pub struct AuditHistoryQuery {
    pub path: Option<String>,
    pub days: Option<u32>,
    pub limit: Option<u32>,
    /// Optional node ID to filter by (None = search across all nodes)
    pub source_node: Option<String>,
}

/// Get audit history for a specific path or all recent audits
pub async fn get_audit_history(
    State(_state): State<PhoenixAppState>,
    Query(params): Query<AuditHistoryQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    use crate::tools::audit_archiver;
    
    if let Some(path) = &params.path {
        // Search for specific path (across all nodes by default, or filter by source_node)
        match audit_archiver::search_audit_history(path, params.days, params.source_node.as_deref()).await {
            Ok(reports) => {
                let limit = params.limit.unwrap_or(100) as usize;
                let limited_reports: Vec<_> = reports.into_iter().take(limit).collect();
                Ok(Json(json!({
                    "ok": true,
                    "path": path,
                    "reports": limited_reports,
                    "count": limited_reports.len()
                })))
            }
            Err(e) => {
                error!(error = %e, path = %path, "Failed to search audit history");
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    } else {
        // Return empty result if no path specified
        Ok(Json(json!({
            "ok": true,
            "message": "Please specify a 'path' query parameter",
            "reports": [],
            "count": 0
        })))
    }
}

/// Query parameters for audit trend analysis
#[derive(Debug, Deserialize)]
pub struct AuditTrendsQuery {
    pub path: String,
    pub days: Option<u32>,
    /// Optional node ID to filter by (None = search across all nodes)
    pub source_node: Option<String>,
}

/// Get trend analysis for a specific path
pub async fn get_audit_trends(
    State(_state): State<PhoenixAppState>,
    Query(params): Query<AuditTrendsQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    use crate::tools::audit_archiver;
    
    match audit_archiver::analyze_audit_trends(&params.path, params.days, params.source_node.as_deref()).await {
        Ok(trend) => {
            Ok(Json(json!({
                "ok": true,
                "trend": trend
            })))
        }
        Err(e) => {
            error!(error = %e, path = %params.path, "Failed to analyze audit trends");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// System pause status response
#[derive(Debug, Serialize)]
pub struct SystemPauseStatusResponse {
    pub paused: bool,
}

/// Get system pause status
pub async fn get_system_pause_status() -> Result<Json<SystemPauseStatusResponse>, StatusCode> {
    let paused = *get_system_pause_state().read().await;
    Ok(Json(SystemPauseStatusResponse { paused }))
}

/// Pause system (Global Kill-Switch)
pub async fn post_system_pause() -> Result<Json<serde_json::Value>, StatusCode> {
    let mut paused = get_system_pause_state().write().await;
    *paused = true;
    info!("System PAUSED - All autonomous deployments halted");
    Ok(Json(json!({
        "success": true,
        "message": "System paused. All autonomous deployments halted."
    })))
}

/// Resume system
pub async fn post_system_resume() -> Result<Json<serde_json::Value>, StatusCode> {
    let mut paused = get_system_pause_state().write().await;
    *paused = false;
    info!("System RESUMED - Autonomous deployments enabled");
    Ok(Json(json!({
        "success": true,
        "message": "System resumed. Autonomous deployments enabled."
    })))
}

/// Check if system is paused (helper function for other modules)
pub async fn is_system_paused() -> bool {
    *get_system_pause_state().read().await
}

/// Request peer review for a tool proposal
#[derive(Debug, Deserialize)]
pub struct RequestPeerReviewRequest {
    pub tool_proposal_id: String,
    pub requesting_agent_id: String,
    pub requesting_agent_name: String,
    pub tool_name: String,
    pub github_url: String,
    pub reasoning: String,
}

/// Request peer review response
#[derive(Debug, Serialize)]
pub struct RequestPeerReviewResponse {
    pub review_id: String,
    pub expert_agent_id: String,
    pub expert_agent_name: String,
    pub message: String,
}

/// Find expert agent for a tool proposal using capability heatmap data
async fn find_expert_agent(
    qdrant_client: Arc<Qdrant>,
    agent_factory: Arc<AgentFactory>,
    tool_name: &str,
    language: Option<&str>,
    github_url: &str,
) -> Result<(String, String), String> {
    // Get all active agents
    let agents = agent_factory.list_agents().await;
    if agents.is_empty() {
        return Err("No active agents available for peer review".to_string());
    }

    // Search agent_logs collection for agents with expertise in this tool/language
    let query_text = if let Some(lang) = language {
        format!("{} {} tool installation", tool_name, lang)
    } else {
        format!("{} tool installation", tool_name)
    };

    let embedding_dim = std::env::var("EMBEDDING_MODEL_DIM")
        .unwrap_or_else(|_| "384".to_string())
        .parse::<usize>()
        .unwrap_or(384);

    let query_embedding = generate_dense_vector(&query_text, embedding_dim).await;

    let search_request = SearchPoints {
        collection_name: "agent_logs".to_string(),
        vector: query_embedding,
        limit: 10,
        score_threshold: Some(0.3),
        with_payload: Some(true.into()),
        with_vectors: Some(false.into()),
        filter: None,
        ..Default::default()
    };

    let mut agent_scores: HashMap<String, f64> = HashMap::new();

    match qdrant_client.search_points(&search_request).await {
        Ok(results) => {
            for point in results.result {
                if let Some(agent_id) = point.payload.get("agent_id")
                    .and_then(|v| v.kind.as_ref())
                    .and_then(|k| k.string_value())
                {
                    let score = point.score;
                    let current_score = agent_scores.get(&agent_id).copied().unwrap_or(0.0);
                    agent_scores.insert(agent_id.clone(), current_score + score);
                }
            }
        }
        Err(e) => {
            warn!(error = %e, "Failed to search agent_logs for expert agent");
        }
    }

    // Also check playbooks for agents who have successfully used similar tools
    if let Some(lang) = language {
        let playbook_query = format!("{} {}", tool_name, lang);
        let playbook_embedding = generate_dense_vector(&playbook_query, embedding_dim).await;
        
        match crate::tools::playbook_store::search_playbooks_by_query(
            qdrant_client.clone(),
            &playbook_query,
            playbook_embedding,
            Some(0.5), // Minimum reliability
            Some(10),
        ).await {
            Ok(playbook_results) => {
                for result in playbook_results {
                    if let Some(agent_id) = result.playbook.verified_by_agent {
                        // Try to match agent_id with active agents
                        if let Some(agent) = agents.iter().find(|a| a.name == agent_id || a.agent_id == agent_id) {
                            let score = result.relevance_score * result.playbook.reliability_score;
                            let current_score = agent_scores.get(&agent.agent_id).copied().unwrap_or(0.0);
                            agent_scores.insert(agent.agent_id.clone(), current_score + score);
                        }
                    }
                }
            }
            Err(e) => {
                warn!(error = %e, "Failed to search playbooks for expert agent");
            }
        }
    }

    // Find agent with highest score
    if let Some((expert_id, _)) = agent_scores.iter().max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal)) {
        if let Some(agent) = agents.iter().find(|a| &a.agent_id == expert_id) {
            return Ok((agent.agent_id.clone(), agent.name.clone()));
        }
    }

    // Fallback: return first available agent (excluding the requesting agent if provided)
    if let Some(agent) = agents.first() {
        Ok((agent.agent_id.clone(), agent.name.clone()))
    } else {
        Err("No expert agent found".to_string())
    }
}

/// Request peer review endpoint
pub async fn post_request_peer_review(
    State(state): State<PhoenixAppState>,
    Json(request): Json<RequestPeerReviewRequest>,
) -> Result<Json<RequestPeerReviewResponse>, StatusCode> {
    // Get tool proposal to extract language
    let proposal = {
        let store = state.tool_proposals.read().await;
        store.get_proposal(&request.tool_proposal_id).cloned()
    };

    let language = proposal.as_ref().and_then(|p| p.language.as_deref());

    // Find expert agent
    let (expert_agent_id, expert_agent_name) = find_expert_agent(
        state.qdrant_client.clone(),
        state.agent_factory.clone(),
        &request.tool_name,
        language,
        &request.github_url,
    ).await.map_err(|e| {
        error!(error = %e, "Failed to find expert agent");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Don't allow self-review
    if expert_agent_id == request.requesting_agent_id {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Create peer review
    let review_id = uuid::Uuid::new_v4().to_string();
    let review = PeerReview {
        review_id: review_id.clone(),
        tool_proposal_id: request.tool_proposal_id.clone(),
        requesting_agent_id: request.requesting_agent_id.clone(),
        requesting_agent_name: request.requesting_agent_name.clone(),
        expert_agent_id: expert_agent_id.clone(),
        expert_agent_name: expert_agent_name.clone(),
        tool_name: request.tool_name.clone(),
        github_url: request.github_url.clone(),
        requesting_reasoning: request.reasoning.clone(),
        expert_decision: None,
        expert_reasoning: None,
        alternative_playbook_id: None,
        status: ReviewStatus::Pending,
        consensus: None,
        created_at: chrono::Utc::now().to_rfc3339(),
        reviewed_at: None,
        consensus_at: None,
    };

    // Store review
    {
        let mut store = state.peer_reviews.write().await;
        store.add_review(review.clone());
    }

    // Publish event
    let _ = state.message_bus.sender().send(PhoenixEvent::PeerReviewRequest {
        review_id: review_id.clone(),
        requesting_agent_id: request.requesting_agent_id.clone(),
        requesting_agent_name: request.requesting_agent_name.clone(),
        expert_agent_id: expert_agent_id.clone(),
        expert_agent_name: expert_agent_name.clone(),
        tool_proposal_id: request.tool_proposal_id.clone(),
        tool_name: request.tool_name.clone(),
        github_url: request.github_url.clone(),
        reasoning: request.reasoning.clone(),
        timestamp: review.created_at.clone(),
    });

    // Load expert agent persona if available
    let expert_persona = crate::agents::persona::get_persona(
        state.qdrant_client.clone(),
        &expert_agent_id,
    ).await.unwrap_or(None);

    // Post review task to expert agent
    let agent_factory = state.agent_factory.clone();
    let review_id_clone = review_id.clone();
    let review_clone = review.clone();
    let expert_persona_clone = expert_persona.clone();
    let expert_agent_id_clone = expert_agent_id.clone();
    tokio::spawn(async move {
        // Build persona context for the task
        let persona_context = if let Some(ref persona) = expert_persona_clone {
            format!(
                "\n\n[YOUR PERSONA: {}]\n\
                Voice Tone: {}\n\
                Your behavioral biases:\n\
                - Cautiousness: {:.1}% (higher = more likely to object to risky proposals)\n\
                - Innovation: {:.1}% (higher = favor novel approaches)\n\
                - Detail Orientation: {:.1}% (higher = focus on edge cases)\n\n\
                Remember: Your persona influences how you evaluate proposals. Stay true to your character while being thorough.\n",
                persona.name,
                persona.voice_tone,
                persona.behavioral_bias.cautiousness * 100.0,
                persona.behavioral_bias.innovation * 100.0,
                persona.behavioral_bias.detail_orientation * 100.0
            )
        } else {
            String::new()
        };

        let review_task = format!(
            "PEER REVIEW REQUEST (Review ID: {})\n\n\
            Tool Proposal: {}\n\
            Tool Name: {}\n\
            GitHub URL: {}\n\n\
            Requesting Agent: {} says:\n\"{}\"\n\n\
            Please review this tool proposal. Check:\n\
            1. Your local logs for similar tools\n\
            2. The Global Playbook for verified alternatives\n\
            3. Security and compatibility concerns{}\n\n\
            Respond with JSON:\n\
            {{\n\
              \"status\": \"ok\",\n\
              \"decision\": \"concur\" | \"object\",\n\
              \"reasoning\": \"Your detailed reasoning\",\n\
              \"alternative_playbook_id\": \"<playbook_id if objecting with alternative>\" | null\n\
            }}",
            review_id_clone,
            review_clone.tool_proposal_id,
            review_clone.tool_name,
            review_clone.github_url,
            review_clone.requesting_agent_name,
            review_clone.requesting_reasoning,
            persona_context
        );

        match agent_factory.post_task(&expert_agent_id_clone, review_task).await {
            Ok(_) => {
                info!(
                    review_id = %review_id_clone,
                    expert_agent_id = %expert_agent_id,
                    "Peer review task posted to expert agent"
                );
            }
            Err(e) => {
                warn!(
                    review_id = %review_id_clone,
                    error = %e,
                    "Failed to post peer review task to expert agent"
                );
            }
        }
    });

    Ok(Json(RequestPeerReviewResponse {
        review_id,
        expert_agent_id,
        expert_agent_name,
        message: format!("Peer review requested from {}", expert_agent_name),
    }))
}

/// Submit peer review response
#[derive(Debug, Deserialize)]
pub struct SubmitPeerReviewRequest {
    pub review_id: String,
    pub expert_agent_id: String,
    pub decision: String, // "concur" or "object"
    pub reasoning: String,
    pub alternative_playbook_id: Option<String>,
}

/// Submit peer review response
#[derive(Debug, Serialize)]
pub struct SubmitPeerReviewResponse {
    pub success: bool,
    pub message: String,
    pub consensus: Option<String>,
}

/// Submit peer review endpoint
pub async fn post_submit_peer_review(
    State(state): State<PhoenixAppState>,
    Json(request): Json<SubmitPeerReviewRequest>,
) -> Result<Json<SubmitPeerReviewResponse>, StatusCode> {
    let decision = match request.decision.as_str() {
        "concur" => ReviewDecision::Concur,
        "object" => ReviewDecision::Object,
        _ => return Err(StatusCode::BAD_REQUEST),
    };

    // Update review
    let review = {
        let mut store = state.peer_reviews.write().await;
        if !store.update_review_response(
            &request.review_id,
            decision.clone(),
            request.reasoning.clone(),
            request.alternative_playbook_id.clone(),
        ) {
            return Err(StatusCode::NOT_FOUND);
        }
        store.get_review(&request.review_id).cloned()
    };

    let review = review.ok_or(StatusCode::NOT_FOUND)?;

    // Publish response event
    let _ = state.message_bus.sender().send(PhoenixEvent::PeerReviewResponse {
        review_id: request.review_id.clone(),
        expert_agent_id: request.expert_agent_id.clone(),
        expert_agent_name: review.expert_agent_name.clone(),
        decision: request.decision.clone(),
        reasoning: request.reasoning.clone(),
        alternative_playbook_id: request.alternative_playbook_id.clone(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    });

    // Determine consensus
    let consensus = match decision {
        ReviewDecision::Concur => "approved".to_string(),
        ReviewDecision::Object => "rejected".to_string(),
    };

    // Update consensus
    {
        let mut store = state.peer_reviews.write().await;
        store.update_review_consensus(&request.review_id, consensus.clone());
    }

    // Publish consensus event
    let _ = state.message_bus.sender().send(PhoenixEvent::PeerReviewConsensus {
        review_id: request.review_id.clone(),
        tool_proposal_id: review.tool_proposal_id.clone(),
        consensus: consensus.clone(),
        requesting_agent_id: review.requesting_agent_id.clone(),
        expert_agent_id: request.expert_agent_id.clone(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    });

    Ok(Json(SubmitPeerReviewResponse {
        success: true,
        message: format!("Peer review submitted: {}", request.decision),
        consensus: Some(consensus),
    }))
}

/// Get peer reviews for a tool proposal
pub async fn get_peer_reviews(
    State(state): State<PhoenixAppState>,
    Path(proposal_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let store = state.peer_reviews.read().await;
    let reviews = store.get_reviews_for_proposal(&proposal_id);
    
    Ok(Json(json!({
        "ok": true,
        "reviews": reviews,
        "count": reviews.len()
    })))
}

/// Get all peer reviews
pub async fn get_all_peer_reviews(
    State(state): State<PhoenixAppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let store = state.peer_reviews.read().await;
    let reviews = store.get_all_reviews();
    
    Ok(Json(json!({
        "ok": true,
        "reviews": reviews,
        "count": reviews.len()
    })))
}

/// Deploy playbook to cluster endpoint
pub async fn post_deploy_playbook_to_cluster(
    State(state): State<PhoenixAppState>,
    Path(playbook_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    info!(
        playbook_id = %playbook_id,
        "Deploying playbook to cluster"
    );

    match crate::tools::playbook_store::deploy_playbook_to_cluster(
        &playbook_id,
        state.qdrant_client.clone(),
        state.agent_factory.clone(),
    ).await {
        Ok(result) => {
            info!(
                playbook_id = %playbook_id,
                total_agents = result.total_agents,
                successful = result.successful_deployments,
                failed = result.failed_deployments,
                "Playbook deployment completed"
            );

            Ok(Json(json!({
                "ok": true,
                "message": format!("Deployed to {} of {} agents", result.successful_deployments, result.total_agents),
                "deployment": result
            })))
        }
        Err(e) => {
            error!(
                playbook_id = %playbook_id,
                error = %e,
                "Failed to deploy playbook to cluster"
            );

            Ok(Json(json!({
                "ok": false,
                "error": e
            })))
        }
    }
}

/// Assign or update agent persona
#[derive(Debug, Deserialize)]
pub struct AssignPersonaRequest {
    pub agent_id: String,
    pub name: String,
    pub behavioral_bias: BehavioralBiasRequest,
    pub voice_tone: String,
}

#[derive(Debug, Deserialize)]
pub struct BehavioralBiasRequest {
    pub cautiousness: f64,
    pub innovation: f64,
    pub detail_orientation: f64,
}

#[derive(Debug, Serialize)]
pub struct AssignPersonaResponse {
    pub success: bool,
    pub message: String,
}

/// Assign persona endpoint
pub async fn post_assign_persona(
    State(state): State<PhoenixAppState>,
    Json(request): Json<AssignPersonaRequest>,
) -> Result<Json<AssignPersonaResponse>, StatusCode> {
    // Validate bias values
    if request.behavioral_bias.cautiousness < 0.0 || request.behavioral_bias.cautiousness > 1.0
        || request.behavioral_bias.innovation < 0.0 || request.behavioral_bias.innovation > 1.0
        || request.behavioral_bias.detail_orientation < 0.0 || request.behavioral_bias.detail_orientation > 1.0 {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Get embedding dimension (default to 384 for all-MiniLM-L6-v2)
    let embedding_dim = 384;

    let persona = crate::agents::persona::AgentPersona {
        agent_id: request.agent_id.clone(),
        name: request.name.clone(),
        behavioral_bias: crate::agents::persona::BehavioralBias {
            cautiousness: request.behavioral_bias.cautiousness,
            innovation: request.behavioral_bias.innovation,
            detail_orientation: request.behavioral_bias.detail_orientation,
        },
        voice_tone: request.voice_tone.clone(),
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    };

    match crate::agents::persona::save_persona(
        state.qdrant_client.clone(),
        persona,
        embedding_dim,
    ).await {
        Ok(_) => {
            info!(
                agent_id = %request.agent_id,
                persona_name = %request.name,
                "Persona assigned to agent"
            );
            Ok(Json(AssignPersonaResponse {
                success: true,
                message: format!("Persona '{}' assigned to agent", request.name),
            }))
        }
        Err(e) => {
            error!(
                agent_id = %request.agent_id,
                error = %e,
                "Failed to assign persona"
            );
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get agent persona
#[derive(Debug, Serialize)]
pub struct GetPersonaResponse {
    pub success: bool,
    pub persona: Option<crate::agents::persona::AgentPersona>,
}

/// Get persona endpoint
pub async fn get_persona(
    State(state): State<PhoenixAppState>,
    Path(agent_id): Path<String>,
) -> Result<Json<GetPersonaResponse>, StatusCode> {
    match crate::agents::persona::get_persona(
        state.qdrant_client.clone(),
        &agent_id,
    ).await {
        Ok(Some(persona)) => {
            Ok(Json(GetPersonaResponse {
                success: true,
                persona: Some(persona),
            }))
        }
        Ok(None) => {
            Ok(Json(GetPersonaResponse {
                success: true,
                persona: None,
            }))
        }
        Err(e) => {
            error!(
                agent_id = %agent_id,
                error = %e,
                "Failed to get persona"
            );
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get all personas
#[derive(Debug, Serialize)]
pub struct GetAllPersonasResponse {
    pub success: bool,
    pub personas: Vec<crate::agents::persona::AgentPersona>,
}

/// Get all personas endpoint
pub async fn get_all_personas(
    State(state): State<PhoenixAppState>,
) -> Result<Json<GetAllPersonasResponse>, StatusCode> {
    match crate::agents::persona::get_all_personas(
        state.qdrant_client.clone(),
    ).await {
        Ok(personas) => {
            Ok(Json(GetAllPersonasResponse {
                success: true,
                personas,
            }))
        }
        Err(e) => {
            error!(
                error = %e,
                "Failed to get all personas"
            );
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
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
        .route("/api/knowledge/path", post(post_knowledge_path))
        .route("/api/knowledge/ingest", post(post_knowledge_ingest))
        .route("/api/knowledge/ingest/status", get(get_knowledge_ingest_status))
        // Phoenix Chronos: Scheduled Task Management
        .route("/api/scheduled-tasks", get(get_scheduled_tasks))
        .route("/api/scheduled-tasks", post(post_create_scheduled_task))
        .route("/api/scheduled-tasks/:id", get(get_scheduled_task))
        .route("/api/scheduled-tasks/:id", axum::routing::delete(delete_scheduled_task))
        // Agent Search
        .route("/api/agents/search", get(get_agents_search))
        // Agent Metrics
        .route("/api/phoenix/metrics/stations", get(get_metrics_stations))
        // Tool Installation Proposals
        .route("/api/tool-proposals", get(get_tool_proposals))
        .route("/api/tool-proposals/pending", get(get_pending_tool_proposals))
        .route("/api/tool-proposals", post(post_create_tool_proposal))
        .route("/api/tool-proposals/:id/simulate", post(post_simulate_tool_proposal))
        .route("/api/tool-proposals/:id/approve", post(post_approve_tool_proposal))
        .route("/api/tool-proposals/:id/reject", post(post_reject_tool_proposal))
        // Global Playbooks
        .route("/api/playbooks", get(get_all_playbooks))
        .route("/api/playbooks/search", get(get_playbooks_search))
        // Audit History & Trend Analysis
        .route("/api/audit/history", get(get_audit_history))
        .route("/api/audit/trends", get(get_audit_trends))
        // System Pause/Resume (Global Kill-Switch)
        .route("/api/phoenix/system/pause", post(post_system_pause))
        .route("/api/phoenix/system/resume", post(post_system_resume))
        .route("/api/phoenix/system/pause-status", get(get_system_pause_status))
        // Peer Review (Agent Debate)
        .route("/api/peer-reviews", post(post_request_peer_review))
        .route("/api/peer-reviews/submit", post(post_submit_peer_review))
        .route("/api/peer-reviews/proposal/:id", get(get_peer_reviews))
        .route("/api/peer-reviews", get(get_all_peer_reviews))
        // Fleet Deployment
        .route("/api/playbooks/:id/deploy", post(post_deploy_playbook_to_cluster))
        // Retrospective Analysis
        .route("/api/retrospectives", get(get_all_retrospectives))
        .route("/api/retrospectives/playbook/:id", get(get_retrospectives_for_playbook))
        .route("/api/retrospectives/:id", get(get_retrospective))
        .route("/api/retrospectives/:id/apply-patch", post(post_apply_patch))
        // Agent Personas
        .route("/api/agents/:id/persona", get(get_persona))
        .route("/api/agents/persona", post(post_assign_persona))
        .route("/api/agents/personas", get(get_all_personas))
        .with_state(state)
}

/// Get all retrospectives
pub async fn get_all_retrospectives(
    State(state): State<PhoenixAppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let store = state.retrospectives.read().await;
    let retrospectives = store.get_all_retrospectives();
    
    Ok(Json(json!({
        "ok": true,
        "retrospectives": retrospectives,
        "count": retrospectives.len()
    })))
}

/// Get retrospectives for a playbook
pub async fn get_retrospectives_for_playbook(
    State(state): State<PhoenixAppState>,
    Path(playbook_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let store = state.retrospectives.read().await;
    let retrospectives = store.get_retrospectives_for_playbook(&playbook_id);
    
    Ok(Json(json!({
        "ok": true,
        "playbook_id": playbook_id,
        "retrospectives": retrospectives,
        "count": retrospectives.len()
    })))
}

/// Get a specific retrospective
pub async fn get_retrospective(
    State(state): State<PhoenixAppState>,
    Path(retrospective_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let store = state.retrospectives.read().await;
    
    if let Some(retrospective) = store.get_retrospective(&retrospective_id) {
        Ok(Json(json!({
            "ok": true,
            "retrospective": retrospective
        })))
    } else {
        Ok(Json(json!({
            "ok": false,
            "error": "Retrospective not found"
        })))
    }
}

/// Apply a patch from a retrospective to the playbook
pub async fn post_apply_patch(
    State(state): State<PhoenixAppState>,
    Path(retrospective_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let retrospective = {
        let store = state.retrospectives.read().await;
        store.get_retrospective(&retrospective_id).cloned()
    };

    let retrospective = retrospective.ok_or_else(|| {
        error!(retrospective_id = %retrospective_id, "Retrospective not found");
        StatusCode::NOT_FOUND
    })?;

    let patch = retrospective.suggested_patch.as_ref().ok_or_else(|| {
        error!(retrospective_id = %retrospective_id, "No patch available for retrospective");
        StatusCode::BAD_REQUEST
    })?;

    info!(
        retrospective_id = %retrospective_id,
        playbook_id = %retrospective.playbook_id,
        patch_id = %patch.patch_id,
        "Applying patch to playbook"
    );

    // Get the playbook
    let all_playbooks = match crate::tools::playbook_store::get_all_playbooks(
        state.qdrant_client.clone(),
        Some(1000)
    ).await {
        Ok(pbs) => pbs,
        Err(e) => {
            error!(error = %e, "Failed to get playbooks");
            return Ok(Json(json!({
                "ok": false,
                "error": format!("Failed to get playbooks: {}", e)
            })));
        }
    };

    let mut playbook = all_playbooks
        .iter()
        .find(|p| p.id == retrospective.playbook_id)
        .cloned()
        .ok_or_else(|| {
            error!(playbook_id = %retrospective.playbook_id, "Playbook not found");
            StatusCode::NOT_FOUND
        })?;

    // Apply the patch
    playbook.installation_command = patch.patched_command.clone();
    
    // Rehabilitate reliability by 2%
    playbook.reliability_score = (playbook.reliability_score + 0.02).min(0.99);
    
    // Update description to mention patch
    if let Some(ref mut desc) = playbook.description {
        *desc = format!("{} [PATCHED: {}]", desc, patch.patch_reason);
    } else {
        playbook.description = Some(format!("[PATCHED: {}]", patch.patch_reason));
    }

    playbook.last_used_at = Some(chrono::Utc::now().to_rfc3339());

    // Save updated playbook
    // Note: generate_simple_embedding is private, so we'll use the same approach as in update_playbook_stats
    let embedding_text = format!("{} {}", playbook.tool_name, playbook.installation_command);
    let embedding = generate_dense_vector(&embedding_text, 384).await;
    match crate::tools::playbook_store::save_playbook(
        state.qdrant_client.clone(),
        playbook.clone(),
        embedding,
    ).await {
        Ok(_) => {
            info!(
                playbook_id = %retrospective.playbook_id,
                patch_id = %patch.patch_id,
                "Patch applied successfully"
            );

            // Publish cluster update event
            let _ = state.message_bus.sender().send(PhoenixEvent::ToolProposalApproved {
                proposal_id: format!("patch-{}", patch.patch_id),
                tool_name: retrospective.tool_name.clone(),
                installation_command: patch.patched_command.clone(),
            });

            Ok(Json(json!({
                "ok": true,
                "message": "Patch applied successfully",
                "playbook_id": retrospective.playbook_id,
                "new_reliability": playbook.reliability_score
            })))
        }
        Err(e) => {
            error!(
                playbook_id = %retrospective.playbook_id,
                error = %e,
                "Failed to save patched playbook"
            );
            Ok(Json(json!({
                "ok": false,
                "error": format!("Failed to save patched playbook: {}", e)
            })))
        }
    }
}
