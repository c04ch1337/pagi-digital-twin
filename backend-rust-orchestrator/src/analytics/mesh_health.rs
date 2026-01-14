//! Mesh Health Report - Network Alignment and Operational Status
//!
//! This module provides high-level metrics about the Blue Flame network's
//! alignment, operational status, and health. Critical for IT-Strategic oversight.

use std::sync::Arc;
use std::time::{Duration, Instant};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::network::handshake::{NodeHandshakeServiceImpl, PeerStatus};
use crate::network::quarantine::QuarantineManager;

/// Mesh health metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshHealthReport {
    pub total_nodes: u32,
    pub aligned_nodes: u32,
    pub quarantined_nodes: u32,
    pub alignment_drift_percentage: f64,
    pub last_updated_utc: String,
}

/// Mesh health service with caching
pub struct MeshHealthService {
    handshake_service: Arc<NodeHandshakeServiceImpl>,
    quarantine_manager: Arc<QuarantineManager>,
    local_manifest_hash: String,
    local_guardrail_version: String,
    cached_report: Arc<RwLock<Option<(MeshHealthReport, Instant)>>>,
    cache_ttl: Duration,
}

impl MeshHealthService {
    /// Create a new mesh health service
    pub fn new(
        handshake_service: Arc<NodeHandshakeServiceImpl>,
        quarantine_manager: Arc<QuarantineManager>,
        local_manifest_hash: String,
        local_guardrail_version: String,
    ) -> Self {
        Self {
            handshake_service,
            quarantine_manager,
            local_manifest_hash,
            local_guardrail_version,
            cached_report: Arc::new(RwLock::new(None)),
            cache_ttl: Duration::from_secs(60), // 1 minute cache
        }
    }

    /// Generate a fresh mesh health report
    pub async fn generate_report(&self) -> MeshHealthReport {
        // Get all peers
        let peers = self.handshake_service.get_verified_peers().await;
        let quarantined = self.quarantine_manager.list_quarantined().await;

        // Count nodes by status
        let total_nodes = peers.len() as u32;
        let aligned_nodes = peers
            .iter()
            .filter(|p| {
                matches!(p.status, PeerStatus::Verified)
                    && !quarantined.iter().any(|q| q.node_id == p.node_id)
            })
            .count() as u32;
        let quarantined_nodes = quarantined.len() as u32;

        // Calculate alignment drift
        // A node is "outdated" if its manifest_hash differs from local
        // Note: We compare manifest_hash for alignment, not software_version
        let outdated_count = peers
            .iter()
            .filter(|p| {
                matches!(p.status, PeerStatus::Verified)
                    && !quarantined.iter().any(|q| q.node_id == p.node_id)
                    && !self.local_manifest_hash.is_empty()
                    && p.manifest_hash != self.local_manifest_hash
            })
            .count();

        let alignment_drift_percentage = if aligned_nodes > 0 {
            (outdated_count as f64 / aligned_nodes as f64) * 100.0
        } else {
            0.0
        };

        MeshHealthReport {
            total_nodes,
            aligned_nodes,
            quarantined_nodes,
            alignment_drift_percentage,
            last_updated_utc: Utc::now().to_rfc3339(),
        }
    }

    /// Get mesh health report (with caching)
    pub async fn get_report(&self) -> MeshHealthReport {
        // Check cache
        {
            let cache = self.cached_report.read().await;
            if let Some((ref report, timestamp)) = *cache {
                if timestamp.elapsed() < self.cache_ttl {
                    info!("Returning cached mesh health report");
                    return report.clone();
                }
            }
        }

        // Generate fresh report
        let report = self.generate_report().await;

        // Update cache
        {
            let mut cache = self.cached_report.write().await;
            *cache = Some((report.clone(), Instant::now()));
        }

        info!(
            total_nodes = report.total_nodes,
            aligned_nodes = report.aligned_nodes,
            quarantined_nodes = report.quarantined_nodes,
            alignment_drift = %report.alignment_drift_percentage,
            "Generated mesh health report"
        );

        report
    }

    /// Invalidate cache (force refresh on next request)
    pub async fn invalidate_cache(&self) {
        let mut cache = self.cached_report.write().await;
        *cache = None;
        info!("Mesh health cache invalidated");
    }
}
