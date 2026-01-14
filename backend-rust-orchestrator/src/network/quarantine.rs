//! Node Quarantine System - Safety Guard for Network Isolation
//!
//! This module manages the quarantine list of nodes that have failed handshakes
//! or violated alignment tokens. Quarantined nodes are immediately rejected
//! from all gRPC requests.

use std::collections::HashMap;
use std::sync::Arc;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{info, warn, error};

use crate::bus::{GlobalMessageBus, PhoenixEvent};

/// Quarantine entry for a node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuarantineEntry {
    pub node_id: String,
    pub ip_address: Option<String>,
    pub reason: String,
    pub timestamp: String,
    pub quarantined_by: String, // Node ID that quarantined this node
}

/// Quarantine manager
pub struct QuarantineManager {
    /// Map of node_id -> QuarantineEntry
    quarantined_nodes: Arc<RwLock<HashMap<String, QuarantineEntry>>>,
    /// Map of ip_address -> node_id (for quick IP lookups)
    quarantined_ips: Arc<RwLock<HashMap<String, String>>>,
    message_bus: Arc<GlobalMessageBus>,
    qdrant_client: Option<Arc<qdrant_client::Qdrant>>,
}

impl QuarantineManager {
    pub fn new(
        message_bus: Arc<GlobalMessageBus>,
        qdrant_client: Option<Arc<qdrant_client::Qdrant>>,
    ) -> Self {
        Self {
            quarantined_nodes: Arc::new(RwLock::new(HashMap::new())),
            quarantined_ips: Arc::new(RwLock::new(HashMap::new())),
            message_bus,
            qdrant_client,
        }
    }

    /// Check if a node_id is quarantined
    pub async fn is_quarantined(&self, node_id: &str) -> bool {
        let nodes = self.quarantined_nodes.read().await;
        nodes.contains_key(node_id)
    }

    /// Check if an IP address is quarantined
    pub async fn is_ip_quarantined(&self, ip_address: &str) -> bool {
        let ips = self.quarantined_ips.read().await;
        ips.contains_key(ip_address)
    }

    /// Get the node_id for a quarantined IP, if any
    pub async fn get_node_id_for_ip(&self, ip_address: &str) -> Option<String> {
        let ips = self.quarantined_ips.read().await;
        ips.get(ip_address).cloned()
    }

    /// Quarantine a node
    pub async fn quarantine_node(
        &self,
        node_id: String,
        ip_address: Option<String>,
        reason: String,
        quarantined_by: String,
    ) -> Result<(), String> {
        info!(
            node_id = %node_id,
            reason = %reason,
            "Quarantining node"
        );

        let entry = QuarantineEntry {
            node_id: node_id.clone(),
            ip_address: ip_address.clone(),
            reason: reason.clone(),
            timestamp: Utc::now().to_rfc3339(),
            quarantined_by,
        };

        // Add to in-memory maps
        {
            let mut nodes = self.quarantined_nodes.write().await;
            nodes.insert(node_id.clone(), entry.clone());
        }

        if let Some(ref ip) = ip_address {
            let mut ips = self.quarantined_ips.write().await;
            ips.insert(ip.clone(), node_id.clone());
        }

        // Store in Qdrant for persistence
        if let Some(ref qdrant) = self.qdrant_client {
            if let Err(e) = self.store_quarantine_in_qdrant(&entry).await {
                error!(error = %e, "Failed to store quarantine entry in Qdrant");
            }
        }

        // Broadcast event
        let event = PhoenixEvent::NodeIsolated {
            node_id: node_id.clone(),
            reason: reason.clone(),
            timestamp: Utc::now().to_rfc3339(),
        };
        self.message_bus.publish(event);

        Ok(())
    }

    /// Remove a node from quarantine (re-integration)
    pub async fn reintegrate_node(&self, node_id: &str) -> Result<(), String> {
        info!(node_id = %node_id, "Reintegrating node from quarantine");

        let ip_address = {
            let nodes = self.quarantined_nodes.read().await;
            nodes.get(node_id).and_then(|e| e.ip_address.clone())
        };

        // Remove from in-memory maps
        {
            let mut nodes = self.quarantined_nodes.write().await;
            nodes.remove(node_id);
        }

        if let Some(ref ip) = ip_address {
            let mut ips = self.quarantined_ips.write().await;
            ips.remove(ip);
        }

        // Remove from Qdrant
        if let Some(ref qdrant) = self.qdrant_client {
            if let Err(e) = self.remove_quarantine_from_qdrant(node_id).await {
                warn!(error = %e, "Failed to remove quarantine entry from Qdrant");
            }
        }

        // Broadcast event
        let event = PhoenixEvent::NodeReintegrated {
            node_id: node_id.to_string(),
            timestamp: Utc::now().to_rfc3339(),
        };
        self.message_bus.publish(event);

        Ok(())
    }

    /// Get all quarantined nodes
    pub async fn list_quarantined(&self) -> Vec<QuarantineEntry> {
        let nodes = self.quarantined_nodes.read().await;
        nodes.values().cloned().collect()
    }

    /// Get quarantine entry for a node
    pub async fn get_entry(&self, node_id: &str) -> Option<QuarantineEntry> {
        let nodes = self.quarantined_nodes.read().await;
        nodes.get(node_id).cloned()
    }

    /// Store quarantine entry in Qdrant
    async fn store_quarantine_in_qdrant(&self, entry: &QuarantineEntry) -> Result<(), String> {
        let qdrant = self.qdrant_client.as_ref().ok_or("Qdrant client not available")?;

        // Ensure collection exists
        let collection_name = "quarantine_list";
        let _ = qdrant
            .create_collection(&qdrant_client::qdrant::CreateCollection {
                collection_name: collection_name.to_string(),
                vectors_config: Some(qdrant_client::qdrant::VectorsConfig {
                    config: Some(qdrant_client::qdrant::vectors_config::Config::Params(
                        qdrant_client::qdrant::VectorsParams {
                            size: 128, // Dummy size for metadata-only storage
                            distance: qdrant_client::qdrant::Distance::Cosine as i32,
                        },
                    )),
                }),
                ..Default::default()
            })
            .await;

        // Store as point with metadata
        let point_id = uuid::Uuid::new_v4().to_string();
        let payload: HashMap<String, serde_json::Value> = [
            ("node_id".to_string(), serde_json::Value::String(entry.node_id.clone())),
            ("ip_address".to_string(), entry.ip_address.as_ref().map(|ip| serde_json::Value::String(ip.clone())).unwrap_or(serde_json::Value::Null)),
            ("reason".to_string(), serde_json::Value::String(entry.reason.clone())),
            ("timestamp".to_string(), serde_json::Value::String(entry.timestamp.clone())),
            ("quarantined_by".to_string(), serde_json::Value::String(entry.quarantined_by.clone())),
        ]
        .iter()
        .cloned()
        .collect();

        let vector = vec![0.0f32; 128]; // Dummy vector

        qdrant
            .upsert_points(&qdrant_client::qdrant::UpsertPoints {
                collection_name: collection_name.to_string(),
                points: vec![qdrant_client::qdrant::PointStruct {
                    id: Some(qdrant_client::qdrant::PointId {
                        point_id_options: Some(qdrant_client::qdrant::point_id::PointIdOptions::Uuid(point_id)),
                    }),
                    vectors: Some(qdrant_client::qdrant::Vectors {
                        vectors_options: Some(qdrant_client::qdrant::vectors::VectorsOptions::Vector(qdrant_client::qdrant::Vector { data: vector })),
                    }),
                    payload,
                }],
                ..Default::default()
            })
            .await
            .map_err(|e| format!("Failed to store quarantine entry: {}", e))?;

        Ok(())
    }

    /// Remove quarantine entry from Qdrant
    async fn remove_quarantine_from_qdrant(&self, node_id: &str) -> Result<(), String> {
        let qdrant = self.qdrant_client.as_ref().ok_or("Qdrant client not available")?;

        let collection_name = "quarantine_list";

        // Find points with matching node_id
        let filter = qdrant_client::qdrant::Filter {
            must: vec![qdrant_client::qdrant::Condition {
                condition_one_of: Some(qdrant_client::qdrant::condition::ConditionOneOf::Field(
                    qdrant_client::qdrant::FieldCondition {
                        key: "node_id".to_string(),
                        r#match: Some(qdrant_client::qdrant::Match {
                            match_value: Some(qdrant_client::qdrant::r#match::MatchValue::Value(
                                node_id.to_string(),
                            )),
                        }),
                        ..Default::default()
                    },
                )),
            }],
            ..Default::default()
        };

        let search_result = qdrant
            .scroll(&qdrant_client::qdrant::ScrollPoints {
                collection_name: collection_name.to_string(),
                filter: Some(filter),
                limit: Some(100),
                ..Default::default()
            })
            .await
            .map_err(|e| format!("Failed to search quarantine entries: {}", e))?;

        // Delete found points
        let point_ids: Vec<qdrant_client::qdrant::PointId> = search_result
            .result
            .iter()
            .filter_map(|p| p.id.clone())
            .collect();

        if !point_ids.is_empty() {
            qdrant
                .delete_points(&qdrant_client::qdrant::DeletePoints {
                    collection_name: collection_name.to_string(),
                    points: Some(qdrant_client::qdrant::PointsSelector {
                        points_selector_one_of: Some(
                            qdrant_client::qdrant::points_selector::PointsSelectorOneOf::Points(
                                qdrant_client::qdrant::PointsIdsList { ids: point_ids },
                            ),
                        ),
                    }),
                    ..Default::default()
                })
                .await
                .map_err(|e| format!("Failed to delete quarantine entry: {}", e))?;
        }

        Ok(())
    }

    /// Load quarantine list from Qdrant on startup
    pub async fn load_from_qdrant(&self) -> Result<(), String> {
        let qdrant = match &self.qdrant_client {
            Some(q) => q,
            None => return Ok(()), // No Qdrant, skip loading
        };

        let collection_name = "quarantine_list";

        // Try to scroll all quarantine entries
        let result = qdrant
            .scroll(&qdrant_client::qdrant::ScrollPoints {
                collection_name: collection_name.to_string(),
                limit: Some(1000),
                ..Default::default()
            })
            .await;

        match result {
            Ok(response) => {
                let mut nodes = self.quarantined_nodes.write().await;
                let mut ips = self.quarantined_ips.write().await;

                for point in response.result {
                    if let Some(payload) = point.payload {
                        let node_id = payload
                            .get("node_id")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        let ip_address = payload
                            .get("ip_address")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());

                        if let Some(node_id) = node_id {
                            let entry = QuarantineEntry {
                                node_id: node_id.clone(),
                                ip_address: ip_address.clone(),
                                reason: payload
                                    .get("reason")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string())
                                    .unwrap_or_else(|| "unknown".to_string()),
                                timestamp: payload
                                    .get("timestamp")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string())
                                    .unwrap_or_else(|| Utc::now().to_rfc3339()),
                                quarantined_by: payload
                                    .get("quarantined_by")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string())
                                    .unwrap_or_else(|| "system".to_string()),
                            };

                            nodes.insert(node_id.clone(), entry.clone());
                            if let Some(ip) = ip_address {
                                ips.insert(ip, node_id);
                            }
                        }
                    }
                }

                info!(
                    count = nodes.len(),
                    "Loaded quarantine list from Qdrant"
                );
            }
            Err(_) => {
                // Collection might not exist yet, that's okay
                info!("Quarantine collection not found, starting with empty list");
            }
        }

        Ok(())
    }
}
