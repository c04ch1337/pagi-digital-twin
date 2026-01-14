//! Phoenix Memory Exchange - Peer-to-Peer Knowledge Transfer
//!
//! This module implements direct, peer-to-peer knowledge transfer between bare-metal nodes,
//! using the PhoenixEvent bus for coordination and gRPC streaming for data transfer.
//! All exchanged data is scrubbed using the Phoenix-Redacted filter to remove sensitive
//! information (hostnames, IPs, etc.) before transmission.
//!
//! Flow:
//! 1. Node A sends MemoryExchangeRequest via PhoenixEvent bus
//! 2. Node B verifies alignment token
//! 3. Node B retrieves relevant Qdrant vectors
//! 4. Node B applies Phoenix-Redacted filter
//! 5. Node B streams clean embeddings to Node A via gRPC

use std::sync::Arc;
use std::process::Command;
use std::path::PathBuf;
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::time::interval;
use tonic::{Request, Response, Status};
use tracing::{error, info, warn};
use serde_json;
use uuid::Uuid;
use chrono::{Utc, DateTime};

use crate::bus::{GlobalMessageBus, PhoenixEvent};
use crate::network::handshake::NodeHandshakeServiceImpl;
use crate::security::PrivacyFilter;
use qdrant_client::{
    qdrant::{SearchPoints, ScoredPoint, Value, DeletePoints, PointsSelector, points_selector::PointsSelectorOneOf, Filter, FieldCondition, Match},
    Qdrant,
};

// Include generated proto code
use crate::memory_exchange_proto::phoenix_memory_exchange_service_server::{
    PhoenixMemoryExchangeService, PhoenixMemoryExchangeServiceServer,
};
use crate::memory_exchange_proto::{
    ExchangeMemoryRequest, ExchangeMemoryResponse,
    AlignmentVerificationRequest, AlignmentVerificationResponse,
};

/// Topic frequency with timestamp for decay tracking
#[derive(Debug, Clone)]
struct TopicFrequency {
    count: usize,
    last_accessed: SystemTime,
}

/// Phoenix Memory Exchange Service Implementation
#[derive(Clone)]
pub struct PhoenixMemoryExchangeServiceImpl {
    qdrant_client: Arc<Qdrant>,
    message_bus: Arc<GlobalMessageBus>,
    handshake_service: Arc<NodeHandshakeServiceImpl>,
    privacy_filter: Arc<PrivacyFilter>,
    node_id: String,
    /// Topic frequency tracking: topic -> (count, last_accessed)
    topic_frequencies: Arc<RwLock<HashMap<String, TopicFrequency>>>,
    /// Node topic volumes: node_id -> total exchanges
    node_volumes: Arc<RwLock<HashMap<String, usize>>>,
    /// Last snapshot timestamp (for safety checks)
    last_snapshot: Arc<RwLock<Option<std::time::SystemTime>>>,
    /// Maintenance mode flag - when enabled, pauses PhoenixEvent bus traffic
    maintenance_mode: Arc<RwLock<bool>>,
}

impl PhoenixMemoryExchangeServiceImpl {
    pub fn new(
        qdrant_client: Arc<Qdrant>,
        message_bus: Arc<GlobalMessageBus>,
        handshake_service: Arc<NodeHandshakeServiceImpl>,
        node_id: String,
    ) -> Self {
        Self {
            qdrant_client,
            message_bus,
            handshake_service,
            privacy_filter: Arc::new(PrivacyFilter::new()),
            node_id,
            topic_frequencies: Arc::new(RwLock::new(HashMap::new())),
            node_volumes: Arc::new(RwLock::new(HashMap::new())),
            last_snapshot: Arc::new(RwLock::new(None)),
            maintenance_mode: Arc::new(RwLock::new(false)),
        }
    }

    /// Start background task to decay topic frequencies (24 hour TTL)
    pub async fn start_topic_decay_task(&self) {
        let topic_frequencies = self.topic_frequencies.clone();
        let mut interval = interval(Duration::from_secs(3600)); // Check every hour

        tokio::spawn(async move {
            loop {
                interval.tick().await;
                
                let now = SystemTime::now();
                let decay_threshold = Duration::from_secs(24 * 3600); // 24 hours

                let mut frequencies = topic_frequencies.write().await;
                let mut to_remove = Vec::new();

                for (topic, freq) in frequencies.iter() {
                    if let Ok(elapsed) = now.duration_since(freq.last_accessed) {
                        if elapsed > decay_threshold {
                            // Topic hasn't been accessed in 24 hours, mark for removal
                            to_remove.push(topic.clone());
                        } else {
                            // Apply decay: reduce frequency based on age
                            let decay_factor = 1.0 - (elapsed.as_secs() as f64 / decay_threshold.as_secs() as f64);
                            let new_count = (freq.count as f64 * decay_factor.max(0.0)) as usize;
                            if new_count == 0 && elapsed > Duration::from_secs(12 * 3600) {
                                // If count would be 0 and it's been 12+ hours, mark for removal
                                to_remove.push(topic.clone());
                            }
                        }
                    }
                }

                // Remove decayed topics
                for topic in to_remove {
                    frequencies.remove(&topic);
                    info!(topic = %topic, "Removed decayed topic from heat map");
                }
            }
        });
    }

    /// Start listening for memory exchange requests via PhoenixEvent bus
    pub async fn start_listener(&self) {
        let mut receiver = self.message_bus.subscribe();
        let qdrant = self.qdrant_client.clone();
        let handshake_service = self.handshake_service.clone();
        let privacy_filter = self.privacy_filter.clone();
        let node_id = self.node_id.clone();

        tokio::spawn(async move {
            loop {
                match receiver.recv().await {
                    Ok(event) => {
                        if let PhoenixEvent::MemoryExchangeRequest {
                            requesting_node,
                            topic,
                            namespace,
                            ..
                        } = event {
                            if requesting_node != node_id {
                                info!(
                                    requesting_node = %requesting_node,
                                    topic = %topic,
                                    namespace = %namespace,
                                    "Received memory exchange request via PhoenixEvent"
                                );
                                // In a full implementation, we would initiate gRPC streaming here
                                // For now, we log the request
                            }
                        }
                    }
                    Err(e) => {
                        error!(error = %e, "Error receiving memory exchange event");
                    }
                }
            }
        });
    }

    /// Verify alignment token before allowing memory exchange
    async fn verify_alignment_token(
        &self,
        node_id: &str,
        alignment_token: &str,
    ) -> Result<bool, String> {
        // Get the peer to verify their alignment token
        if let Some(peer) = self.handshake_service.get_peer(node_id).await {
            // In a full implementation, we would verify the alignment token
            // For now, we check if the peer is verified
            match peer.status {
                crate::network::handshake::PeerStatus::Verified => {
                    info!(node_id = %node_id, "Peer is verified, alignment token accepted");
                    Ok(true)
                }
                _ => {
                    warn!(node_id = %node_id, "Peer is not verified, rejecting alignment token");
                    Ok(false)
                }
            }
        } else {
            warn!(node_id = %node_id, "Peer not found in verified peers");
            Ok(false)
        }
    }

    /// Scrub content using Ferrellgas scrubber and return (clean_text, redaction_count)
    async fn scrub_with_ferrellgas_scrubber(&self, content: &str) -> Result<(String, usize), String> {
        // Find the scrubber script
        let script_path = if let Ok(current_dir) = std::env::current_dir() {
            let script = current_dir.join("scripts").join("ferrellgas_scrubber.py");
            if script.exists() {
                script
            } else {
                current_dir.parent()
                    .map(|p| p.join("scripts").join("ferrellgas_scrubber.py"))
                    .unwrap_or_else(|| script)
            }
        } else {
            PathBuf::from("scripts").join("ferrellgas_scrubber.py")
        };

        if !script_path.exists() {
            // Fallback to Rust PrivacyFilter if scrubber not available
            warn!("Ferrellgas scrubber not found, using Rust PrivacyFilter");
            let filter = PrivacyFilter::new();
            let scrubbed = filter.scrub_playbook(content.to_string());
            return Ok((scrubbed, 0)); // Can't count redactions with Rust filter alone
        }

        // Create a temporary file with the content
        let temp_file = std::env::temp_dir().join(format!("scrub_{}.txt", Uuid::new_v4()));
        fs::write(&temp_file, content).await
            .map_err(|e| format!("Failed to write temp file: {}", e))?;

        // Run the scrubber with --json flag
        let output = Command::new("python3")
            .arg(script_path.to_string_lossy().as_ref())
            .arg(temp_file.to_string_lossy().as_ref())
            .arg("--json")
            .output()
            .or_else(|_| {
                // Try python if python3 fails
                Command::new("python")
                    .arg(script_path.to_string_lossy().as_ref())
                    .arg(temp_file.to_string_lossy().as_ref())
                    .arg("--json")
                    .output()
            })
            .map_err(|e| format!("Failed to execute scrubber: {}", e))?;

        // Clean up temp file
        let _ = fs::remove_file(&temp_file).await;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!(stderr = %stderr, "Scrubber returned non-zero exit code, falling back to Rust filter");
            let filter = PrivacyFilter::new();
            let scrubbed = filter.scrub_playbook(content.to_string());
            return Ok((scrubbed, 0));
        }

        // Parse JSON output
        let stdout = String::from_utf8_lossy(&output.stdout);
        match serde_json::from_str::<serde_json::Value>(&stdout) {
            Ok(json) => {
                let clean_text = json.get("clean_text")
                    .and_then(|v| v.as_str())
                    .unwrap_or(content)
                    .to_string();
                let redaction_count = json.get("redaction_count")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as usize;
                Ok((clean_text, redaction_count))
            }
            Err(e) => {
                warn!(error = %e, "Failed to parse scrubber JSON output, falling back to Rust filter");
                let filter = PrivacyFilter::new();
                let scrubbed = filter.scrub_playbook(content.to_string());
                Ok((scrubbed, 0))
            }
        }
    }

    /// Search Qdrant for relevant vectors and apply redaction
    async fn search_and_redact(
        &self,
        topic: &str,
        namespace: &str,
        top_k: i32,
    ) -> Result<Vec<ScoredPoint>, Status> {
        // This is a placeholder - in a full implementation, we would:
        // 1. Generate embedding for the topic query
        // 2. Search Qdrant collection
        // 3. Apply privacy filter to content
        // 4. Return redacted vectors

        info!(
            topic = %topic,
            namespace = %namespace,
            top_k = top_k,
            "Searching Qdrant for memory vectors"
        );

        // For now, return empty results
        // In production, this would:
        // - Use an embedding model to convert topic to vector
        // - Search Qdrant collections (agent_logs, telemetry, etc.)
        // - Apply PrivacyFilter::scrub_playbook to each result's content
        // - Return redacted ScoredPoint objects

        Ok(Vec::new())
    }
}

#[tonic::async_trait]
impl PhoenixMemoryExchangeService for PhoenixMemoryExchangeServiceImpl {
    type ExchangeMemoryStream = tokio_stream::wrappers::ReceiverStream<Result<ExchangeMemoryResponse, Status>>;

    async fn exchange_memory(
        &self,
        request: Request<ExchangeMemoryRequest>,
    ) -> Result<Response<Self::ExchangeMemoryStream>, Status> {
        let req = request.into_inner();
        let remote_address = request.remote_addr()
            .map(|addr| addr.to_string())
            .unwrap_or_else(|| "unknown".to_string());

        info!(
            requesting_node = %req.requesting_node_id,
            topic = %req.topic,
            namespace = %req.namespace,
            "Received memory exchange request"
        );

        // Verify alignment token
        if !self.verify_alignment_token(&req.requesting_node_id, &req.alignment_token).await
            .map_err(|e| Status::internal(format!("Alignment verification failed: {}", e)))? {
            return Err(Status::permission_denied("Alignment token verification failed"));
        }

        // Search and redact memory vectors
        let vectors = self.search_and_redact(
            &req.topic,
            &req.namespace,
            req.top_k.max(1).min(100), // Clamp between 1 and 100
        ).await?;

        // Create response stream
        let (tx, rx) = tokio::sync::mpsc::channel(100);

        // Clone necessary data for the spawned task
        let message_bus = self.message_bus.clone();
        let source_node = self.node_id.clone();
        let destination_node = req.requesting_node_id.clone();
        let topic = req.topic.clone();
        let scrubber_self = self.clone();
        let topic_frequencies = self.topic_frequencies.clone();
        let node_volumes = self.node_volumes.clone();

        // Stream redacted vectors
        tokio::spawn(async move {
            let mut total_redacted_count = 0;
            let mut total_bytes = 0u64;
            let mut fragment_count = 0;

            for (idx, point) in vectors.iter().enumerate() {
                // Extract content from point payload
                let content = if let Some(payload) = &point.payload {
                    if let Some(Value::StringValue(s)) = payload.get("content") {
                        s.clone()
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };

                // Apply Ferrellgas scrubber (with fallback to Rust PrivacyFilter)
                let (redacted_content, redaction_count) = match scrubber_self.scrub_with_ferrellgas_scrubber(&content).await {
                    Ok(result) => result,
                    Err(e) => {
                        warn!(error = %e, "Failed to scrub with Ferrellgas scrubber, using Rust filter");
                        let filter = PrivacyFilter::new();
                        (filter.scrub_playbook(content), 0)
                    }
                };

                total_redacted_count += redaction_count;
                total_bytes += redacted_content.len() as u64;
                fragment_count += 1;

                let response = ExchangeMemoryResponse {
                    memory_id: point.id.as_ref().map(|id| format!("{:?}", id)).unwrap_or_default(),
                    vector: point.vectors.as_ref()
                        .and_then(|v| v.vectors.as_ref())
                        .map(|v| v.vector.clone())
                        .unwrap_or_default(),
                    redacted_content,
                    memory_type: point.payload.as_ref()
                        .and_then(|p| p.get("memory_type"))
                        .and_then(|v| match v {
                            Value::StringValue(s) => Some(s.clone()),
                            _ => None,
                        })
                        .unwrap_or_default(),
                    timestamp: point.payload.as_ref()
                        .and_then(|p| p.get("timestamp"))
                        .and_then(|v| match v {
                            Value::StringValue(s) => Some(s.clone()),
                            _ => None,
                        })
                        .unwrap_or_default(),
                    similarity_score: point.score,
                    is_complete: idx == vectors.len() - 1,
                };

                if tx.send(Ok(response)).await.is_err() {
                    error!("Receiver dropped, stopping memory exchange stream");
                    break;
                }
            }

            // Publish MemoryTransfer event when exchange completes
            if fragment_count > 0 {
                // Track topic frequency with timestamp
                {
                    let mut frequencies = topic_frequencies.write().await;
                    let entry = frequencies.entry(topic.clone()).or_insert_with(|| {
                        TopicFrequency {
                            count: 0,
                            last_accessed: SystemTime::now(),
                        }
                    });
                    entry.count += 1;
                    entry.last_accessed = SystemTime::now();
                }

                // Track node volumes (source node as knowledge source)
                {
                    let mut volumes = node_volumes.write().await;
                    *volumes.entry(source_node.clone()).or_insert(0) += fragment_count;
                }

                let transfer_event = PhoenixEvent::MemoryTransfer {
                    source_node: source_node.clone(),
                    destination_node: destination_node.clone(),
                    topic: topic.clone(),
                    fragments_count: fragment_count,
                    bytes_transferred: total_bytes,
                    redacted_entities_count: total_redacted_count,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                };
                message_bus.publish(transfer_event);
                info!(
                    source = %source_node,
                    destination = %destination_node,
                    topic = %topic,
                    fragments = fragment_count,
                    bytes = total_bytes,
                    redacted = total_redacted_count,
                    "Memory transfer completed and published to bus"
                );
            }
        });

        Ok(Response::new(tokio_stream::wrappers::ReceiverStream::new(rx)))
    }

    async fn verify_alignment(
        &self,
        request: Request<AlignmentVerificationRequest>,
    ) -> Result<Response<AlignmentVerificationResponse>, Status> {
        let req = request.into_inner();

        let verified = self.verify_alignment_token(&req.node_id, &req.alignment_token).await
            .map_err(|e| Status::internal(format!("Verification error: {}", e)))?;

        Ok(Response::new(AlignmentVerificationResponse {
            verified,
            message: if verified {
                "Alignment token verified".to_string()
            } else {
                "Alignment token verification failed".to_string()
            },
        }))
    }
}

/// Memory exchange statistics
#[derive(Debug, Clone, Default)]
pub struct MemoryExchangeStats {
    pub bytes_transferred_24h: u64,
    pub fragments_exchanged_24h: usize,
    pub active_transfers: usize,
}

impl PhoenixMemoryExchangeServiceImpl {
    /// Get memory exchange statistics (for API access)
    pub async fn get_statistics(&self) -> MemoryExchangeStats {
        // TODO: Implement actual statistics tracking
        // For now, return default values
        // In a full implementation, we would track:
        // - Bytes transferred in the last 24 hours
        // - Number of fragments exchanged
        // - Currently active transfers
        MemoryExchangeStats::default()
    }

    /// Get count of verified peers (for API access)
    pub async fn get_verified_peers_count(&self) -> usize {
        self.handshake_service.get_verified_peers().await.len()
    }

    /// Get topic frequencies (for heat map visualization)
    pub async fn get_topic_frequencies(&self) -> HashMap<String, usize> {
        let frequencies = self.topic_frequencies.read().await;
        frequencies
            .iter()
            .map(|(topic, freq)| (topic.clone(), freq.count))
            .collect()
    }

    /// Prune a specific topic from all Qdrant collections
    pub async fn prune_topic(&self, topic: &str) -> Result<usize, String> {
        info!(topic = %topic, "Pruning topic from Qdrant collections");

        // Remove from topic frequencies
        {
            let mut frequencies = self.topic_frequencies.write().await;
            frequencies.remove(topic);
        }

        // Delete vectors from Qdrant collections that match this topic
        let collections = vec!["agent_logs", "telemetry", "playbooks"];
        let mut total_deleted = 0;

        for collection_name in collections {
            match self.delete_topic_from_collection(collection_name, topic).await {
                Ok(count) => {
                    total_deleted += count;
                    info!(
                        collection = %collection_name,
                        topic = %topic,
                        deleted = count,
                        "Deleted vectors from collection"
                    );
                }
                Err(e) => {
                    warn!(
                        collection = %collection_name,
                        topic = %topic,
                        error = %e,
                        "Failed to delete vectors from collection"
                    );
                }
            }
        }

        // Broadcast pruning event to mesh
        let prune_event = PhoenixEvent::MemoryPrune {
            topic: topic.to_string(),
            deleted_count: total_deleted,
            timestamp: Utc::now().to_rfc3339(),
        };
        self.message_bus.publish(prune_event);

        Ok(total_deleted)
    }

    /// Delete vectors matching a topic from a Qdrant collection
    async fn delete_topic_from_collection(
        &self,
        collection_name: &str,
        topic: &str,
    ) -> Result<usize, String> {
        // Create a filter to match vectors with this topic in metadata
        let filter = Filter {
            must: vec![
                qdrant_client::qdrant::Condition {
                    condition_one_of: Some(
                        qdrant_client::qdrant::condition::ConditionOneOf::Field(
                            FieldCondition {
                                key: "topic".to_string(),
                                r#match: Some(Match {
                                    match_value: Some(
                                        qdrant_client::qdrant::r#match::MatchValue::Value(
                                            qdrant_client::qdrant::Value {
                                                kind: Some(
                                                    qdrant_client::qdrant::value::Kind::StringValue(
                                                        topic.to_string(),
                                                    ),
                                                ),
                                            },
                                        ),
                                    ),
                                }),
                                ..Default::default()
                            },
                        ),
                    ),
                },
            ],
            ..Default::default()
        };

        // Delete points matching the filter
        let delete_request = DeletePoints {
            collection_name: collection_name.to_string(),
            wait: Some(true),
            points: Some(PointsSelector {
                points_selector_one_of: Some(
                    PointsSelectorOneOf::Filter(filter),
                ),
            }),
            ..Default::default()
        };

        match self.qdrant_client.delete_points(delete_request).await {
            Ok(result) => {
                let deleted_count = result.result.map(|r| r.points_count as usize).unwrap_or(0);
                Ok(deleted_count)
            }
            Err(e) => Err(format!("Failed to delete points from Qdrant: {}", e)),
        }
    }

    /// Get node volumes (for heat map visualization)
    pub async fn get_node_volumes(&self) -> HashMap<String, usize> {
        let volumes = self.node_volumes.read().await;
        volumes.clone()
    }

    /// Create snapshots for all Qdrant collections
    pub async fn create_mesh_snapshot(&self) -> Result<Vec<String>, String> {
        info!("Creating mesh-wide Qdrant snapshots");
        
        let collections = vec!["agent_logs", "telemetry", "playbooks", "quarantine_list"];
        let mut snapshot_paths = Vec::new();
        
        for collection_name in collections {
            match self.create_collection_snapshot(collection_name).await {
                Ok(path) => {
                    info!(
                        collection = %collection_name,
                        snapshot_path = %path,
                        "Snapshot created successfully"
                    );
                    snapshot_paths.push(path);
                }
                Err(e) => {
                    warn!(
                        collection = %collection_name,
                        error = %e,
                        "Failed to create snapshot for collection"
                    );
                    // Continue with other collections even if one fails
                }
            }
        }
        
        // Update last snapshot timestamp
        {
            let mut last_snapshot = self.last_snapshot.write().await;
            *last_snapshot = Some(std::time::SystemTime::now());
        }
        
        if snapshot_paths.is_empty() {
            return Err("Failed to create any snapshots".to_string());
        }
        
        Ok(snapshot_paths)
    }

    /// Create a snapshot for a specific collection
    async fn create_collection_snapshot(&self, collection_name: &str) -> Result<String, String> {
        use qdrant_client::qdrant::CreateSnapshot;
        
        let snapshot_request = CreateSnapshot {
            collection_name: collection_name.to_string(),
            ..Default::default()
        };
        
        match self.qdrant_client.create_snapshot(snapshot_request).await {
            Ok(response) => {
                let snapshot_name = response.name;
                // Qdrant snapshots are stored in the Qdrant data directory
                // The path format is typically: <collection_name>/snapshots/<snapshot_name>
                let snapshot_path = format!("{}/snapshots/{}", collection_name, snapshot_name);
                Ok(snapshot_path)
            }
            Err(e) => Err(format!("Failed to create snapshot for {}: {}", collection_name, e))
        }
    }

    /// Get last snapshot timestamp
    pub async fn get_last_snapshot_time(&self) -> Option<std::time::SystemTime> {
        let last_snapshot = self.last_snapshot.read().await;
        *last_snapshot
    }

    /// Check if a snapshot was taken recently (within last 60 minutes)
    pub async fn has_recent_snapshot(&self) -> bool {
        let last_snapshot = self.last_snapshot.read().await;
        if let Some(snapshot_time) = *last_snapshot {
            if let Ok(elapsed) = std::time::SystemTime::now().duration_since(snapshot_time) {
                return elapsed.as_secs() < 3600; // 60 minutes
            }
        }
        false
    }

    /// Enable maintenance mode (pauses PhoenixEvent bus traffic)
    pub async fn enable_maintenance_mode(&self) {
        let mut mode = self.maintenance_mode.write().await;
        *mode = true;
        info!("Maintenance mode enabled - PhoenixEvent bus traffic paused");
        
        // Publish maintenance event
        self.message_bus.publish(PhoenixEvent::MaintenanceStarted {
            operation: "memory_restore".to_string(),
            timestamp: Utc::now().to_rfc3339(),
        });
    }

    /// Disable maintenance mode (resumes PhoenixEvent bus traffic)
    pub async fn disable_maintenance_mode(&self) {
        let mut mode = self.maintenance_mode.write().await;
        *mode = false;
        info!("Maintenance mode disabled - PhoenixEvent bus traffic resumed");
    }

    /// Check if maintenance mode is enabled
    pub async fn is_maintenance_mode(&self) -> bool {
        let mode = self.maintenance_mode.read().await;
        *mode
    }

    /// List all available snapshots for all collections
    pub async fn list_snapshots(&self) -> Result<Vec<SnapshotInfo>, String> {
        // Try using Qdrant client's list_snapshots if available, otherwise use REST API
        let collections = vec!["agent_logs", "telemetry", "playbooks", "quarantine_list"];
        let mut all_snapshots = Vec::new();
        
        // Get Qdrant URL from environment or use default
        // Qdrant REST API uses port 6333, but QDRANT_URL might point to gRPC (6334)
        // Convert gRPC URL to REST URL if needed
        let qdrant_url = std::env::var("QDRANT_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:6333".to_string())
            .replace(":6334", ":6333") // Convert gRPC port to REST port
            .replace("http://", "http://")
            .replace("https://", "https://");
        
        for collection_name in collections {
            // Try REST API approach (more reliable)
            let snapshot_url = format!("{}/collections/{}/snapshots", qdrant_url, collection_name);
            
            match reqwest::Client::new().get(&snapshot_url).send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        match response.json::<serde_json::Value>().await {
                            Ok(json) => {
                                if let Some(result) = json.get("result") {
                                    if let Some(snapshots_array) = result.get("snapshots") {
                                        if let Some(snapshots_list) = snapshots_array.as_array() {
                                            for snapshot_json in snapshots_list {
                                                if let (Some(name), Some(size)) = (
                                                    snapshot_json.get("name").and_then(|n| n.as_str()),
                                                    snapshot_json.get("size").and_then(|s| s.as_u64()),
                                                ) {
                                                    // Parse creation time if available
                                                    let creation_time = snapshot_json
                                                        .get("creation_time")
                                                        .and_then(|ct| ct.as_str())
                                                        .map(|s| s.to_string())
                                                        .unwrap_or_else(|| {
                                                            chrono::Utc::now().to_rfc3339()
                                                        });
                                                    
                                                    // Parse creation time to DateTime for comparison
                                                    let snapshot_time = chrono::DateTime::parse_from_rfc3339(&creation_time)
                                                        .map(|dt| dt.with_timezone(&chrono::Utc))
                                                        .ok();
                                                    
                                                    // Calculate compliance metadata
                                                    // For now, we'll set defaults - in a full implementation,
                                                    // this would cross-reference with ComplianceMonitor logs
                                                    let (compliance_score, is_recommended, is_blessed) = 
                                                        calculate_snapshot_metadata(snapshot_time);
                                                    
                                                    all_snapshots.push(SnapshotInfo {
                                                        snapshot_id: name.to_string(),
                                                        collection_name: collection_name.to_string(),
                                                        creation_time,
                                                        size: size.unwrap_or(0),
                                                        compliance_score,
                                                        is_recommended,
                                                        is_blessed,
                                                    });
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                warn!(
                                    collection = %collection_name,
                                    error = %e,
                                    "Failed to parse snapshot list response"
                                );
                            }
                        }
                    } else {
                        // If REST API fails, try gRPC client method as fallback
                        // Note: This may not be available in all Qdrant client versions
                        warn!(
                            collection = %collection_name,
                            status = %response.status(),
                            "REST API failed, snapshots may not be available for this collection"
                        );
                    }
                }
                Err(e) => {
                    warn!(
                        collection = %collection_name,
                        error = %e,
                        "Failed to fetch snapshots via REST API"
                    );
                }
            }
        }
        
        // Sort by creation time (newest first)
        all_snapshots.sort_by(|a, b| b.creation_time.cmp(&a.creation_time));
        
        // Mark recommended recovery points (snapshots taken before major compliance dips)
        mark_recommended_recovery_points(&mut all_snapshots);
        
        Ok(all_snapshots)
    }
}

/// Calculate compliance metadata for a snapshot
/// In a full implementation, this would cross-reference with ComplianceMonitor logs
fn calculate_snapshot_metadata(
    snapshot_time: Option<chrono::DateTime<chrono::Utc>>,
) -> (Option<f64>, bool, bool) {
    // Placeholder: In production, this would:
    // 1. Query ComplianceMonitor for records around snapshot_time
    // 2. Calculate average compliance score
    // 3. Check if snapshot was taken before a major compliance dip
    
    // For now, return defaults that will be calculated properly when ComplianceMonitor is integrated
    (None, false, false)
}

/// Mark snapshots as recommended recovery points based on compliance drift detection
fn mark_recommended_recovery_points(snapshots: &mut [SnapshotInfo]) {
    // In a full implementation, this would:
    // 1. Cross-reference snapshot timestamps with ComplianceMonitor logs
    // 2. Identify snapshots taken immediately before major compliance dips
    // 3. Mark them as is_recommended = true
    
    // For now, mark snapshots with high compliance scores as recommended
    for snapshot in snapshots.iter_mut() {
        if let Some(score) = snapshot.compliance_score {
            if score >= 95.0 {
                snapshot.is_blessed = true;
                snapshot.is_recommended = true;
            } else if score >= 80.0 {
                snapshot.is_recommended = true;
            }
        }
    }

    /// Restore from a snapshot
    pub async fn restore_from_snapshot(&self, snapshot_id: &str, collection_name: &str) -> Result<(), String> {
        // Verify snapshot exists
        let snapshots = self.list_snapshots().await?;
        let snapshot_exists = snapshots.iter().any(|s| 
            s.snapshot_id == snapshot_id && s.collection_name == collection_name
        );
        
        if !snapshot_exists {
            return Err(format!("Snapshot {} not found for collection {}", snapshot_id, collection_name));
        }

        // Enable maintenance mode before restore
        self.enable_maintenance_mode().await;
        
        // Wait a moment for bus traffic to pause
        tokio::time::sleep(Duration::from_secs(2)).await;
        
        info!(
            snapshot_id = %snapshot_id,
            collection = %collection_name,
            "Starting snapshot restore"
        );

        // Qdrant restore is done via the REST API recover endpoint
        // This requires the snapshot file to be available in Qdrant's snapshot directory
        // Convert gRPC URL to REST URL if needed
        let qdrant_url = std::env::var("QDRANT_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:6333".to_string())
            .replace(":6334", ":6333") // Convert gRPC port to REST port
            .replace("http://", "http://")
            .replace("https://", "https://");
        
        let recover_url = format!(
            "{}/collections/{}/snapshots/{}/recover",
            qdrant_url, collection_name, snapshot_id
        );
        
        // Attempt to recover from snapshot via REST API
        match reqwest::Client::new()
            .put(&recover_url)
            .send()
            .await
        {
            Ok(response) => {
                if response.status().is_success() {
                    info!(
                        snapshot_id = %snapshot_id,
                        collection = %collection_name,
                        "Snapshot restore completed successfully"
                    );
                    
                    // Wait a moment for Qdrant to process the restore
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    
                    // Disable maintenance mode after successful restore
                    self.disable_maintenance_mode().await;
                    Ok(())
                } else {
                    let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                    error!(
                        snapshot_id = %snapshot_id,
                        collection = %collection_name,
                        status = %response.status(),
                        error = %error_text,
                        "Failed to restore snapshot"
                    );
                    
                    // Disable maintenance mode even on failure
                    self.disable_maintenance_mode().await;
                    Err(format!("Qdrant restore failed: {} - {}", response.status(), error_text))
                }
            }
            Err(e) => {
                error!(
                    snapshot_id = %snapshot_id,
                    collection = %collection_name,
                    error = %e,
                    "Failed to send restore request to Qdrant"
                );
                
                // Disable maintenance mode even on failure
                self.disable_maintenance_mode().await;
                Err(format!("Failed to connect to Qdrant: {}", e))
            }
        }
    }
}

/// Snapshot information
#[derive(Debug, Clone)]
pub struct SnapshotInfo {
    pub snapshot_id: String,
    pub collection_name: String,
    pub creation_time: String,
    pub size: u64,
    /// Compliance score at time of snapshot (0-100)
    pub compliance_score: Option<f64>,
    /// Whether this snapshot is recommended as a recovery point
    pub is_recommended: bool,
    /// Whether this snapshot has 95%+ compliance (blessed state)
    pub is_blessed: bool,
}

/// Get the gRPC server for Phoenix Memory Exchange
pub fn get_memory_exchange_server(
    service: PhoenixMemoryExchangeServiceImpl,
) -> PhoenixMemoryExchangeServiceServer<PhoenixMemoryExchangeServiceImpl> {
    PhoenixMemoryExchangeServiceServer::new(service)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::bus::GlobalMessageBus;

    #[tokio::test]
    async fn test_memory_transfer_event_includes_redaction_count() {
        // This test verifies that MemoryTransfer events include redacted_entities_count
        let message_bus = Arc::new(GlobalMessageBus::new());
        let mut receiver = message_bus.subscribe();

        // Create a test MemoryTransfer event
        let test_event = PhoenixEvent::MemoryTransfer {
            source_node: "node-a".to_string(),
            destination_node: "node-b".to_string(),
            topic: "test-topic".to_string(),
            fragments_count: 5,
            bytes_transferred: 1024,
            redacted_entities_count: 42,
            timestamp: chrono::Utc::now().to_rfc3339(),
        };

        // Publish the event
        message_bus.publish(test_event.clone());

        // Receive and verify the event
        let received = receiver.recv().await.unwrap();
        match received {
            PhoenixEvent::MemoryTransfer {
                source_node,
                destination_node,
                topic,
                fragments_count,
                bytes_transferred,
                redacted_entities_count,
                ..
            } => {
                assert_eq!(source_node, "node-a");
                assert_eq!(destination_node, "node-b");
                assert_eq!(topic, "test-topic");
                assert_eq!(fragments_count, 5);
                assert_eq!(bytes_transferred, 1024);
                assert_eq!(redacted_entities_count, 42, "Redaction count should be preserved in event");
            }
            _ => panic!("Received wrong event type"),
        }
    }

    #[tokio::test]
    async fn test_memory_transfer_event_includes_redaction_count() {
        // This test verifies that MemoryTransfer events include redacted_entities_count
        let message_bus = Arc::new(GlobalMessageBus::new());
        let mut receiver = message_bus.subscribe();

        // Create a test MemoryTransfer event
        let test_event = PhoenixEvent::MemoryTransfer {
            source_node: "node-a".to_string(),
            destination_node: "node-b".to_string(),
            topic: "test-topic".to_string(),
            fragments_count: 5,
            bytes_transferred: 1024,
            redacted_entities_count: 42,
            timestamp: chrono::Utc::now().to_rfc3339(),
        };

        // Publish the event
        message_bus.publish(test_event.clone());

        // Receive and verify the event
        let received = receiver.recv().await.unwrap();
        match received {
            PhoenixEvent::MemoryTransfer {
                source_node,
                destination_node,
                topic,
                fragments_count,
                bytes_transferred,
                redacted_entities_count,
                ..
            } => {
                assert_eq!(source_node, "node-a");
                assert_eq!(destination_node, "node-b");
                assert_eq!(topic, "test-topic");
                assert_eq!(fragments_count, 5);
                assert_eq!(bytes_transferred, 1024);
                assert_eq!(redacted_entities_count, 42, "Redaction count should be preserved in event");
            }
            _ => panic!("Received wrong event type"),
        }
    }
}
