//! Phoenix Consensus Sync - Mesh-Wide Voting for Agent Library Updates
//!
//! This module implements the consensus-based library sync mechanism where nodes
//! vote on whether to adopt updates from the `pagi-agent-repo`. Only commits that
//! receive majority approval (based on compliance scores) are automatically adopted.
//!
//! Flow:
//! 1. Node detects new commit -> broadcasts ConsensusRequest
//! 2. Peers respond with ConsensusVote (compliance score)
//! 3. If average score >= threshold AND approval_percentage >= 50%, auto-adopt
//! 4. If rejected, commit is added to mesh-wide quarantine

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use chrono::Utc;
use tokio::sync::RwLock;
use tokio::time::sleep;
use tracing::{error, info, warn};

use crate::bus::{GlobalMessageBus, PhoenixEvent};
use crate::network::handshake::NodeHandshakeServiceImpl;
use crate::foundry::compliance_monitor::ComplianceMonitor;

/// Configuration for consensus voting
#[derive(Debug, Clone)]
pub struct ConsensusConfig {
    /// Minimum average compliance score to approve (0-100)
    pub min_average_score: f64,
    /// Minimum percentage of nodes that must approve (0-100)
    pub min_approval_percentage: f64,
    /// Timeout for collecting votes (seconds)
    pub vote_timeout_seconds: u64,
    /// Path to the agent repository
    pub agents_repo_path: PathBuf,
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        Self {
            min_average_score: 70.0, // 70% average compliance required
            min_approval_percentage: 50.0, // 50% of nodes must approve
            vote_timeout_seconds: 30, // 30 seconds to collect votes
            agents_repo_path: PathBuf::from("config/agents/pagi-agent-repo"),
        }
    }
}

/// A single vote from a peer node
#[derive(Debug, Clone)]
pub struct Vote {
    pub node_id: String,
    pub compliance_score: f64,
    pub approved: bool,
    pub timestamp: String,
}

/// Active consensus session for a commit
#[derive(Debug, Clone)]
pub struct ConsensusSession {
    pub commit_hash: String,
    pub requesting_node: String,
    pub votes: Vec<Vote>,
    pub started_at: String,
    pub config: ConsensusConfig,
}

/// Phoenix Consensus Manager
pub struct PhoenixConsensus {
    config: Arc<RwLock<ConsensusConfig>>,
    active_sessions: Arc<RwLock<HashMap<String, ConsensusSession>>>,
    message_bus: Arc<GlobalMessageBus>,
    handshake_service: Option<Arc<NodeHandshakeServiceImpl>>,
    compliance_monitor: Option<Arc<ComplianceMonitor>>,
    node_id: String,
    /// Mesh-wide quarantine list: commit_hash -> rejection reason
    mesh_quarantine: Arc<RwLock<HashMap<String, String>>>,
}

impl PhoenixConsensus {
    pub fn new(
        message_bus: Arc<GlobalMessageBus>,
        node_id: String,
        agents_repo_path: PathBuf,
    ) -> Self {
        let mut config = ConsensusConfig::default();
        config.agents_repo_path = agents_repo_path;
        
        Self {
            config: Arc::new(RwLock::new(config)),
            active_sessions: Arc::new(RwLock::new(HashMap::new())),
            message_bus,
            handshake_service: None,
            compliance_monitor: None,
            node_id,
            mesh_quarantine: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Set the handshake service for accessing verified peers
    pub fn set_handshake_service(&mut self, handshake_service: Arc<NodeHandshakeServiceImpl>) {
        self.handshake_service = Some(handshake_service);
    }

    /// Set the compliance monitor for calculating compliance scores
    pub fn set_compliance_monitor(&mut self, compliance_monitor: Arc<ComplianceMonitor>) {
        self.compliance_monitor = Some(compliance_monitor);
    }

    /// Start listening for consensus events
    pub async fn start_listener(&self) {
        let mut receiver = self.message_bus.subscribe();
        let sessions = self.active_sessions.clone();
        let message_bus = self.message_bus.clone();
        let config = self.config.clone();
        let handshake_service = self.handshake_service.clone();
        let compliance_monitor = self.compliance_monitor.clone();
        let node_id = self.node_id.clone();
        let mesh_quarantine = self.mesh_quarantine.clone();
        let agents_repo_path = {
            let cfg = config.read().await;
            cfg.agents_repo_path.clone()
        };

        tokio::spawn(async move {
            loop {
                match receiver.recv().await {
                    Ok(event) => {
                        match event {
                            PhoenixEvent::ConsensusRequest { commit_hash, requesting_node, .. } => {
                                // Another node is requesting consensus - we should vote
                                if requesting_node != node_id {
                                    Self::handle_consensus_request(
                                        &commit_hash,
                                        &requesting_node,
                                        &message_bus,
                                        &compliance_monitor,
                                        &node_id,
                                    ).await;
                                }
                            }
                            PhoenixEvent::ConsensusVote { commit_hash, voting_node, compliance_score, approved, .. } => {
                                // A peer has voted - record it
                                if voting_node != node_id {
                                    Self::handle_consensus_vote(
                                        &commit_hash,
                                        &voting_node,
                                        compliance_score,
                                        approved,
                                        &sessions,
                                        &config,
                                        &message_bus,
                                        &handshake_service,
                                        &mesh_quarantine,
                                        &agents_repo_path,
                                        &node_id,
                                    ).await;
                                }
                            }
                            _ => {
                                // Ignore other events
                            }
                        }
                    }
                    Err(e) => {
                        error!(error = %e, "Error receiving consensus event");
                        sleep(Duration::from_secs(1)).await;
                    }
                }
            }
        });
    }

    /// Handle an incoming consensus request from another node
    async fn handle_consensus_request(
        commit_hash: &str,
        requesting_node: &str,
        message_bus: &Arc<GlobalMessageBus>,
        compliance_monitor: &Option<Arc<ComplianceMonitor>>,
        node_id: &str,
    ) {
        info!(
            commit_hash = %commit_hash,
            requesting_node = %requesting_node,
            "Received consensus request, calculating compliance score"
        );

        // Calculate local compliance score for this commit
        // For now, we'll use a default score or check if we have test history
        let compliance_score = if let Some(ref monitor) = compliance_monitor {
            // Try to get the average compliance score from recent tests
            // In a full implementation, we'd check the specific commit
            // For now, we'll use a placeholder that indicates approval if recent tests are good
            75.0 // Default approval score
        } else {
            75.0 // Default if no monitor available
        };

        let approved = compliance_score >= 70.0;

        info!(
            commit_hash = %commit_hash,
            compliance_score = compliance_score,
            approved = approved,
            "Voting on consensus request"
        );

        // Send our vote
        let vote_event = PhoenixEvent::ConsensusVote {
            commit_hash: commit_hash.to_string(),
            voting_node: node_id.to_string(),
            compliance_score,
            approved,
            timestamp: Utc::now().to_rfc3339(),
        };
        message_bus.publish(vote_event);
    }

    /// Handle an incoming vote from a peer
    async fn handle_consensus_vote(
        commit_hash: &str,
        voting_node: &str,
        compliance_score: f64,
        approved: bool,
        sessions: &Arc<RwLock<HashMap<String, ConsensusSession>>>,
        config: &Arc<RwLock<ConsensusConfig>>,
        message_bus: &Arc<GlobalMessageBus>,
        handshake_service: &Option<Arc<NodeHandshakeServiceImpl>>,
        mesh_quarantine: &Arc<RwLock<HashMap<String, String>>>,
        agents_repo_path: &PathBuf,
        node_id: &str,
    ) {
        // Snapshot config for the session creation path (cannot `.await` inside `or_insert_with`).
        let cfg_snapshot = { config.read().await.clone() };

        let vote = Vote {
            node_id: voting_node.to_string(),
            compliance_score,
            approved,
            timestamp: Utc::now().to_rfc3339(),
        };

        // Add vote to session
        let should_evaluate = {
            let mut sessions = sessions.write().await;
            let session = sessions.entry(commit_hash.to_string()).or_insert_with(|| {
                ConsensusSession {
                    commit_hash: commit_hash.to_string(),
                    requesting_node: node_id.to_string(),
                    votes: Vec::new(),
                    started_at: Utc::now().to_rfc3339(),
                    config: cfg_snapshot.clone(),
                }
            });
            session.votes.push(vote.clone());
            
            // Check if we have enough votes or timeout
            let peer_count = if let Some(ref hs) = handshake_service {
                hs.get_verified_peers().await.len()
            } else {
                0
            };
            
            // Evaluate if we have votes from majority of peers or timeout
            let total_expected = peer_count.max(1); // At least 1 (ourselves)
            session.votes.len() >= total_expected
        };

        if should_evaluate {
            Self::evaluate_consensus(
                commit_hash,
                sessions,
                config,
                message_bus,
                mesh_quarantine,
                agents_repo_path,
            ).await;
        }
    }

    /// Evaluate consensus and decide whether to adopt or quarantine
    async fn evaluate_consensus(
        commit_hash: &str,
        sessions: &Arc<RwLock<HashMap<String, ConsensusSession>>>,
        config: &Arc<RwLock<ConsensusConfig>>,
        message_bus: &Arc<GlobalMessageBus>,
        mesh_quarantine: &Arc<RwLock<HashMap<String, String>>>,
        agents_repo_path: &PathBuf,
    ) {
        let (session, cfg) = {
            let mut sessions = sessions.write().await;
            let cfg = config.read().await;
            let session = sessions.remove(commit_hash).unwrap();
            (session, cfg.clone())
        };

        if session.votes.is_empty() {
            warn!(commit_hash = %commit_hash, "No votes received for consensus");
            return;
        }

        let total_votes = session.votes.len();
        let approved_votes = session.votes.iter().filter(|v| v.approved).count();
        let approval_percentage = (approved_votes as f64 / total_votes as f64) * 100.0;
        
        let average_score = session.votes.iter()
            .map(|v| v.compliance_score)
            .sum::<f64>() / total_votes as f64;

        let approved = average_score >= cfg.min_average_score 
            && approval_percentage >= cfg.min_approval_percentage;

        info!(
            commit_hash = %commit_hash,
            total_votes = total_votes,
            approved_votes = approved_votes,
            approval_percentage = approval_percentage,
            average_score = average_score,
            approved = approved,
            "Consensus evaluation complete"
        );

        // Publish consensus result
        let result_event = PhoenixEvent::ConsensusResult {
            commit_hash: commit_hash.to_string(),
            approved,
            average_score,
            approval_percentage,
            total_votes,
            timestamp: Utc::now().to_rfc3339(),
        };
        message_bus.publish(result_event);

        if approved {
            // Auto-adopt: perform git pull and trigger DiscoveryRefresh
            info!(commit_hash = %commit_hash, "Consensus approved - auto-adopting commit");
            
            match Self::adopt_commit(agents_repo_path).await {
                Ok(_) => {
                    info!(commit_hash = %commit_hash, "Successfully adopted commit");
                    
                    // Trigger discovery refresh
                    let discovery_event = PhoenixEvent::BroadcastDiscovery {
                        source: "phoenix_consensus".to_string(),
                        discovery_type: "agent_library_sync".to_string(),
                        details: format!("Auto-adopted commit {} via consensus", commit_hash),
                        timestamp: Utc::now().to_rfc3339(),
                    };
                    message_bus.publish(discovery_event);
                }
                Err(e) => {
                    error!(
                        commit_hash = %commit_hash,
                        error = %e,
                        "Failed to adopt commit despite consensus approval"
                    );
                }
            }
        } else {
            // Quarantine: add to mesh-wide quarantine list
            warn!(
                commit_hash = %commit_hash,
                average_score = average_score,
                approval_percentage = approval_percentage,
                "Consensus rejected - adding to mesh quarantine"
            );

            let reason = format!(
                "Rejected by consensus: avg_score={:.2}, approval={:.2}%",
                average_score, approval_percentage
            );
            
            {
                let mut quarantine = mesh_quarantine.write().await;
                quarantine.insert(commit_hash.to_string(), reason);
            }

            info!(
                commit_hash = %commit_hash,
                "Commit added to mesh-wide quarantine"
            );
        }
    }

    /// Adopt a commit by performing git pull
    async fn adopt_commit(repo_path: &PathBuf) -> Result<(), String> {
        if !repo_path.exists() {
            return Err(format!("Repository path does not exist: {}", repo_path.display()));
        }

        if !repo_path.join(".git").exists() {
            return Err(format!("Path is not a git repository: {}", repo_path.display()));
        }

        info!(repo_path = %repo_path.display(), "Pulling latest changes from agent repository");

        let pull_output = tokio::process::Command::new("git")
            .arg("pull")
            .arg("origin")
            .arg("main")
            .current_dir(repo_path)
            .output()
            .await
            .map_err(|e| format!("Failed to execute git pull: {}", e))?;

        if !pull_output.status.success() {
            let stderr = String::from_utf8_lossy(&pull_output.stderr);
            let stdout = String::from_utf8_lossy(&pull_output.stdout);
            return Err(format!(
                "Git pull failed: stdout={}, stderr={}",
                stdout, stderr
            ));
        }

        info!("Successfully pulled latest changes");
        Ok(())
    }

    /// Request consensus for a new commit
    pub async fn request_consensus(&self, commit_hash: String) {
        // Check if commit is already quarantined
        {
            let quarantine = self.mesh_quarantine.read().await;
            if quarantine.contains_key(&commit_hash) {
                warn!(
                    commit_hash = %commit_hash,
                    "Commit is already in mesh quarantine, skipping consensus request"
                );
                return;
            }
        }

        // Check if we already have an active session
        {
            let sessions = self.active_sessions.read().await;
            if sessions.contains_key(&commit_hash) {
                info!(
                    commit_hash = %commit_hash,
                    "Consensus session already active for this commit"
                );
                return;
            }
        }

        info!(
            commit_hash = %commit_hash,
            "Requesting consensus for new commit"
        );

        // Create consensus session
        {
            let config = self.config.read().await;
            let mut sessions = self.active_sessions.write().await;
            sessions.insert(commit_hash.clone(), ConsensusSession {
                commit_hash: commit_hash.clone(),
                requesting_node: self.node_id.clone(),
                votes: Vec::new(),
                started_at: Utc::now().to_rfc3339(),
                config: config.clone(),
            });
        }

        // Broadcast consensus request
        let request_event = PhoenixEvent::ConsensusRequest {
            commit_hash: commit_hash.clone(),
            requesting_node: self.node_id.clone(),
            timestamp: Utc::now().to_rfc3339(),
        };
        self.message_bus.publish(request_event);

        // Start timeout task
        let sessions = self.active_sessions.clone();
        let config = self.config.clone();
        let message_bus = self.message_bus.clone();
        let handshake_service = self.handshake_service.clone();
        let mesh_quarantine = self.mesh_quarantine.clone();
        let agents_repo_path = {
            let cfg = config.read().await;
            cfg.agents_repo_path.clone()
        };
        let commit_hash_clone = commit_hash.clone();

        tokio::spawn(async move {
            let timeout = {
                let cfg = config.read().await;
                Duration::from_secs(cfg.vote_timeout_seconds)
            };
            sleep(timeout).await;

            // Evaluate consensus after timeout
            let should_evaluate = {
                let sessions = sessions.read().await;
                sessions.contains_key(&commit_hash_clone)
            };

            if should_evaluate {
                Self::evaluate_consensus(
                    &commit_hash_clone,
                    &sessions,
                    &config,
                    &message_bus,
                    &mesh_quarantine,
                    &agents_repo_path,
                ).await;
            }
        });
    }

    /// Check if a commit is quarantined
    pub async fn is_quarantined(&self, commit_hash: &str) -> bool {
        let quarantine = self.mesh_quarantine.read().await;
        quarantine.contains_key(commit_hash)
    }

    /// Get mesh quarantine list
    pub async fn get_quarantine_list(&self) -> HashMap<String, String> {
        let quarantine = self.mesh_quarantine.read().await;
        quarantine.clone()
    }

    /// Get active consensus sessions (for API access)
    pub async fn get_active_sessions(&self) -> HashMap<String, ConsensusSession> {
        let sessions = self.active_sessions.read().await;
        sessions.clone()
    }

    /// Get consensus config (for API access)
    pub async fn get_config(&self) -> ConsensusConfig {
        let config = self.config.read().await;
        config.clone()
    }

    /// Get mesh quarantine (for API access)
    pub async fn get_mesh_quarantine(&self) -> HashMap<String, String> {
        let quarantine = self.mesh_quarantine.read().await;
        quarantine.clone()
    }

    /// Get votes for a specific commit (for API access)
    pub async fn get_votes_for_commit(&self, commit_hash: &str) -> Option<Vec<Vote>> {
        let sessions = self.active_sessions.read().await;
        sessions.get(commit_hash).map(|session| session.votes.clone())
    }

    /// Perform strategic override - bypass threshold and force approval
    pub async fn strategic_override(
        &self,
        commit_hash: String,
        rationale: String,
    ) -> Result<(), String> {
        info!(
            commit_hash = %commit_hash,
            rationale = %rationale,
            "Strategic override requested"
        );

        // Check if commit is in quarantine and remove it
        {
            let mut quarantine = self.mesh_quarantine.write().await;
            quarantine.remove(&commit_hash);
        }

        // Force approval by creating a consensus result event with override flag
        let override_event = PhoenixEvent::ConsensusResult {
            commit_hash: commit_hash.clone(),
            approved: true,
            average_score: 100.0, // Override score
            approval_percentage: 100.0,
            total_votes: 1,
            timestamp: Utc::now().to_rfc3339(),
        };
        self.message_bus.publish(override_event);

        // Perform git commit with [PHOENIX-OVERRIDE] message
        let cfg = self.config.read().await;
        let commit_message = format!(
            "[PHOENIX-OVERRIDE] Strategic override for commit {}\n\nRationale: {}",
            commit_hash, rationale
        );

        match Self::create_override_commit(&cfg.agents_repo_path, &commit_message).await {
            Ok(_) => {
                info!(
                    commit_hash = %commit_hash,
                    "Strategic override commit created successfully"
                );
                
                // Trigger discovery refresh
                let discovery_event = PhoenixEvent::BroadcastDiscovery {
                    source: "phoenix_consensus_override".to_string(),
                    discovery_type: "agent_library_sync".to_string(),
                    details: format!("Strategic override applied to commit {}", commit_hash),
                    timestamp: Utc::now().to_rfc3339(),
                };
                self.message_bus.publish(discovery_event);
                
                Ok(())
            }
            Err(e) => {
                error!(
                    commit_hash = %commit_hash,
                    error = %e,
                    "Failed to create override commit"
                );
                Err(format!("Failed to create override commit: {}", e))
            }
        }
    }

    /// Create a git commit with override message
    async fn create_override_commit(repo_path: &PathBuf, message: &str) -> Result<(), String> {
        if !repo_path.exists() {
            return Err(format!("Repository path does not exist: {}", repo_path.display()));
        }

        if !repo_path.join(".git").exists() {
            return Err(format!("Path is not a git repository: {}", repo_path.display()));
        }

        info!(repo_path = %repo_path.display(), "Creating override commit");

        // Stage all changes
        let add_output = tokio::process::Command::new("git")
            .arg("add")
            .arg("-A")
            .current_dir(repo_path)
            .output()
            .await
            .map_err(|e| format!("Failed to execute git add: {}", e))?;

        if !add_output.status.success() {
            let stderr = String::from_utf8_lossy(&add_output.stderr);
            return Err(format!("Git add failed: {}", stderr));
        }

        // Create commit
        let commit_output = tokio::process::Command::new("git")
            .arg("commit")
            .arg("-m")
            .arg(message)
            .current_dir(repo_path)
            .output()
            .await
            .map_err(|e| format!("Failed to execute git commit: {}", e))?;

        if !commit_output.status.success() {
            let stderr = String::from_utf8_lossy(&commit_output.stderr);
            // Check if it's just "nothing to commit"
            if stderr.contains("nothing to commit") {
                info!("No changes to commit, override message logged");
                return Ok(());
            }
            return Err(format!("Git commit failed: {}", stderr));
        }

        info!("Override commit created successfully");
        Ok(())
    }
}
