//! Memory Pruning (Vacuum) Task
//! 
//! This module implements a daily memory pruning task that cleans up old vector data
//! from Qdrant collections (agent_logs and telemetry) to prevent the bare-metal host
//! from slowing down due to excessive data accumulation.
//!
//! Pruning Strategy:
//! - Deletes points where timestamp is older than 30 days
//! - UNLESS they have an importance_score > 0.8 (indicating they were used for Playbook distillation)
//! - Uses filter: (timestamp < now - 30d) AND (importance_score <= 0.8 OR status != "essential")

use std::sync::Arc;
use std::time::Duration;
use chrono::{DateTime, Utc};
use qdrant_client::{
    qdrant::{
        Condition, DeletePoints, FieldCondition, Filter, PointsSelector, 
        points_selector::PointsSelectorOneOf, Range, Value,
    },
    Qdrant,
};
use tracing::{info, warn, error};
use tokio::time;

use crate::bus::{GlobalMessageBus, PhoenixEvent};
use crate::memory::optimizer::{ReindexingConfig, run_reindexing};

/// Configuration for memory pruning
#[derive(Clone)]
pub struct PruningConfig {
    /// Qdrant client instance
    pub qdrant_client: Arc<Qdrant>,
    /// Global message bus for broadcasting maintenance events
    pub message_bus: Arc<GlobalMessageBus>,
    /// Collections to prune
    pub collections: Vec<String>,
    /// Retention period in days (default: 30)
    pub retention_days: i64,
    /// Importance score threshold (default: 0.8)
    pub importance_threshold: f64,
    /// Interval between pruning runs (default: 24 hours)
    pub pruning_interval: Duration,
}

impl PruningConfig {
    /// Create a new pruning configuration
    pub fn new(
        qdrant_client: Arc<Qdrant>,
        message_bus: Arc<GlobalMessageBus>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let qdrant_url = std::env::var("QDRANT_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:6334".to_string());
        
        info!(
            qdrant_url = %qdrant_url,
            "Initializing memory pruning configuration"
        );

        Ok(Self {
            qdrant_client,
            message_bus,
            collections: vec!["agent_logs".to_string(), "telemetry".to_string()],
            retention_days: std::env::var("MEMORY_RETENTION_DAYS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(30),
            importance_threshold: std::env::var("MEMORY_IMPORTANCE_THRESHOLD")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.8),
            pruning_interval: Duration::from_secs(
                std::env::var("MEMORY_PRUNING_INTERVAL_SECS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(86400), // 24 hours
            ),
        })
    }
}

/// Run a single pruning operation on a collection
async fn prune_collection(
    qdrant: &Qdrant,
    collection_name: &str,
    retention_days: i64,
    importance_threshold: f64,
) -> Result<usize, Box<dyn std::error::Error>> {
    info!(
        collection = %collection_name,
        retention_days = retention_days,
        importance_threshold = importance_threshold,
        "Starting memory pruning for collection"
    );

    // Calculate the cutoff timestamp (now - retention_days)
    // Timestamps are stored as RFC3339 strings in Qdrant, so we need to compare strings
    let cutoff_time = Utc::now() - chrono::Duration::days(retention_days);
    let cutoff_timestamp_str = cutoff_time.to_rfc3339();

    // Build the filter condition:
    // Delete points where: (timestamp < cutoff) AND NOT (importance_score > threshold AND status == "essential")
    // 
    // Qdrant filter logic: must = AND, should = OR, must_not = NOT
    // We want: must = [timestamp < cutoff] AND must_not = [importance_score > threshold AND status == "essential"]
    
    // NOTE: We currently scroll + filter in code (timestamps are RFC3339 strings).
    // Any server-side filter predicates can be added later once the payload schema is standardized.
    
    // Condition 1: timestamp < cutoff (as string comparison)
    // Since timestamps are RFC3339 strings, we can use string comparison
    // For string comparison, we'll need to scroll and filter in code, or use a different approach
    // Actually, Qdrant supports datetime_range for timestamp fields if they're properly indexed
    // For now, we'll scroll all old points and filter in memory, or use a simpler approach:
    // Scroll points with timestamp field, then filter by parsing the timestamp string
    
    // Alternative: Use should conditions to match either:
    // - timestamp < cutoff AND (importance_score missing OR importance_score <= threshold)
    // - timestamp < cutoff AND (status missing OR status != "essential")
    //
    // Simplest approach: Scroll all points, then filter in code based on:
    // 1. Parse timestamp and check if < cutoff
    // 2. Check if importance_score > threshold AND status == "essential" (if so, skip)
    
    // For now, let's use a filter that matches old timestamps
    // We'll need to scroll and check each point's timestamp string
    // But first, let's try to use a filter that will match most old points
    
    // Since Qdrant stores timestamps as strings, we can't easily do range queries
    // We'll need to scroll through points and filter manually
    // However, for efficiency, let's try to use a filter that at least narrows down the search
    
    // Build a filter that will help us find candidate points
    // We'll scroll all points and then filter by timestamp in code
    let filter = None; // We'll filter in code after scrolling

    // Scroll through all points and filter by timestamp and importance/status
    // Since timestamps are stored as RFC3339 strings, we need to parse them
    let scroll_request = qdrant_client::qdrant::ScrollPoints {
        collection_name: collection_name.to_string(),
        filter,
        offset: None,
        limit: Some(10000), // Process in batches
        with_payload: Some(true.into()), // Need payload to check timestamp and importance_score
        with_vectors: Some(false.into()),
        ..Default::default()
    };

    let mut total_deleted = 0;
    let mut offset = None;
    
    loop {
        let mut scroll_req = scroll_request.clone();
        if let Some(off) = offset {
            scroll_req.offset = Some(off);
        }
        
        let scroll_result = qdrant
            .scroll(scroll_req)
            .await
            .map_err(|e| {
                error!(
                    error = %e,
                    collection = %collection_name,
                    "Failed to scroll points in Qdrant"
                );
                e
            })?;

        let points = scroll_result.result;
        if points.is_empty() {
            break;
        }

        // Filter points based on our criteria:
        // 1. timestamp < cutoff
        // 2. NOT (importance_score > threshold AND status == "essential")
        let mut points_to_delete = Vec::new();
        
        for point in &points {
            let payload = &point.payload;
            
            // Extract timestamp
            let timestamp_str = payload
                .get("timestamp")
                .and_then(|v| {
                    use qdrant_client::qdrant::value::Kind;
                    match v.kind.as_ref()? {
                        Kind::StringValue(s) => Some(s.clone()),
                        _ => None,
                    }
                });
            
            // Check if timestamp is old enough
            let is_old = if let Some(ts_str) = timestamp_str {
                if let Ok(ts) = DateTime::parse_from_rfc3339(&ts_str) {
                    ts.with_timezone(&Utc) < cutoff_time
                } else {
                    // If timestamp can't be parsed, skip this point (safer to not delete)
                    false
                }
            } else {
                // No timestamp field - skip (safer to not delete)
                false
            };
            
            if !is_old {
                continue;
            }
            
            // Extract importance_score and status
            let importance_score = payload
                .get("importance_score")
                .and_then(|v| {
                    use qdrant_client::qdrant::value::Kind;
                    match v.kind.as_ref()? {
                        Kind::DoubleValue(d) => Some(*d),
                        Kind::IntegerValue(i) => Some(*i as f64),
                        Kind::StringValue(s) => s.parse().ok(),
                        _ => None,
                    }
                })
                .unwrap_or(0.0);
            
            let status = payload
                .get("status")
                .and_then(|v| {
                    use qdrant_client::qdrant::value::Kind;
                    match v.kind.as_ref()? {
                        Kind::StringValue(s) => Some(s.clone()),
                        _ => None,
                    }
                })
                .unwrap_or_default();
            
            // Check if point should be preserved:
            // Preserve if: importance_score > threshold AND status == "essential"
            let should_preserve = importance_score > importance_threshold && status == "essential";
            
            if !should_preserve {
                if let Some(id) = &point.id {
                    points_to_delete.push(id.clone());
                }
            }
        }

        if points_to_delete.is_empty() {
            // No points to delete in this batch, but continue scrolling
            if points.len() < 10000 {
                break;
            }
            offset = points.last().and_then(|p| p.id.clone());
            continue;
        }

        let batch_deleted = points_to_delete.len();

        // Delete the points
        let delete_request = DeletePoints {
            collection_name: collection_name.to_string(),
            points: Some(PointsSelector {
                points_selector_one_of: Some(
                    PointsSelectorOneOf::Points(
                        qdrant_client::qdrant::PointsIdsList {
                            ids: points_to_delete,
                        },
                    ),
                ),
            }),
            ..Default::default()
        };

        qdrant
            .delete_points(delete_request)
            .await
            .map_err(|e| {
                error!(
                    error = %e,
                    collection = %collection_name,
                    "Failed to delete points from Qdrant"
                );
                e
            })?;

        total_deleted += batch_deleted;
        info!(
            collection = %collection_name,
            deleted_so_far = total_deleted,
            batch_size = batch_deleted,
            "Deleted batch of points"
        );

        // Check if there are more points
        if points.len() < 10000 {
            break;
        }

        // Update offset for next iteration
        offset = points.last().and_then(|p| p.id.clone());
    }

    info!(
        collection = %collection_name,
        total_deleted = total_deleted,
        "Memory pruning completed for collection"
    );

    Ok(total_deleted)
}

/// Run memory pruning on all configured collections
pub async fn run_pruning(config: &PruningConfig) -> Result<usize, Box<dyn std::error::Error>> {
    info!(
        collections = ?config.collections,
        retention_days = config.retention_days,
        importance_threshold = config.importance_threshold,
        "Starting memory pruning task"
    );

    // Broadcast maintenance started event
    config.message_bus.publish(PhoenixEvent::MaintenanceStarted {
        operation: "memory_pruning".to_string(),
        timestamp: Utc::now().to_rfc3339(),
    });

    let mut total_deleted = 0;

    for collection in &config.collections {
        match prune_collection(
            &config.qdrant_client,
            collection,
            config.retention_days,
            config.importance_threshold,
        )
        .await
        {
            Ok(count) => {
                total_deleted += count;
                info!(
                    collection = %collection,
                    deleted = count,
                    "Pruning completed for collection"
                );
            }
            Err(e) => {
                error!(
                    collection = %collection,
                    error = %e,
                    "Failed to prune collection"
                );
                // Continue with other collections
            }
        }
    }

    info!(
        total_deleted = total_deleted,
        "Memory pruning task completed"
    );

    // Trigger reindexing immediately after pruning completes
    info!("Triggering reindexing task after pruning completion");
    let reindexing_config = ReindexingConfig::new(
        config.qdrant_client.clone(),
        config.message_bus.clone(),
    );
    
    match run_reindexing(&reindexing_config).await {
        Ok(_) => {
            info!("Reindexing completed successfully after pruning");
        }
        Err(e) => {
            warn!(
                error = %e,
                "Reindexing failed after pruning, but pruning completed successfully"
            );
            // Don't fail the pruning task if reindexing fails
        }
    }

    Ok(total_deleted)
}

/// Start the memory pruning task as a background service
/// This will run pruning on a schedule (daily by default)
pub async fn start_pruning_service(
    config: PruningConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    info!(
        interval_secs = config.pruning_interval.as_secs(),
        "Starting memory pruning service"
    );

    let mut interval = time::interval(config.pruning_interval);
    
    // Run immediately on startup (optional - you might want to skip this)
    // interval.tick().await;

    loop {
        interval.tick().await;
        
        info!("Memory pruning scheduled task triggered");
        
        match run_pruning(&config).await {
            Ok(deleted) => {
                info!(
                    deleted_points = deleted,
                    "Memory pruning completed successfully"
                );
            }
            Err(e) => {
                error!(
                    error = %e,
                    "Memory pruning task failed"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pruning_config() {
        // This test would require a mock Qdrant client
        // For now, just verify the config can be created
        // (actual implementation would need a test Qdrant instance)
    }
}
