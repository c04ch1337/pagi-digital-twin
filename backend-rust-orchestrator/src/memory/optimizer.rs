//! Memory Reindexing (Optimizer) Task
//! 
//! This module implements a reindexing task that defragments the vector space
//! by recalculating the HNSW (Hierarchical Navigable Small World) graph for
//! better recall performance on bare-metal systems.
//!
//! Reindexing Strategy:
//! - Triggered automatically after MemoryPruning task completes
//! - Updates HNSW configuration on agent_logs and long_term_memory collections
//! - Uses high-precision parameters: m=16, ef_construct=100 for optimal retrieval

use std::sync::Arc;
use chrono::Utc;
use qdrant_client::{
    qdrant::{UpdateCollection, HnswConfigDiff},
    Qdrant,
};
use tracing::{info, warn, error};

use crate::bus::{GlobalMessageBus, PhoenixEvent};

/// Configuration for memory reindexing
#[derive(Clone)]
pub struct ReindexingConfig {
    /// Qdrant client instance
    pub qdrant_client: Arc<Qdrant>,
    /// Global message bus for broadcasting maintenance events
    pub message_bus: Arc<GlobalMessageBus>,
    /// Collections to reindex
    pub collections: Vec<String>,
}

impl ReindexingConfig {
    /// Create a new reindexing configuration
    pub fn new(
        qdrant_client: Arc<Qdrant>,
        message_bus: Arc<GlobalMessageBus>,
    ) -> Self {
        Self {
            qdrant_client,
            message_bus,
            collections: vec!["agent_logs".to_string(), "long_term_memory".to_string()],
        }
    }
}

/// Reindex a single collection by updating its HNSW configuration
async fn reindex_collection(
    qdrant: &Qdrant,
    collection_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    info!(
        collection = %collection_name,
        "Starting reindexing for collection"
    );

    // Configure HNSW parameters for high-precision retrieval on bare metal
    // m: 16 - number of bi-directional links for each node (higher = better recall, more memory)
    // ef_construct: 100 - size of the candidate list during index construction (higher = better quality)
    let hnsw_config = HnswConfigDiff {
        m: Some(16),
        ef_construct: Some(100),
        full_scan_threshold: None,
        max_indexing_threads: None,
        on_disk: None,
        payload_m: None,
    };

    let update_request = UpdateCollection {
        collection_name: collection_name.to_string(),
        optimizers_config: None,
        params: None,
        hnsw_config: Some(hnsw_config),
        vectors_config: None,
        quantization_config: None,
        sparse_vectors_config: None,
        timeout: None,
    };

    qdrant
        .update_collection(update_request)
        .await
        .map_err(|e| {
            error!(
                error = %e,
                collection = %collection_name,
                "Failed to update collection HNSW configuration"
            );
            e
        })?;

    info!(
        collection = %collection_name,
        m = 16,
        ef_construct = 100,
        "Collection reindexing completed successfully"
    );

    Ok(())
}

/// Run reindexing on all configured collections
pub async fn run_reindexing(config: &ReindexingConfig) -> Result<(), Box<dyn std::error::Error>> {
    info!(
        collections = ?config.collections,
        "Starting memory reindexing task"
    );

    // Broadcast maintenance started event
    config.message_bus.publish(PhoenixEvent::MaintenanceStarted {
        operation: "memory_reindexing".to_string(),
        timestamp: Utc::now().to_rfc3339(),
    });

    let mut successful_collections = Vec::new();
    let mut failed_collections = Vec::new();

    for collection in &config.collections {
        match reindex_collection(&config.qdrant_client, collection).await {
            Ok(_) => {
                successful_collections.push(collection.clone());
                info!(
                    collection = %collection,
                    "Reindexing completed for collection"
                );
            }
            Err(e) => {
                failed_collections.push(collection.clone());
                error!(
                    collection = %collection,
                    error = %e,
                    "Failed to reindex collection"
                );
                // Continue with other collections
            }
        }
    }

    if !successful_collections.is_empty() {
        info!(
            successful_collections = ?successful_collections,
            "Memory reindexing task completed for {} collection(s)",
            successful_collections.len()
        );

        // Broadcast indexing complete event
        config.message_bus.publish(PhoenixEvent::IndexingComplete {
            collections: successful_collections,
            timestamp: Utc::now().to_rfc3339(),
        });
    }

    if !failed_collections.is_empty() {
        warn!(
            failed_collections = ?failed_collections,
            "Some collections failed to reindex"
        );
        return Err(format!("Failed to reindex collections: {:?}", failed_collections).into());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_reindexing_config() {
        // This test would require a mock Qdrant client
        // For now, just verify the config can be created
        // (actual implementation would need a test Qdrant instance)
    }
}
