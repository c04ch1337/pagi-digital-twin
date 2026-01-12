use std::env;
use tonic::{transport::Server, Request, Response, Status};
use tracing::{info, error, warn};
use uuid::Uuid;
use chrono::Utc;
use rand::Rng;
use std::time::Duration;
use tokio::sync::RwLock;

// Include the generated proto code
pub mod proto {
    tonic::include_proto!("memory");
}

use proto::memory_service_server::{MemoryService, MemoryServiceServer};
use proto::{
    CommitMemoryRequest, CommitMemoryResponse, HealthCheckRequest, HealthCheckResponse,
    QueryMemoryRequest, QueryMemoryResponse, MemoryResult,
    ListMemoriesRequest, ListMemoriesResponse,
    DeleteMemoryRequest, DeleteMemoryResponse,
};

// Qdrant imports
use qdrant_client::{
    qdrant::{
        vectors_config::Config, CreateCollection, Distance, PointStruct,
        ScoredPoint, SearchPoints, VectorParams, VectorsConfig, Value, Match, UpsertPoints,
        ScrollPoints, DeletePoints, PointId, PointsSelector, points_selector::PointsSelectorOneOf,
        PointsIdsList,
    },
    Qdrant,
};
use std::collections::HashMap;

use std::sync::Arc;

#[derive(Clone)]
pub struct MemoryServiceImpl {
    /// Qdrant client (preferred in production). When unavailable and explicitly allowed,
    /// the service can fall back to a lightweight in-memory store for local dev.
    qdrant_client: Option<Arc<Qdrant>>,
    in_memory_store: Arc<RwLock<Vec<InMemoryRecord>>>,
    embedding_dim: usize,
}

#[derive(Clone, Debug)]
struct InMemoryRecord {
    id: String,
    timestamp: String,
    content: String,
    twin_id: String,
    risk_level: String,
    memory_type: String,
    namespace: String,
    metadata: HashMap<String, String>,
}

impl MemoryServiceImpl {
    async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // Load environment variables
        dotenvy::dotenv().ok();

        let memory_backend = env::var("MEMORY_BACKEND").unwrap_or_else(|_| "qdrant".to_string());
        let allow_in_memory_fallback = env::var("QDRANT_REQUIRED")
            .map(|v| v.to_lowercase() != "true")
            .unwrap_or(false);

        // Get Qdrant URL from environment
        let qdrant_url = env::var("QDRANT_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:6334".to_string());

        // Get embedding dimension from environment
        let embedding_dim = env::var("EMBEDDING_MODEL_DIM")
            .unwrap_or_else(|_| "384".to_string())
            .parse::<usize>()
            .expect("EMBEDDING_MODEL_DIM must be a valid integer");

        // Always initialize the in-memory store so the service can run in dev even if Qdrant is unavailable.
        let in_memory_store = Arc::new(RwLock::new(Vec::<InMemoryRecord>::new()));

        if memory_backend.to_lowercase() == "mock" || memory_backend.to_lowercase() == "memory" {
            info!(
                memory_backend = %memory_backend,
                embedding_dim = embedding_dim,
                "Starting Memory service with in-memory backend"
            );

            return Ok(Self {
                qdrant_client: None,
                in_memory_store,
                embedding_dim,
            });
        }

        info!(
            qdrant_url = %qdrant_url,
            embedding_dim = embedding_dim,
            allow_in_memory_fallback = allow_in_memory_fallback,
            "Initializing Qdrant client"
        );

        // Initialize Qdrant client.
        // We do a short retry loop to handle startup ordering (e.g., service starts before Qdrant is ready).
        let qdrant_api_key = env::var("QDRANT_API_KEY").ok();
        let client = Qdrant::from_url(&qdrant_url).api_key(qdrant_api_key).build()?;

        let max_attempts: u32 = env::var("QDRANT_CONNECT_ATTEMPTS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(10);
        let retry_delay_ms: u64 = env::var("QDRANT_CONNECT_RETRY_DELAY_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(1000);

        let mut connected = false;
        for attempt in 1..=max_attempts {
            match client.health_check().await {
                Ok(_) => {
                    connected = true;
                    break;
                }
                Err(e) => {
                    warn!(
                        attempt = attempt,
                        max_attempts = max_attempts,
                        retry_delay_ms = retry_delay_ms,
                        qdrant_url = %qdrant_url,
                        error = %e,
                        "Qdrant health_check failed; retrying"
                    );
                    tokio::time::sleep(Duration::from_millis(retry_delay_ms)).await;
                }
            }
        }

        if !connected {
            let msg = format!(
                "Failed to connect to Qdrant at {} after {} attempts",
                qdrant_url, max_attempts
            );

            if allow_in_memory_fallback {
                warn!(
                    qdrant_url = %qdrant_url,
                    "Qdrant unavailable; falling back to in-memory backend (set QDRANT_REQUIRED=true to fail fast)"
                );

                return Ok(Self {
                    qdrant_client: None,
                    in_memory_store,
                    embedding_dim,
                });
            }

            return Err(msg.into());
        }

        info!(qdrant_url = %qdrant_url, "Successfully connected to Qdrant");

        let qdrant_client = Arc::new(client);
        Self::spawn_qdrant_heartbeat(Arc::clone(&qdrant_client), qdrant_url.clone());

        Ok(Self {
            qdrant_client: Some(qdrant_client),
            in_memory_store,
            embedding_dim,
        })
    }

    fn spawn_qdrant_heartbeat(qdrant: Arc<Qdrant>, qdrant_url: String) {
        // Background heartbeat that continuously checks connectivity.
        // This does not stop the server; it provides runtime observability.
        tokio::task::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(15));
            let mut was_healthy = true;

            loop {
                interval.tick().await;
                match qdrant.health_check().await {
                    Ok(_) => {
                        if !was_healthy {
                            info!(qdrant_url = %qdrant_url, "Qdrant connectivity restored");
                        }
                        was_healthy = true;
                    }
                    Err(e) => {
                        if was_healthy {
                            // CRITICAL: connectivity to the mandatory vector DB was lost.
                            // We do not crash immediately; this is an ops signal.
                            error!(
                                qdrant_url = %qdrant_url,
                                error = %e,
                                "CRITICAL: Lost connectivity to Qdrant"
                            );
                        }
                        was_healthy = false;
                    }
                }
            }
        });
    }

    /// Generate a mock embedding vector for the given text
    /// In production, this would call an embedding model service
    fn generate_mock_embedding(&self, _text: &str) -> Vec<f32> {
        let mut rng = rand::thread_rng();
        (0..self.embedding_dim)
            .map(|_| rng.gen_range(-1.0..1.0))
            .collect()
    }

    /// Ensure a Qdrant collection exists for the given namespace
    async fn ensure_collection(&self, namespace: &str) -> Result<(), Status> {
        // In-memory backend doesn't require collection management.
        if self.qdrant_client.is_none() {
            return Ok(());
        }

        let collection_name = namespace;

        let qdrant = self
            .qdrant_client
            .as_ref()
            .expect("qdrant_client checked above")
            .as_ref();

        // Check if collection exists
        let collections = qdrant
            .list_collections()
            .await
            .map_err(|e| {
                error!(error = %e, namespace = %namespace, "Failed to list collections");
                Status::internal(format!("Failed to list collections: {}", e))
            })?;

        let collection_exists = collections
            .collections
            .iter()
            .any(|c| c.name == collection_name);

        if !collection_exists {
            info!(
                namespace = %namespace,
                collection = %collection_name,
                embedding_dim = self.embedding_dim,
                "Creating Qdrant collection"
            );

            let create_collection = CreateCollection {
                collection_name: collection_name.to_string(),
                vectors_config: Some(VectorsConfig {
                    config: Some(Config::Params(VectorParams {
                        size: self.embedding_dim as u64,
                        distance: Distance::Cosine as i32,
                        ..Default::default()
                    })),
                }),
                ..Default::default()
            };

            qdrant
                .create_collection(create_collection)
                .await
                .map_err(|e| {
                    error!(
                        error = %e,
                        namespace = %namespace,
                        "Failed to create collection"
                    );
                    Status::internal(format!("Failed to create collection: {}", e))
                })?;

            info!(
                namespace = %namespace,
                "Collection created successfully"
            );
        }

        Ok(())
    }

    /// Extract string value from Qdrant Value
    fn extract_string_value(value: &Value) -> String {
        use qdrant_client::qdrant::value::Kind;

        match value.kind.as_ref() {
            Some(Kind::StringValue(s)) => s.clone(),
            Some(Kind::IntegerValue(i)) => i.to_string(),
            Some(Kind::DoubleValue(d)) => d.to_string(),
            Some(Kind::BoolValue(b)) => b.to_string(),
            _ => String::new(),
        }
    }
}

#[tonic::async_trait]
impl MemoryService for MemoryServiceImpl {
    async fn query_memory(
        &self,
        request: Request<QueryMemoryRequest>,
    ) -> Result<Response<QueryMemoryResponse>, Status> {
        let req = request.into_inner();
        let query = req.query;
        let namespace = req.namespace;
        let twin_id = req.twin_id;
        let top_k = req.top_k.max(1).min(100) as u64; // Clamp between 1 and 100

        info!(
            query = %query,
            namespace = %namespace,
            top_k = top_k,
            "Querying memory"
        );

        // In-memory backend path (dev): naive substring scoring.
        if self.qdrant_client.is_none() {
            let q = query.to_lowercase();
            let terms: Vec<&str> = q.split_whitespace().filter(|t| !t.is_empty()).collect();

            let store = self.in_memory_store.read().await;
            let mut scored: Vec<(f64, InMemoryRecord)> = store
                .iter()
                .filter(|r| r.namespace == namespace)
                .filter(|r| twin_id.is_empty() || r.twin_id == twin_id)
                .map(|r| {
                    let haystack = r.content.to_lowercase();
                    let matches = terms.iter().filter(|t| haystack.contains(**t)).count();
                    let similarity = if terms.is_empty() {
                        0.0
                    } else {
                        (matches as f64) / (terms.len() as f64)
                    };
                    (similarity, r.clone())
                })
                .collect();

            scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

            let results: Vec<MemoryResult> = scored
                .into_iter()
                .take(top_k as usize)
                .map(|(similarity, r)| MemoryResult {
                    id: r.id,
                    timestamp: r.timestamp,
                    content: r.content,
                    agent_id: r.twin_id,
                    risk_level: r.risk_level,
                    similarity,
                    memory_type: r.memory_type,
                    metadata: r.metadata,
                })
                .collect();

            let total_count = results.len() as i32;

            info!(
                namespace = %namespace,
                result_count = results.len(),
                "Memory query completed (in-memory backend)"
            );

            return Ok(Response::new(QueryMemoryResponse {
                results,
                total_count,
                namespace,
            }));
        }

        // Ensure collection exists
        self.ensure_collection(&namespace).await?;

        // Generate mock embedding for query
        let query_vector = self.generate_mock_embedding(&query);

        // Build filter for twin_id if provided
        let filter = if !twin_id.is_empty() {
            Some(qdrant_client::qdrant::Filter {
                must: vec![qdrant_client::qdrant::Condition {
                    condition_one_of: Some(
                        qdrant_client::qdrant::condition::ConditionOneOf::Field(
                            qdrant_client::qdrant::FieldCondition {
                                key: "twin_id".to_string(),
                                r#match: Some(Match {
                                    match_value: Some(
                                        qdrant_client::qdrant::r#match::MatchValue::Keyword(
                                            twin_id.clone(),
                                        ),
                                    ),
                                }),
                                ..Default::default()
                            },
                        ),
                    ),
                }],
                ..Default::default()
            })
        } else {
            None
        };

        // Search in Qdrant
        let search_points = SearchPoints {
            collection_name: namespace.clone(),
            vector: query_vector,
            filter: filter.clone(),
            limit: top_k,
            with_payload: Some(true.into()),
            ..Default::default()
        };

        let qdrant = self
            .qdrant_client
            .as_ref()
            .expect("qdrant_client is required in this branch")
            .as_ref();

        let search_results = qdrant
            .search_points(search_points)
            .await
            .map_err(|e| {
                error!(
                    error = %e,
                    namespace = %namespace,
                    "Failed to search points in Qdrant"
                );
                Status::internal(format!("Failed to search memory: {}", e))
            })?;

        // Map Qdrant results to gRPC MemoryResult
        let results: Vec<MemoryResult> = search_results
            .result
            .into_iter()
            .map(|scored_point: ScoredPoint| {
                let payload = scored_point.payload;
                
                // Extract fields from payload
                let id = payload
                    .get("id")
                    .map(|v| Self::extract_string_value(v))
                    .unwrap_or_else(|| String::new());
                let timestamp = payload
                    .get("timestamp")
                    .map(|v| Self::extract_string_value(v))
                    .unwrap_or_else(|| String::new());
                let content = payload
                    .get("content")
                    .map(|v| Self::extract_string_value(v))
                    .unwrap_or_else(|| String::new());
                let agent_id = payload
                    .get("twin_id")
                    .map(|v| Self::extract_string_value(v))
                    .unwrap_or_else(|| String::new());
                let risk_level = payload
                    .get("risk_level")
                    .map(|v| Self::extract_string_value(v))
                    .unwrap_or_else(|| String::new());
                let memory_type = payload
                    .get("memory_type")
                    .map(|v| Self::extract_string_value(v))
                    .unwrap_or_else(|| String::new());

                // Extract metadata
                let mut metadata = HashMap::new();
                if let Some(meta) = payload.get("metadata") {
                    use qdrant_client::qdrant::value::Kind;
                    if let Some(Kind::StructValue(meta_struct)) = meta.kind.as_ref() {
                        for (k, v) in &meta_struct.fields {
                            metadata.insert(k.clone(), Self::extract_string_value(v));
                        }
                    }
                }

                // Convert Qdrant score (distance) to similarity (0.0 to 1.0)
                // Qdrant uses cosine distance, so we convert: similarity = 1.0 - distance
                // Clamp to ensure it's between 0.0 and 1.0
                let similarity = (1.0 - scored_point.score as f64).max(0.0).min(1.0);

                MemoryResult {
                    id,
                    timestamp,
                    content,
                    agent_id,
                    risk_level,
                    similarity,
                    memory_type,
                    metadata,
                }
            })
            .collect();

        info!(
            namespace = %namespace,
            result_count = results.len(),
            "Memory query completed"
        );

        let total_count = results.len() as i32;

        Ok(Response::new(QueryMemoryResponse {
            results,
            total_count,
            namespace,
        }))
    }

    async fn commit_memory(
        &self,
        request: Request<CommitMemoryRequest>,
    ) -> Result<Response<CommitMemoryResponse>, Status> {
        let req = request.into_inner();

        let memory_id = Uuid::new_v4();
        let timestamp = Utc::now();

        info!(
            memory_id = %memory_id,
            namespace = %req.namespace,
            twin_id = %req.twin_id,
            "Committing memory"
        );

        // In-memory backend path (dev).
        if self.qdrant_client.is_none() {
            let record = InMemoryRecord {
                id: memory_id.to_string(),
                timestamp: timestamp.to_rfc3339(),
                content: req.content,
                twin_id: req.twin_id,
                risk_level: req.risk_level,
                memory_type: req.memory_type,
                namespace: req.namespace,
                metadata: req.metadata,
            };

            {
                let mut store = self.in_memory_store.write().await;
                store.push(record);
            }

            info!(memory_id = %memory_id, "Memory committed successfully (in-memory backend)");

            return Ok(Response::new(CommitMemoryResponse {
                memory_id: memory_id.to_string(),
                success: true,
                error_message: String::new(),
            }));
        }

        // Ensure collection exists
        self.ensure_collection(&req.namespace).await?;

        // Generate mock embedding for content
        let vector = self.generate_mock_embedding(&req.content);

        // Build payload with metadata - convert HashMap to Qdrant Value map
        let mut payload_map = HashMap::new();
        
        // Convert metadata HashMap to Qdrant Value map
        let mut metadata_map = HashMap::new();
        for (k, v) in &req.metadata {
            metadata_map.insert(
                k.clone(),
                Value {
                    kind: Some(qdrant_client::qdrant::value::Kind::StringValue(v.clone())),
                },
            );
        }

        payload_map.insert(
            "id".to_string(),
            Value {
                kind: Some(qdrant_client::qdrant::value::Kind::StringValue(memory_id.to_string())),
            },
        );
        payload_map.insert(
            "timestamp".to_string(),
            Value {
                kind: Some(qdrant_client::qdrant::value::Kind::StringValue(timestamp.to_rfc3339())),
            },
        );
        payload_map.insert(
            "content".to_string(),
            Value {
                kind: Some(qdrant_client::qdrant::value::Kind::StringValue(req.content)),
            },
        );
        payload_map.insert(
            "twin_id".to_string(),
            Value {
                kind: Some(qdrant_client::qdrant::value::Kind::StringValue(req.twin_id)),
            },
        );
        payload_map.insert(
            "memory_type".to_string(),
            Value {
                kind: Some(qdrant_client::qdrant::value::Kind::StringValue(req.memory_type)),
            },
        );
        payload_map.insert(
            "risk_level".to_string(),
            Value {
                kind: Some(qdrant_client::qdrant::value::Kind::StringValue(req.risk_level)),
            },
        );
        payload_map.insert(
            "metadata".to_string(),
            Value {
                kind: Some(qdrant_client::qdrant::value::Kind::StructValue(
                    qdrant_client::qdrant::Struct {
                        fields: metadata_map,
                    },
                )),
            },
        );

        // Create point for Qdrant
        let point = PointStruct::new(
            memory_id.to_string(),
            vector,
            payload_map,
        );

        // Upsert point to Qdrant
        let upsert_req = UpsertPoints {
            collection_name: req.namespace.clone(),
            points: vec![point],
            ..Default::default()
        };

        let qdrant = self
            .qdrant_client
            .as_ref()
            .expect("qdrant_client is required in this branch")
            .as_ref();

        qdrant
            .upsert_points(upsert_req)
            .await
            .map_err(|e| {
                error!(
                    error = %e,
                    memory_id = %memory_id,
                    namespace = %req.namespace,
                    "Failed to upsert point to Qdrant"
                );
                Status::internal(format!("Failed to commit memory: {}", e))
            })?;

        info!(
            memory_id = %memory_id,
            "Memory committed successfully"
        );

        Ok(Response::new(CommitMemoryResponse {
            memory_id: memory_id.to_string(),
            success: true,
            error_message: String::new(),
        }))
    }

    async fn list_memories(
        &self,
        request: Request<ListMemoriesRequest>,
    ) -> Result<Response<ListMemoriesResponse>, Status> {
        let req = request.into_inner();
        let namespace = req.namespace;
        let page = req.page.max(1);
        // Default to 50 if page_size is 0 or not provided, otherwise clamp between 1 and 1000
        let page_size = if req.page_size <= 0 {
            50
        } else {
            req.page_size.min(1000)
        } as u64;
        let twin_id = req.twin_id;

        info!(
            namespace = %namespace,
            page = page,
            page_size = page_size,
            "Listing memories"
        );

        // In-memory backend path (dev)
        if self.qdrant_client.is_none() {
            let store = self.in_memory_store.read().await;
            let mut filtered: Vec<&InMemoryRecord> = store
                .iter()
                .filter(|r| namespace.is_empty() || r.namespace == namespace)
                .filter(|r| twin_id.is_empty() || r.twin_id == twin_id)
                .collect();

            // Sort by timestamp descending (newest first)
            filtered.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

            let total_count = filtered.len() as i32;
            let total_pages = ((total_count as f64) / (page_size as f64)).ceil() as i32;
            let start_idx = ((page - 1) * page_size as i32) as usize;
            let end_idx = (start_idx + page_size as usize).min(filtered.len());

            let memories: Vec<MemoryResult> = filtered[start_idx..end_idx]
                .iter()
                .map(|r| MemoryResult {
                    id: r.id.clone(),
                    timestamp: r.timestamp.clone(),
                    content: r.content.clone(),
                    agent_id: r.twin_id.clone(),
                    risk_level: r.risk_level.clone(),
                    similarity: 1.0, // No similarity score for list operation
                    memory_type: r.memory_type.clone(),
                    metadata: r.metadata.clone(),
                })
                .collect();

            return Ok(Response::new(ListMemoriesResponse {
                memories,
                total_count,
                page,
                page_size: page_size as i32,
                total_pages,
                namespace,
            }));
        }

        // Qdrant backend path
        if namespace.is_empty() {
            return Err(Status::invalid_argument(
                "namespace is required when using Qdrant backend"
            ));
        }

        // Ensure collection exists
        self.ensure_collection(&namespace).await?;

        // Build filter for twin_id if provided
        let filter = if !twin_id.is_empty() {
            Some(qdrant_client::qdrant::Filter {
                must: vec![qdrant_client::qdrant::Condition {
                    condition_one_of: Some(
                        qdrant_client::qdrant::condition::ConditionOneOf::Field(
                            qdrant_client::qdrant::FieldCondition {
                                key: "twin_id".to_string(),
                                r#match: Some(Match {
                                    match_value: Some(
                                        qdrant_client::qdrant::r#match::MatchValue::Keyword(
                                            twin_id.clone(),
                                        ),
                                    ),
                                }),
                                ..Default::default()
                            },
                        ),
                    ),
                }],
                ..Default::default()
            })
        } else {
            None
        };

        let qdrant = self
            .qdrant_client
            .as_ref()
            .expect("qdrant_client is required in this branch")
            .as_ref();

        // For total count, we need to scroll through all points
        // Qdrant doesn't provide a direct count, so we'll scroll with a reasonable limit
        // and calculate pages based on what we get
        let page_size_u32 = page_size.min(u32::MAX as u64) as u32;
        let offset_u64 = ((page - 1) * page_size as i32) as u64;
        
        let scroll_request = ScrollPoints {
            collection_name: namespace.clone(),
            filter: filter.clone(),
            limit: Some(page_size_u32),
            offset: if offset_u64 > 0 { 
                // Qdrant offset is a PointId for pagination, but we can use None for first page
                // and handle pagination differently, or use the offset as a continuation token
                None // For now, use None and handle pagination via limit only
            } else { 
                None 
            },
            with_payload: Some(true.into()),
            with_vectors: Some(false.into()),
            ..Default::default()
        };

        let scroll_result = qdrant
            .scroll(scroll_request)
            .await
            .map_err(|e| {
                error!(
                    error = %e,
                    namespace = %namespace,
                    "Failed to scroll points in Qdrant"
                );
                Status::internal(format!("Failed to list memories: {}", e))
            })?;

        // Convert Qdrant points to MemoryResult
        // scroll() returns ScrollResponse which has a result field that is Vec<RetrievedPoint>
        let memories: Vec<MemoryResult> = scroll_result
            .result
            .into_iter()
            .map(|point| {
                let payload = point.payload;
                
                let id = payload
                    .get("id")
                    .map(|v| Self::extract_string_value(v))
                    .unwrap_or_else(|| {
                        // Fallback to point ID if id not in payload
                        match point.id {
                            Some(PointId { point_id_options: Some(qdrant_client::qdrant::point_id::PointIdOptions::Uuid(uuid)) }) => uuid,
                            Some(PointId { point_id_options: Some(qdrant_client::qdrant::point_id::PointIdOptions::Num(num)) }) => num.to_string(),
                            _ => String::new(),
                        }
                    });
                let timestamp = payload
                    .get("timestamp")
                    .map(|v| Self::extract_string_value(v))
                    .unwrap_or_else(|| String::new());
                let content = payload
                    .get("content")
                    .map(|v| Self::extract_string_value(v))
                    .unwrap_or_else(|| String::new());
                let agent_id = payload
                    .get("twin_id")
                    .map(|v| Self::extract_string_value(v))
                    .unwrap_or_else(|| String::new());
                let risk_level = payload
                    .get("risk_level")
                    .map(|v| Self::extract_string_value(v))
                    .unwrap_or_else(|| String::new());
                let memory_type = payload
                    .get("memory_type")
                    .map(|v| Self::extract_string_value(v))
                    .unwrap_or_else(|| String::new());

                let mut metadata = HashMap::new();
                if let Some(meta) = payload.get("metadata") {
                    use qdrant_client::qdrant::value::Kind;
                    if let Some(Kind::StructValue(meta_struct)) = meta.kind.as_ref() {
                        for (k, v) in &meta_struct.fields {
                            metadata.insert(k.clone(), Self::extract_string_value(v));
                        }
                    }
                }

                MemoryResult {
                    id,
                    timestamp,
                    content,
                    agent_id,
                    risk_level,
                    similarity: 1.0, // No similarity score for list operation
                    memory_type,
                    metadata,
                }
            })
            .collect();

        // For total count, we'll need to do a separate scroll to count
        // This is not ideal but Qdrant doesn't provide a direct count API
        // We'll estimate based on the result or do a full scroll (expensive for large collections)
        // For now, we'll use a heuristic: if we got a full page, there might be more
        let total_count = if memories.len() == page_size as usize {
            // Estimate: we got a full page, so there are at least this many
            // In production, you might want to do a full scroll or maintain a count
            (page * page_size as i32) + 1 // Conservative estimate
        } else {
            // We got less than a full page, so this is likely the total
            (page - 1) * page_size as i32 + memories.len() as i32
        };

        let total_pages = ((total_count as f64) / (page_size as f64)).ceil() as i32;

        info!(
            namespace = %namespace,
            result_count = memories.len(),
            total_count = total_count,
            "Memory list completed"
        );

        Ok(Response::new(ListMemoriesResponse {
            memories,
            total_count,
            page,
            page_size: page_size as i32,
            total_pages,
            namespace,
        }))
    }

    async fn delete_memory(
        &self,
        request: Request<DeleteMemoryRequest>,
    ) -> Result<Response<DeleteMemoryResponse>, Status> {
        let req = request.into_inner();
        let memory_id = req.memory_id;
        let namespace = req.namespace;

        warn!(
            memory_id = %memory_id,
            namespace = %namespace,
            "AUDIT: Memory deletion requested"
        );

        // In-memory backend path (dev)
        if self.qdrant_client.is_none() {
            let mut store = self.in_memory_store.write().await;
            let initial_len = store.len();
            store.retain(|r| r.id != memory_id);
            let deleted = store.len() < initial_len;

            if deleted {
                warn!(
                    memory_id = %memory_id,
                    namespace = %namespace,
                    "AUDIT: Memory deleted successfully (in-memory backend)"
                );
                return Ok(Response::new(DeleteMemoryResponse {
                    success: true,
                    error_message: String::new(),
                }));
            } else {
                return Ok(Response::new(DeleteMemoryResponse {
                    success: false,
                    error_message: format!("Memory with id {} not found", memory_id),
                }));
            }
        }

        // Qdrant backend path
        if namespace.is_empty() {
            return Err(Status::invalid_argument(
                "namespace is required when using Qdrant backend"
            ));
        }

        // Ensure collection exists
        self.ensure_collection(&namespace).await?;

        let qdrant = self
            .qdrant_client
            .as_ref()
            .expect("qdrant_client is required in this branch")
            .as_ref();

        // Convert memory_id to PointId
        // Try to parse as UUID first, otherwise treat as numeric
        let point_id = if let Ok(uuid) = uuid::Uuid::parse_str(&memory_id) {
            PointId {
                point_id_options: Some(
                    qdrant_client::qdrant::point_id::PointIdOptions::Uuid(uuid.to_string())
                ),
            }
        } else {
            // Try parsing as number
            if let Ok(num) = memory_id.parse::<u64>() {
                PointId {
                    point_id_options: Some(
                        qdrant_client::qdrant::point_id::PointIdOptions::Num(num)
                    ),
                }
            } else {
                return Err(Status::invalid_argument(
                    format!("Invalid memory_id format: {}", memory_id)
                ));
            }
        };

        let delete_request = DeletePoints {
            collection_name: namespace.clone(),
            points: Some(PointsSelector {
                points_selector_one_of: Some(
                    PointsSelectorOneOf::Points(
                        PointsIdsList {
                            ids: vec![point_id],
                        }
                    )
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
                    memory_id = %memory_id,
                    namespace = %namespace,
                    "Failed to delete point from Qdrant"
                );
                Status::internal(format!("Failed to delete memory: {}", e))
            })?;

        warn!(
            memory_id = %memory_id,
            namespace = %namespace,
            "AUDIT: Memory deleted successfully from Qdrant"
        );

        Ok(Response::new(DeleteMemoryResponse {
            success: true,
            error_message: String::new(),
        }))
    }

    async fn health_check(
        &self,
        _request: Request<HealthCheckRequest>,
    ) -> Result<Response<HealthCheckResponse>, Status> {
        let is_healthy = match self.qdrant_client.as_ref() {
            Some(qdrant) => qdrant.health_check().await.is_ok(),
            None => true,
        };

        Ok(Response::new(HealthCheckResponse {
            status: if is_healthy {
                "healthy".to_string()
            } else {
                "unhealthy".to_string()
            },
            version: env!("CARGO_PKG_VERSION").to_string(),
            message: if is_healthy {
                "Memory service is operational".to_string()
            } else {
                "Qdrant connection failed".to_string()
            },
        }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "backend_rust_memory=info,tonic=info,qdrant_client=info".into()),
        )
        .init();

    // Load environment variables
    dotenvy::dotenv().ok();

    // Get gRPC port from environment
    let grpc_port = env::var("MEMORY_GRPC_PORT")
        .unwrap_or_else(|_| "50052".to_string())
        .parse::<u16>()
        .expect("MEMORY_GRPC_PORT must be a valid port number");

    let addr = format!("0.0.0.0:{}", grpc_port)
        .parse()
        .expect("Invalid address");

    let service = MemoryServiceImpl::new().await?;

    info!(
        addr = %addr,
        port = grpc_port,
        qdrant_enabled = service.qdrant_client.is_some(),
        "Starting Memory gRPC server"
    );

    Server::builder()
        .add_service(MemoryServiceServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
