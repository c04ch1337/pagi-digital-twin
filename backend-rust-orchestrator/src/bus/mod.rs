use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{error, info, warn};

/// Global event bus for inter-agent communication and system-wide events.
pub struct GlobalMessageBus {
    sender: broadcast::Sender<PhoenixEvent>,
}

/// Events that can be broadcast across the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PhoenixEvent {
    /// Task update from an agent
    TaskUpdate {
        agent_id: String,
        task: String,
        status: String,
        timestamp: String,
    },
    /// Resource warning when an agent exceeds limits
    ResourceWarning {
        agent_id: String,
        agent_name: String,
        resource_type: String, // "memory" or "cpu"
        current_value: String,
        threshold: String,
        timestamp: String,
    },
    /// Agent handshake/registration event
    AgentHandshake {
        agent_id: String,
        agent_name: String,
        mission: String,
        timestamp: String,
    },
    /// Broadcast discovery of new resources or capabilities
    BroadcastDiscovery {
        source: String,
        discovery_type: String,
        details: String,
        timestamp: String,
    },
    /// Maintenance operation started (e.g., memory pruning)
    MaintenanceStarted {
        operation: String,
        timestamp: String,
    },
    /// Indexing/reindexing operation completed
    IndexingComplete {
        collections: Vec<String>,
        timestamp: String,
    },
    /// Unauthorized node detected during handshake
    UnauthorizedNodeDetected {
        node_id: String,
        reason: String, // "manifest_mismatch" | "alignment_token_mismatch" | "signature_invalid" | "nonce_expired"
        remote_address: String,
        timestamp: String,
    },
    /// Node has been isolated/quarantined
    NodeIsolated {
        node_id: String,
        reason: String,
        timestamp: String,
    },
    /// Node has been re-integrated from quarantine
    NodeReintegrated {
        node_id: String,
        timestamp: String,
    },
    /// New peer discovered and verified
    PeerVerified {
        node_id: String,
        software_version: String,
        manifest_hash: String,
        remote_address: String,
        timestamp: String,
    },
    /// Node discovered via mDNS (before handshake)
    NodeDiscovered {
        ip: String,
        node_id: String,
        timestamp: String,
    },
    /// Compliance alert when an agent fails compliance and triggers rollback
    /// Compliance alert when an agent fails compliance and triggers rollback
    ComplianceAlert {
        agent_id: String,
        manifest_hash: String,
        compliance_score: f64,
        quarantined_by: String, // node_id that detected the violation
        timestamp: String,
    },
    /// Consensus request for a new commit in pagi-agent-repo
    ConsensusRequest {
        commit_hash: String,
        requesting_node: String,
        timestamp: String,
    },
    /// Vote from a peer node on a commit
    ConsensusVote {
        commit_hash: String,
        voting_node: String,
        compliance_score: f64, // Local War Room compliance score for this commit
        approved: bool,
        timestamp: String,
    },
    /// Consensus result - commit approved or rejected by mesh
    ConsensusResult {
        commit_hash: String,
        approved: bool,
        average_score: f64,
        approval_percentage: f64, // Percentage of nodes that approved
        total_votes: usize,
        timestamp: String,
    },
    /// Memory exchange request via PhoenixEvent bus
    MemoryExchangeRequest {
        requesting_node: String,
        topic: String,
        namespace: String,
        timestamp: String,
    },
    /// Memory transfer event - indicates knowledge flowing between nodes
    MemoryTransfer {
        source_node: String,
        destination_node: String,
        topic: String,
        fragments_count: usize,
        bytes_transferred: u64,
        redacted_entities_count: usize,
        timestamp: String,
    },
    /// Quarantine alert - commit or node has been quarantined
    QuarantineAlert {
        entity_type: String, // "commit" or "node"
        entity_id: String,
        reason: String,
        quarantined_by: String,
        timestamp: String,
    },
    /// Memory prune event - topic has been pruned from collections
    MemoryPrune {
        topic: String,
        deleted_count: usize,
        timestamp: String,
    },
    /// Configuration update event - propagates new guardrail rules to mesh peers
    UpdateConfig {
        rule_id: String,
        rule_type: String, // "python_regex", "rust_filter", "config_update"
        config_data: String, // JSON-encoded configuration data
        applied_by: String, // node_id that applied the rule
        timestamp: String,
    },
    /// Tool installation proposal created by an agent (e.g., Phoenix Auditor)
    ToolProposalCreated {
        proposal_id: String,
        agent_name: String,
        tool_name: String,
    },
    /// Tool installation proposal approved by human
    ToolProposalApproved {
        proposal_id: String,
        tool_name: String,
        installation_command: String,
    },
    /// Tool installation proposal rejected by human
    ToolProposalRejected {
        proposal_id: String,
        tool_name: String,
    },
    /// Peer review request - agent requesting review from expert
    PeerReviewRequest {
        review_id: String,
        requesting_agent_id: String,
        requesting_agent_name: String,
        expert_agent_id: String,
        expert_agent_name: String,
        tool_proposal_id: String,
        tool_name: String,
        github_url: String,
        reasoning: String,
        timestamp: String,
    },
    /// Peer review response - expert agent's review
    PeerReviewResponse {
        review_id: String,
        expert_agent_id: String,
        expert_agent_name: String,
        decision: String, // "concur" or "object"
        reasoning: String,
        alternative_playbook_id: Option<String>,
        timestamp: String,
    },
    /// Peer review debate completed - consensus reached
    PeerReviewConsensus {
        review_id: String,
        tool_proposal_id: String,
        consensus: String, // "approved" or "rejected"
        requesting_agent_id: String,
        expert_agent_id: String,
        timestamp: String,
    },
    /// Post-mortem retrospective analysis for failed tool installation
    PostMortemRetrospective {
        retrospective_id: String,
        playbook_id: String,
        tool_name: String,
        agent_id: String,
        agent_name: String,
        root_cause: String,
        error_pattern: String,
        suggested_patch: Option<String>, // JSON-encoded PlaybookPatch
        reliability_impact: f64,
        timestamp: String,
    },
}

impl GlobalMessageBus {
    /// Create a new global message bus with a default channel capacity of 1000.
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(1000);
        Self { sender }
    }

    /// Publish an event to the bus. Returns the number of active subscribers.
    pub fn publish(&self, event: PhoenixEvent) -> usize {
        match self.sender.send(event) {
            Ok(count) => {
                info!("Event published to {} subscribers", count);
                count
            }
            Err(e) => {
                error!("Failed to publish event: {}", e);
                0
            }
        }
    }

    /// Subscribe to the message bus. Returns a receiver that can be used to listen for events.
    pub fn subscribe(&self) -> broadcast::Receiver<PhoenixEvent> {
        self.sender.subscribe()
    }

    /// Get a reference to the sender (useful for cloning into async tasks).
    pub fn sender(&self) -> broadcast::Sender<PhoenixEvent> {
        self.sender.clone()
    }
}

impl Default for GlobalMessageBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_message_bus_publish_subscribe() {
        let bus = GlobalMessageBus::new();
        let mut receiver = bus.subscribe();

        let event = PhoenixEvent::TaskUpdate {
            agent_id: "test-agent".to_string(),
            task: "test task".to_string(),
            status: "completed".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        };

        bus.publish(event.clone());
        
        let received = receiver.recv().await.unwrap();
        match (event, received) {
            (
                PhoenixEvent::TaskUpdate { agent_id: id1, .. },
                PhoenixEvent::TaskUpdate { agent_id: id2, .. },
            ) => assert_eq!(id1, id2),
            _ => panic!("Event type mismatch"),
        }
    }
}
