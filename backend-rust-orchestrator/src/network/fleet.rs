//! Phoenix Fleet Manager - Distributed Node Registry
//!
//! This module provides fleet-wide node tracking, health monitoring, and
//! cross-node knowledge sharing for the Phoenix AGI cluster.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error};

/// Node status in the fleet
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NodeStatus {
    /// Node is healthy and operational
    Nominal,
    /// Node has drifted from expected state (failed verification)
    InDrift,
    /// Node is in repair mode (self-correcting)
    InRepair,
    /// Node has not sent heartbeat recently (stale)
    Stale,
    /// Node is offline or unreachable
    Offline,
}

/// Information about a fleet node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub node_id: String,
    pub hostname: String,
    pub ip_address: String,
    pub status: NodeStatus,
    pub last_heartbeat: DateTime<Utc>,
    pub last_audit_timestamp: Option<DateTime<Utc>>,
    pub software_version: Option<String>,
    pub registered_at: DateTime<Utc>,
}

/// Fleet state manager
#[derive(Clone)]
pub struct FleetState {
    nodes: Arc<RwLock<HashMap<String, Node>>>,
    /// Timeout in seconds for considering a node stale (default: 60 seconds)
    heartbeat_timeout_secs: u64,
}

impl FleetState {
    /// Create a new FleetState
    pub fn new(heartbeat_timeout_secs: Option<u64>) -> Self {
        Self {
            nodes: Arc::new(RwLock::new(HashMap::new())),
            heartbeat_timeout_secs: heartbeat_timeout_secs.unwrap_or(60),
        }
    }

    /// Register or update a node heartbeat
    pub async fn heartbeat(&self, node_id: String, hostname: String, ip_address: String, software_version: Option<String>) -> Result<(), String> {
        let mut nodes = self.nodes.write().await;
        let now = Utc::now();
        
        let node = nodes.entry(node_id.clone()).or_insert_with(|| {
            info!(
                node_id = %node_id,
                hostname = %hostname,
                ip_address = %ip_address,
                "New node registered in fleet"
            );
            Node {
                node_id: node_id.clone(),
                hostname: hostname.clone(),
                ip_address: ip_address.clone(),
                status: NodeStatus::Nominal,
                last_heartbeat: now,
                last_audit_timestamp: None,
                software_version: software_version.clone(),
                registered_at: now,
            }
        });

        // Update existing node
        node.last_heartbeat = now;
        node.hostname = hostname;
        node.ip_address = ip_address;
        if let Some(version) = software_version {
            node.software_version = Some(version);
        }

        // Update status based on heartbeat freshness
        if node.status == NodeStatus::Offline || node.status == NodeStatus::Stale {
            info!(
                node_id = %node_id,
                "Node recovered from stale/offline status"
            );
            node.status = NodeStatus::Nominal;
        }

        Ok(())
    }

    /// Update node status (e.g., when entering repair mode)
    pub async fn update_node_status(&self, node_id: &str, status: NodeStatus) -> Result<(), String> {
        let mut nodes = self.nodes.write().await;
        
        if let Some(node) = nodes.get_mut(node_id) {
            let old_status = node.status.clone();
            node.status = status.clone();
            
            info!(
                node_id = %node_id,
                old_status = ?old_status,
                new_status = ?status,
                "Node status updated"
            );
            Ok(())
        } else {
            Err(format!("Node {} not found in fleet", node_id))
        }
    }

    /// Update last audit timestamp for a node
    pub async fn update_audit_timestamp(&self, node_id: &str, timestamp: DateTime<Utc>) -> Result<(), String> {
        let mut nodes = self.nodes.write().await;
        
        if let Some(node) = nodes.get_mut(node_id) {
            node.last_audit_timestamp = Some(timestamp);
            Ok(())
        } else {
            Err(format!("Node {} not found in fleet", node_id))
        }
    }

    /// Get all nodes in the fleet
    pub async fn list_nodes(&self) -> Vec<Node> {
        let nodes = self.nodes.read().await;
        let mut node_list: Vec<Node> = nodes.values().cloned().collect();
        
        // Sort by hostname for consistent ordering
        node_list.sort_by(|a, b| a.hostname.cmp(&b.hostname));
        node_list
    }

    /// Get a specific node by ID
    pub async fn get_node(&self, node_id: &str) -> Option<Node> {
        let nodes = self.nodes.read().await;
        nodes.get(node_id).cloned()
    }

    /// Get fleet health summary
    pub async fn get_fleet_health(&self) -> FleetHealth {
        let nodes = self.nodes.read().await;
        let now = Utc::now();
        let timeout_duration = chrono::Duration::seconds(self.heartbeat_timeout_secs as i64);

        let mut health = FleetHealth {
            total_nodes: nodes.len(),
            nominal: 0,
            in_drift: 0,
            in_repair: 0,
            stale: 0,
            offline: 0,
        };

        for node in nodes.values() {
            let time_since_heartbeat = now - node.last_heartbeat;
            
            // Update status based on heartbeat freshness
            let effective_status = if time_since_heartbeat > timeout_duration {
                if node.status == NodeStatus::Offline {
                    &NodeStatus::Offline
                } else {
                    &NodeStatus::Stale
                }
            } else {
                &node.status
            };

            match effective_status {
                NodeStatus::Nominal => health.nominal += 1,
                NodeStatus::InDrift => health.in_drift += 1,
                NodeStatus::InRepair => health.in_repair += 1,
                NodeStatus::Stale => health.stale += 1,
                NodeStatus::Offline => health.offline += 1,
            }
        }

        health
    }

    /// Clean up stale nodes (remove nodes that haven't sent heartbeat in a long time)
    pub async fn cleanup_stale_nodes(&self, max_age_secs: u64) -> usize {
        let mut nodes = self.nodes.write().await;
        let now = Utc::now();
        let max_age = chrono::Duration::seconds(max_age_secs as i64);
        
        let mut removed = 0;
        nodes.retain(|node_id, node| {
            let age = now - node.last_heartbeat;
            if age > max_age {
                warn!(
                    node_id = %node_id,
                    age_secs = age.num_seconds(),
                    "Removing stale node from fleet"
                );
                removed += 1;
                false
            } else {
                true
            }
        });

        removed
    }
}

/// Fleet health summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FleetHealth {
    pub total_nodes: usize,
    pub nominal: usize,
    pub in_drift: usize,
    pub in_repair: usize,
    pub stale: usize,
    pub offline: usize,
}

/// Heartbeat request from a node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatRequest {
    pub node_id: String,
    pub hostname: String,
    pub ip_address: String,
    pub software_version: Option<String>,
}

/// Heartbeat response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatResponse {
    pub success: bool,
    pub message: String,
    pub fleet_size: usize,
}
