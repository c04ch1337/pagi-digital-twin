use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};
use chrono::{DateTime, Utc};

use super::forge_api::ComplianceResult;
use crate::bus::GlobalMessageBus;
use crate::network::immune_system::GlobalImmuneResponse;

/// Configuration for auto-rollback behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoRollbackConfig {
    /// Enable/disable auto-rollback
    pub enabled: bool,
    /// Compliance score threshold (0-100). Rollback triggers if score < threshold
    pub threshold: f64,
    /// Minimum number of failed tests before rollback (prevents false positives)
    pub min_failures: usize,
}

impl Default for AutoRollbackConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold: 70.0, // 70% compliance required
            min_failures: 1, // Rollback on first failure below threshold
        }
    }
}

/// A single compliance test result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceTestRecord {
    pub agent_id: String,
    pub test_id: String,
    pub timestamp: DateTime<Utc>,
    pub compliance: ComplianceResult,
    pub score: f64,
    pub mission: String,
    pub rolled_back: bool,
}

/// Compliance monitor service that tracks agent compliance and triggers auto-rollback
pub struct ComplianceMonitor {
    config: Arc<RwLock<AutoRollbackConfig>>,
    test_history: Arc<RwLock<HashMap<String, Vec<ComplianceTestRecord>>>>, // agent_id -> test records
    agents_repo_path: PathBuf,
    message_bus: Option<Arc<GlobalMessageBus>>,
    immune_response: Option<Arc<GlobalImmuneResponse>>,
    node_id: String,
}

impl ComplianceMonitor {
    pub fn new(agents_repo_path: PathBuf) -> Self {
        Self {
            config: Arc::new(RwLock::new(AutoRollbackConfig::default())),
            test_history: Arc::new(RwLock::new(HashMap::new())),
            agents_repo_path,
            message_bus: None,
            immune_response: None,
            node_id: "local".to_string(),
        }
    }

    /// Set the message bus for broadcasting compliance alerts
    pub fn set_message_bus(&mut self, message_bus: Arc<GlobalMessageBus>) {
        self.message_bus = Some(message_bus);
    }

    /// Set the immune response system for P2P propagation
    pub fn set_immune_response(&mut self, immune_response: Arc<GlobalImmuneResponse>) {
        self.immune_response = Some(immune_response);
    }

    /// Set the node ID for identifying this node in alerts
    pub fn set_node_id(&mut self, node_id: String) {
        self.node_id = node_id;
    }

    /// Calculate compliance score from a ComplianceResult (0-100)
    pub fn calculate_score(compliance: &ComplianceResult) -> f64 {
        let checks = vec![
            compliance.privacy.passed,
            compliance.efficiency.passed,
            compliance.tone.passed,
        ];
        
        let passed = checks.iter().filter(|&&p| p).count();
        (passed as f64 / checks.len() as f64) * 100.0
    }

    /// Record a compliance test result and check if rollback is needed
    pub async fn record_test(
        &self,
        agent_id: String,
        mission: String,
        compliance: ComplianceResult,
    ) -> Result<Option<String>, String> {
        let score = Self::calculate_score(&compliance);
        let test_id = format!("test_{}", Utc::now().timestamp_millis());
        
        let record = ComplianceTestRecord {
            agent_id: agent_id.clone(),
            test_id: test_id.clone(),
            timestamp: Utc::now(),
            compliance: compliance.clone(),
            score,
            mission,
            rolled_back: false,
        };

        // Store the test record
        {
            let mut history = self.test_history.write().await;
            history
                .entry(agent_id.clone())
                .or_insert_with(Vec::new)
                .push(record.clone());
        }

        info!(
            agent_id = %agent_id,
            test_id = %test_id,
            score = score,
            "Compliance test recorded"
        );

        // Check if rollback is needed
        let config = self.config.read().await;
        if !config.enabled {
            return Ok(None);
        }

        if score < config.threshold {
            warn!(
                agent_id = %agent_id,
                score = score,
                threshold = config.threshold,
                "Compliance score below threshold, checking rollback conditions"
            );

            // Count recent failures
            let recent_failures = self.count_recent_failures(&agent_id, config.min_failures).await;
            
            if recent_failures >= config.min_failures {
                info!(
                    agent_id = %agent_id,
                    failures = recent_failures,
                    "Triggering auto-rollback"
                );
                
                // Trigger rollback
                match self.trigger_rollback(&agent_id, score).await {
                    Ok(commit_hash) => {
                        // Mark this test as rolled back
                        let mut history = self.test_history.write().await;
                        if let Some(records) = history.get_mut(&agent_id) {
                            if let Some(record) = records.iter_mut().find(|r| r.test_id == test_id) {
                                record.rolled_back = true;
                            }
                        }
                        
                        Ok(Some(commit_hash))
                    }
                    Err(e) => {
                        error!(
                            agent_id = %agent_id,
                            error = %e,
                            "Failed to trigger auto-rollback"
                        );
                        Err(format!("Auto-rollback failed: {}", e))
                    }
                }
            } else {
                info!(
                    agent_id = %agent_id,
                    recent_failures = recent_failures,
                    min_failures = config.min_failures,
                    "Not enough failures to trigger rollback"
                );
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    /// Count recent failures for an agent (within last 10 tests)
    async fn count_recent_failures(&self, agent_id: &str, min_failures: usize) -> usize {
        let history = self.test_history.read().await;
        let config = self.config.read().await;
        
        if let Some(records) = history.get(agent_id) {
            records
                .iter()
                .rev()
                .take(10) // Check last 10 tests
                .filter(|r| r.score < config.threshold)
                .count()
        } else {
            0
        }
    }

    /// Trigger automatic rollback to the last known good commit
    async fn trigger_rollback(&self, agent_id: &str, compliance_score: f64) -> Result<String, String> {
        use git2::Repository;
        use std::str::FromStr;
        use sha2::{Sha256, Digest};
        
        let repo = Repository::open(&self.agents_repo_path)
            .map_err(|e| format!("Failed to open repository: {}", e))?;

        // Find the last commit that had a passing compliance score
        let history = self.test_history.read().await;
        let config = self.config.read().await;
        
        // Get agent history from git
        let mut revwalk = repo.revwalk()
            .map_err(|e| format!("Failed to create revwalk: {}", e))?;
        revwalk.push_head()
            .map_err(|e| format!("Failed to push head: {}", e))?;

        // Find the most recent commit with passing compliance
        let mut last_good_commit: Option<String> = None;
        
        for oid in revwalk {
            let oid = oid.map_err(|e| format!("Failed to get oid: {}", e))?;
            let commit = repo.find_commit(oid)
                .map_err(|e| format!("Failed to find commit: {}", e))?;
            
            let commit_hash = oid.to_string();
            
            // Check if we have test records for this commit
            // For now, we'll rollback to the previous commit (HEAD~1)
            // In a full implementation, we'd track which commit each test was run against
            if let Some(records) = history.get(agent_id) {
                // Find records that might correspond to this commit
                // This is a simplified check - in production, you'd track commit hashes with tests
                let has_passing = records.iter().any(|r| r.score >= config.threshold);
                if has_passing {
                    last_good_commit = Some(commit_hash);
                    break;
                }
            }
        }

        // If no good commit found, rollback to HEAD~1 (previous commit)
        let target_commit = last_good_commit.unwrap_or_else(|| {
            // Get HEAD~1
            if let Ok(head) = repo.head() {
                if let Ok(commit) = head.peel_to_commit() {
                    if let Ok(parent) = commit.parent(0) {
                        return parent.id().to_string();
                    }
                }
            }
            // Fallback: use current HEAD (shouldn't happen)
            repo.head().and_then(|h| h.peel_to_commit().map(|c| c.id().to_string()))
                .unwrap_or_else(|_| "HEAD".to_string())
        });

        info!(
            agent_id = %agent_id,
            target_commit = %target_commit,
            "Rolling back agent to commit"
        );

        // Compute manifest hash for the agent (from manifest.yaml + prompt.txt)
        let manifest_hash = {
            let agent_dir = self.agents_repo_path.join("agents").join(agent_id);
            let manifest_path = agent_dir.join("manifest.yaml");
            let prompt_path = agent_dir.join("prompt.txt");
            
            let mut hasher = Sha256::new();
            if let Ok(manifest_content) = std::fs::read_to_string(&manifest_path) {
                hasher.update(manifest_content.as_bytes());
            }
            if let Ok(prompt_content) = std::fs::read_to_string(&prompt_path) {
                hasher.update(prompt_content.as_bytes());
            }
            hex::encode(hasher.finalize())
        };
        
        let oid = git2::Oid::from_str(&target_commit)
            .map_err(|e| format!("Invalid commit hash: {}", e))?;
        
        let commit = repo.find_commit(oid)
            .map_err(|e| format!("Commit not found: {}", e))?;
        
        let tree = commit.tree()
            .map_err(|e| format!("Failed to get tree: {}", e))?;
        
        let mut checkout_builder = git2::build::CheckoutBuilder::new();
        checkout_builder.path(format!("agents/{}/", agent_id));
        checkout_builder.force();
        
        repo.checkout_tree(tree.as_object(), Some(&mut checkout_builder))
            .map_err(|e| format!("Failed to checkout tree: {}", e))?;
        
        let signature = repo.signature()
            .map_err(|e| format!("Failed to get signature: {}", e))?;
        
        let parent_commit = repo.head()
            .ok()
            .and_then(|h| h.peel_to_commit().ok())
            .ok_or_else(|| "No HEAD commit".to_string())?;
        
        let tree_id = repo.index()
            .ok()
            .and_then(|mut idx| {
                idx.add_path(&std::path::PathBuf::from(format!("agents/{}/", agent_id))).ok()?;
                idx.write_tree().ok()
            })
            .ok_or_else(|| "Failed to write tree".to_string())?;
        
        let new_tree = repo.find_tree(tree_id)
            .map_err(|e| format!("Failed to find tree: {}", e))?;
        
        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            &format!("[AUTO-ROLLBACK] Agent {} compliance score below threshold", agent_id),
            &new_tree,
            &[&parent_commit],
        ).map_err(|e| format!("Failed to create revert commit: {}", e))?;
        
        info!(
            agent_id = %agent_id,
            commit_hash = %target_commit,
            "Auto-rollback completed successfully"
        );

        // Trigger global immune response - broadcast compliance alert
        if let Some(ref immune_response) = self.immune_response {
            immune_response.handle_compliance_alert(
                agent_id.to_string(),
                manifest_hash,
                compliance_score,
                self.node_id.clone(),
            ).await;
        } else if let Some(ref message_bus) = self.message_bus {
            // Fallback: just broadcast to message bus if immune response not available
            use crate::bus::PhoenixEvent;
            let event = PhoenixEvent::ComplianceAlert {
                agent_id: agent_id.to_string(),
                manifest_hash,
                compliance_score,
                quarantined_by: self.node_id.clone(),
                timestamp: Utc::now().to_rfc3339(),
            };
            message_bus.publish(event);
        }
        
        Ok(target_commit)
    }

    /// Get compliance history for an agent
    pub async fn get_agent_history(&self, agent_id: &str) -> Vec<ComplianceTestRecord> {
        let history = self.test_history.read().await;
        history.get(agent_id).cloned().unwrap_or_default()
    }

    /// Get current configuration
    pub async fn get_config(&self) -> AutoRollbackConfig {
        self.config.read().await.clone()
    }

    /// Update configuration
    pub async fn update_config(&self, config: AutoRollbackConfig) {
        *self.config.write().await = config;
        info!("Auto-rollback configuration updated");
    }

    /// Get compliance statistics for an agent
    pub async fn get_agent_stats(&self, agent_id: &str) -> Option<ComplianceStats> {
        let history = self.test_history.read().await;
        let records = history.get(agent_id)?;
        
        if records.is_empty() {
            return None;
        }

        let total_tests = records.len();
        let passed_tests = records.iter().filter(|r| r.score >= 70.0).count();
        let avg_score = records.iter().map(|r| r.score).sum::<f64>() / total_tests as f64;
        let rollbacks = records.iter().filter(|r| r.rolled_back).count();
        let recent_score = records.last().map(|r| r.score).unwrap_or(0.0);

        Some(ComplianceStats {
            total_tests,
            passed_tests,
            failed_tests: total_tests - passed_tests,
            avg_score,
            recent_score,
            rollbacks,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceStats {
    pub total_tests: usize,
    pub passed_tests: usize,
    pub failed_tests: usize,
    pub avg_score: f64,
    pub recent_score: f64,
    pub rollbacks: usize,
}
