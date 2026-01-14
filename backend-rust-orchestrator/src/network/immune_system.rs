//! Global Immune Response System - P2P Compliance Alert Propagation
//!
//! This module implements the network-wide "immune system" that automatically
//! propagates compliance alerts across the P2P mesh when a node detects an
//! unaligned agent mutation. When Node A triggers an AUTO-ROLLBACK, it broadcasts
//! a ComplianceAlert to all connected peers, who then quarantine the offending
//! manifest hash to prevent its use.
//!
//! Architecture:
//! 1. ComplianceMonitor triggers rollback -> broadcasts ComplianceAlert
//! 2. GlobalImmuneResponse receives alert -> propagates via gRPC to peers
//! 3. Peers receive PropagateQuarantine -> flag manifest as Untrusted
//! 4. AgentForge blocks usage of untrusted manifests

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};
use chrono::Utc;

use crate::bus::{GlobalMessageBus, PhoenixEvent};
use crate::network::handshake::NodeHandshakeServiceImpl;

/// Manifest trust status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestTrustStatus {
    Trusted,
    Untrusted,
    Quarantined,
}

/// Information about a quarantined manifest
#[derive(Debug, Clone)]
pub struct QuarantinedManifest {
    pub manifest_hash: String,
    pub agent_id: String,
    pub quarantined_by: String, // node_id that detected the violation
    pub reason: String,
    pub compliance_score: f64,
    pub timestamp: String,
}

/// Global Immune Response system that tracks untrusted manifests and propagates alerts
pub struct GlobalImmuneResponse {
    /// Map of manifest_hash -> QuarantinedManifest
    quarantined_manifests: Arc<RwLock<HashMap<String, QuarantinedManifest>>>,
    /// Map of agent_id -> manifest_hash (for quick lookup)
    agent_manifest_map: Arc<RwLock<HashMap<String, String>>>,
    /// Reference to message bus for broadcasting alerts
    message_bus: Arc<GlobalMessageBus>,
    /// Reference to handshake service for P2P propagation
    handshake_service: Option<Arc<NodeHandshakeServiceImpl>>,
}

impl GlobalImmuneResponse {
    /// Create a new GlobalImmuneResponse instance
    pub fn new(message_bus: Arc<GlobalMessageBus>) -> Self {
        Self {
            quarantined_manifests: Arc::new(RwLock::new(HashMap::new())),
            agent_manifest_map: Arc::new(RwLock::new(HashMap::new())),
            message_bus,
            handshake_service: None,
        }
    }

    /// Set the handshake service for P2P propagation
    pub fn set_handshake_service(&mut self, handshake_service: Arc<NodeHandshakeServiceImpl>) {
        self.handshake_service = Some(handshake_service);
    }

    /// Handle a compliance alert from the ComplianceMonitor
    /// This is called when an AUTO-ROLLBACK is triggered
    pub async fn handle_compliance_alert(
        &self,
        agent_id: String,
        manifest_hash: String,
        compliance_score: f64,
        node_id: String,
    ) {
        info!(
            agent_id = %agent_id,
            manifest_hash = %manifest_hash,
            score = compliance_score,
            "Handling compliance alert - quarantining manifest"
        );

        // Compute manifest hash if not provided (from agent files)
        let final_manifest_hash = if manifest_hash.is_empty() {
            self.compute_agent_manifest_hash(&agent_id).await
                .unwrap_or_else(|| format!("agent_{}", agent_id))
        } else {
            manifest_hash
        };

        // Store the agent -> manifest mapping
        {
            let mut map = self.agent_manifest_map.write().await;
            map.insert(agent_id.clone(), final_manifest_hash.clone());
        }

        // Quarantine the manifest
        let quarantine = QuarantinedManifest {
            manifest_hash: final_manifest_hash.clone(),
            agent_id: agent_id.clone(),
            quarantined_by: node_id.clone(),
            reason: format!("Compliance score {} below threshold (70%)", compliance_score),
            compliance_score,
            timestamp: Utc::now().to_rfc3339(),
        };

        {
            let mut quarantined = self.quarantined_manifests.write().await;
            quarantined.insert(final_manifest_hash.clone(), quarantine.clone());
        }

        // Broadcast the alert to the message bus
        let event = PhoenixEvent::ComplianceAlert {
            agent_id: agent_id.clone(),
            manifest_hash: final_manifest_hash.clone(),
            compliance_score,
            quarantined_by: node_id.clone(),
            timestamp: Utc::now().to_rfc3339(),
        };
        self.message_bus.publish(event);

        // Propagate to all connected peers via gRPC
        if let Some(ref handshake_service) = self.handshake_service {
            self.propagate_quarantine_to_peers(
                handshake_service,
                &final_manifest_hash,
                &agent_id,
                compliance_score,
                &node_id,
            ).await;
        } else {
            warn!("Handshake service not set - cannot propagate quarantine to peers");
        }
    }

    /// Propagate quarantine alert to all connected peers
    async fn propagate_quarantine_to_peers(
        &self,
        handshake_service: &NodeHandshakeServiceImpl,
        manifest_hash: &str,
        agent_id: &str,
        compliance_score: f64,
        node_id: &str,
    ) {
        let peers = handshake_service.get_verified_peers().await;
        
        if peers.is_empty() {
            info!("No peers connected - quarantine alert not propagated");
            return;
        }

        info!(
            peer_count = peers.len(),
            manifest_hash = %manifest_hash,
            "Propagating quarantine alert to peers"
        );

        // In a full implementation, we would use gRPC to send PropagateQuarantine messages
        // For now, we'll log the intent. The actual gRPC implementation will be added
        // when we update the handshake service.
        for peer in peers {
            info!(
                peer_node_id = %peer.node_id,
                manifest_hash = %manifest_hash,
                "Would send PropagateQuarantine to peer"
            );
        }
    }

    /// Check if a manifest hash is trusted
    pub async fn is_manifest_trusted(&self, manifest_hash: &str) -> ManifestTrustStatus {
        let quarantined = self.quarantined_manifests.read().await;
        if quarantined.contains_key(manifest_hash) {
            ManifestTrustStatus::Untrusted
        } else {
            ManifestTrustStatus::Trusted
        }
    }

    /// Check if an agent's manifest is trusted
    pub async fn is_agent_manifest_trusted(&self, agent_id: &str) -> ManifestTrustStatus {
        let map = self.agent_manifest_map.read().await;
        if let Some(manifest_hash) = map.get(agent_id) {
            let quarantined = self.quarantined_manifests.read().await;
            if quarantined.contains_key(manifest_hash) {
                return ManifestTrustStatus::Untrusted;
            }
        }
        ManifestTrustStatus::Trusted
    }

    /// Get all quarantined manifests
    pub async fn get_quarantined_manifests(&self) -> Vec<QuarantinedManifest> {
        let quarantined = self.quarantined_manifests.read().await;
        quarantined.values().cloned().collect()
    }

    /// Compute manifest hash for an agent (from manifest.yaml + prompt.txt)
    async fn compute_agent_manifest_hash(&self, agent_id: &str) -> Option<String> {
        use sha2::{Sha256, Digest};
        use std::path::PathBuf;

        // Try to find the agent directory
        // This is a simplified version - in production, you'd pass the agents_repo_path
        let possible_paths = vec![
            PathBuf::from("agents").join(agent_id),
            PathBuf::from("test-agent-repo").join("agents").join(agent_id),
        ];

        for agent_dir in possible_paths {
            let manifest_path = agent_dir.join("manifest.yaml");
            let prompt_path = agent_dir.join("prompt.txt");

            if manifest_path.exists() && prompt_path.exists() {
                let mut hasher = Sha256::new();
                
                // Hash manifest.yaml
                if let Ok(manifest_content) = tokio::fs::read_to_string(&manifest_path).await {
                    hasher.update(manifest_content.as_bytes());
                }
                
                // Hash prompt.txt
                if let Ok(prompt_content) = tokio::fs::read_to_string(&prompt_path).await {
                    hasher.update(prompt_content.as_bytes());
                }

                let hash = hasher.finalize();
                return Some(hex::encode(hash));
            }
        }

        None
    }

    /// Handle a PropagateQuarantine message from a peer
    pub async fn handle_peer_quarantine(
        &self,
        manifest_hash: String,
        agent_id: String,
        quarantined_by: String,
        compliance_score: f64,
    ) {
        info!(
            manifest_hash = %manifest_hash,
            agent_id = %agent_id,
            quarantined_by = %quarantined_by,
            "Received quarantine alert from peer"
        );

        // Store the quarantine
        let quarantine = QuarantinedManifest {
            manifest_hash: manifest_hash.clone(),
            agent_id: agent_id.clone(),
            quarantined_by: quarantined_by.clone(),
            reason: format!("Quarantined by peer {} (compliance score: {})", quarantined_by, compliance_score),
            compliance_score,
            timestamp: Utc::now().to_rfc3339(),
        };

        {
            let mut quarantined = self.quarantined_manifests.write().await;
            quarantined.insert(manifest_hash.clone(), quarantine);
        }

        {
            let mut map = self.agent_manifest_map.write().await;
            map.insert(agent_id, manifest_hash);
        }

        // Broadcast the event locally
        let event = PhoenixEvent::ComplianceAlert {
            agent_id,
            manifest_hash,
            compliance_score,
            quarantined_by,
            timestamp: Utc::now().to_rfc3339(),
        };
        self.message_bus.publish(event);
    }
}
