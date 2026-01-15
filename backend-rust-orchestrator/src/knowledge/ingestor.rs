//! Auto-Domain Ingestor
//!
//! Automatically classifies and ingests files into the correct knowledge domain
//! (MIND, BODY, HEART, SOUL) based on semantic analysis.

use crate::knowledge::domain_router::{DomainRouter, KnowledgeDomain};
use notify::{Watcher, RecommendedWatcher, RecursiveMode, Event, EventKind};
use qdrant_client::{
    qdrant::{
        PointStruct, UpsertPoints, CreateCollection, VectorsConfig, VectorParams,
        vectors_config::Config, Distance, HnswConfigDiff, SparseVectorsConfig, SparseVectorParams,
        PointId, point_id::PointIdOptions,
    },
    Qdrant,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{info, warn, error};
use uuid::Uuid;

/// Ingestor service state
#[derive(Clone)]
pub struct AutoIngestor {
    /// Qdrant client for vector storage
    qdrant_client: Arc<Qdrant>,
    /// Domain router for collection mapping
    domain_router: DomainRouter,
    /// Watch directory path
    pub watch_dir: PathBuf,
    /// Embedding dimension
    embedding_dim: usize,
    /// LLM settings for classification (optional)
    llm_settings: Option<LLMSettings>,
    /// Processing status
    status: Arc<RwLock<IngestionStatus>>,
    /// Performance tracking per domain
    performance_tracker: Arc<RwLock<HashMap<KnowledgeDomain, Vec<Duration>>>>,
}

/// LLM settings for semantic classification
#[derive(Clone, Debug)]
pub struct LLMSettings {
    pub provider: String,
    pub url: String,
    pub api_key: String,
    pub model: String,
}

/// Performance statistics for a domain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceStats {
    /// Average time to ingest a file (milliseconds)
    pub avg_time_ms: f64,
    /// Minimum time to ingest (milliseconds)
    pub min_time_ms: f64,
    /// Maximum time to ingest (milliseconds)
    pub max_time_ms: f64,
    /// Total files processed
    pub total_files: usize,
}

/// Performance metrics per domain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainPerformanceMetrics {
    pub mind: PerformanceStats,
    pub body: PerformanceStats,
    pub heart: PerformanceStats,
    pub soul: PerformanceStats,
}

/// Ingestion status tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestionStatus {
    pub is_active: bool,
    pub files_processed: usize,
    pub files_failed: usize,
    pub current_file: Option<String>,
    pub last_error: Option<String>,
    /// Performance metrics: time-to-ingest per domain (in milliseconds)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub performance_metrics: Option<DomainPerformanceMetrics>,
}

impl Default for IngestionStatus {
    fn default() -> Self {
        Self {
            is_active: false,
            files_processed: 0,
            files_failed: 0,
            current_file: None,
            last_error: None,
            performance_metrics: None,
        }
    }
}

impl Default for PerformanceStats {
    fn default() -> Self {
        Self {
            avg_time_ms: 0.0,
            min_time_ms: 0.0,
            max_time_ms: 0.0,
            total_files: 0,
        }
    }
}

/// File classification result
#[derive(Debug, Clone)]
struct ClassificationResult {
    domain: KnowledgeDomain,
    confidence: f64,
    reasoning: String,
}

impl AutoIngestor {
    /// Create a new Auto-Domain Ingestor
    pub fn new(
        qdrant_client: Arc<Qdrant>,
        watch_dir: PathBuf,
        embedding_dim: usize,
        llm_settings: Option<LLMSettings>,
    ) -> Self {
        Self {
            qdrant_client,
            domain_router: DomainRouter::new(),
            watch_dir,
            embedding_dim,
            llm_settings,
            status: Arc::new(RwLock::new(IngestionStatus::default())),
            performance_tracker: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get current ingestion status
    pub async fn get_status(&self) -> IngestionStatus {
        self.status.read().await.clone()
    }

    /// Start watching the directory for new files
    pub async fn start_watching(&self) -> Result<(), String> {
        use notify::Watcher as _;
        use tokio::sync::mpsc;

        // Create channel for file events
        let (tx, mut rx) = mpsc::unbounded_channel();

        // Create watcher
        let mut watcher: RecommendedWatcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                let _ = tx.send(event);
            }
        })
        .map_err(|e| format!("Failed to create file watcher: {}", e))?;

        // Watch the directory
        watcher
            .watch(&self.watch_dir, RecursiveMode::Recursive)
            .map_err(|e| format!("Failed to watch directory {}: {}", self.watch_dir.display(), e))?;

        info!(
            watch_dir = %self.watch_dir.display(),
            "Auto-Domain Ingestor started watching directory"
        );

        // Spawn task to process file events
        let ingestor = self.clone();
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                if let EventKind::Create(_) | EventKind::Modify(_) = event.kind {
                    for path in event.paths {
                        if path.is_file() {
                            let ingestor_clone = ingestor.clone();
                            tokio::spawn(async move {
                                if let Err(e) = ingestor_clone.process_file(&path).await {
                                    error!(
                                        file = %path.display(),
                                        error = %e,
                                        "Failed to process file"
                                    );
                                }
                            });
                        }
                    }
                }
            }
        });

        Ok(())
    }

    /// Process a single file: classify, chunk, and ingest
    pub async fn process_file(&self, file_path: &Path) -> Result<(), String> {
        let start_time = Instant::now();
        
        // Update status
        {
            let mut status = self.status.write().await;
            status.is_active = true;
            status.current_file = Some(file_path.display().to_string());
            status.last_error = None;
        }

        // Read file content
        let content = tokio::fs::read_to_string(file_path)
            .await
            .map_err(|e| format!("Failed to read file {}: {}", file_path.display(), e))?;

        if content.trim().is_empty() {
            warn!(file = %file_path.display(), "File is empty, skipping");
            return Ok(());
        }

        // Classify domain
        let classification = self.classify_domain(&content, file_path).await?;
        info!(
            file = %file_path.display(),
            domain = ?classification.domain,
            confidence = classification.confidence,
            "Classified file"
        );

        // Chunk content based on domain
        let chunks = self.chunk_content(&content, classification.domain)?;

        // Ingest chunks into Qdrant
        let collections = self.domain_router.get_collections(classification.domain);
        if collections.is_empty() {
            return Err(format!(
                "No collections configured for domain {:?}",
                classification.domain
            ));
        }

        // Use the first collection (primary collection for the domain)
        let collection_name = &collections[0];
        self.ensure_collection(collection_name).await?;

        let mut success_count = 0;
        for (chunk_idx, chunk) in chunks.iter().enumerate() {
            match self.upsert_chunk(
                collection_name,
                chunk,
                file_path,
                chunk_idx,
                classification.domain,
            )
            .await
            {
                Ok(_) => success_count += 1,
                Err(e) => {
                    error!(
                        chunk_idx = chunk_idx,
                        error = %e,
                        "Failed to upsert chunk"
                    );
                }
            }
        }

        let elapsed = start_time.elapsed();
        
        // Track performance metrics
        {
            let mut tracker = self.performance_tracker.write().await;
            tracker.entry(classification.domain).or_insert_with(Vec::new).push(elapsed);
            
            // Update performance metrics in status (keep last 100 measurements per domain)
            let domain_times = tracker.get(&classification.domain).cloned().unwrap_or_default();
            let recent_times: Vec<Duration> = domain_times.iter().rev().take(100).cloned().collect();
            
            if !recent_times.is_empty() {
                let avg_ms = recent_times.iter().map(|d| d.as_millis() as f64).sum::<f64>() / recent_times.len() as f64;
                let min_ms = recent_times.iter().map(|d| d.as_millis() as f64).min().unwrap_or(0.0);
                let max_ms = recent_times.iter().map(|d| d.as_millis() as f64).max().unwrap_or(0.0);
                
                let mut status = self.status.write().await;
                let metrics = status.performance_metrics.get_or_insert_with(|| DomainPerformanceMetrics {
                    mind: PerformanceStats::default(),
                    body: PerformanceStats::default(),
                    heart: PerformanceStats::default(),
                    soul: PerformanceStats::default(),
                });
                
                let stats = match classification.domain {
                    KnowledgeDomain::Mind => &mut metrics.mind,
                    KnowledgeDomain::Body => &mut metrics.body,
                    KnowledgeDomain::Heart => &mut metrics.heart,
                    KnowledgeDomain::Soul => &mut metrics.soul,
                };
                
                stats.avg_time_ms = avg_ms;
                stats.min_time_ms = min_ms;
                stats.max_time_ms = max_ms;
                stats.total_files = recent_times.len();
            }
        }

        // Update status
        {
            let mut status = self.status.write().await;
            if success_count == chunks.len() {
                status.files_processed += 1;
            } else {
                status.files_failed += 1;
                status.last_error = Some(format!(
                    "Only {}/{} chunks ingested successfully",
                    success_count,
                    chunks.len()
                ));
            }
            status.current_file = None;
            status.is_active = false;
        }

        info!(
            file = %file_path.display(),
            chunks_ingested = success_count,
            total_chunks = chunks.len(),
            collection = collection_name,
            elapsed_ms = elapsed.as_millis(),
            domain = ?classification.domain,
            "File ingestion completed"
        );

        Ok(())
    }

    /// Classify file content into a domain using LLM or keyword fallback
    async fn classify_domain(
        &self,
        content: &str,
        file_path: &Path,
    ) -> Result<ClassificationResult, String> {
        // Extract first 500 tokens (approximate) for classification
        let preview = self.extract_preview(content, 500);

        // Try LLM classification if available
        if let Some(ref llm_settings) = self.llm_settings {
            if let Ok(result) = self.classify_with_llm(&preview, llm_settings).await {
                return Ok(result);
            }
            warn!("LLM classification failed, falling back to keyword-based classification");
        }

        // Fallback to keyword-based classification
        self.classify_with_keywords(&preview, file_path)
    }

    /// Classify using LLM
    async fn classify_with_llm(
        &self,
        preview: &str,
        llm_settings: &LLMSettings,
    ) -> Result<ClassificationResult, String> {
        let system_prompt = r#"You are a semantic classifier for a knowledge base system. 
Analyze the provided content and classify it into one of four domains:

- MIND: Technical/Intellectual content (specs, procedures, playbooks, code, technical documentation)
- BODY: Physical/Operational content (logs, telemetry, system state, hardware metrics, performance data)
- HEART: Emotional/Personal content (user preferences, personas, interaction history, personal context)
- SOUL: Ethical/Governance content (security audits, compliance, governance, safety, ethics, leadership wisdom)

Respond with JSON only:
{
  "domain": "mind|body|heart|soul",
  "confidence": 0.0-1.0,
  "reasoning": "brief explanation"
}"#;

        let user_content = format!("Content preview:\n\n{}", preview);

        // Make LLM request
        let payload = serde_json::json!({
            "model": llm_settings.model,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": user_content}
            ],
            "response_format": {
                "type": "json_object"
            },
            "temperature": 0.2
        });

        let client = reqwest::Client::new();
        let response = client
            .post(&llm_settings.url)
            .header("Authorization", format!("Bearer {}", llm_settings.api_key))
            .header("Content-Type", "application/json")
            .header("HTTP-Referer", "phoenix-agi-auto-ingestor")
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("LLM API request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(format!(
                "LLM API returned error status {}: {}",
                status, error_text
            ));
        }

        let api_response: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse LLM response: {}", e))?;

        // Extract content from response
        let content_str = self.extract_llm_content(&api_response)?;

        // Parse JSON response
        let llm_result: serde_json::Value = serde_json::from_str(&content_str)
            .map_err(|e| format!("Failed to parse LLM JSON: {}", e))?;

        let domain_str = llm_result
            .get("domain")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing 'domain' in LLM response".to_string())?;

        let domain = match domain_str.to_lowercase().as_str() {
            "mind" => KnowledgeDomain::Mind,
            "body" => KnowledgeDomain::Body,
            "heart" => KnowledgeDomain::Heart,
            "soul" => KnowledgeDomain::Soul,
            _ => return Err(format!("Invalid domain '{}' from LLM", domain_str)),
        };

        let confidence = llm_result
            .get("confidence")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.5);

        let reasoning = llm_result
            .get("reasoning")
            .and_then(|v| v.as_str())
            .unwrap_or("No reasoning provided")
            .to_string();

        Ok(ClassificationResult {
            domain,
            confidence,
            reasoning,
        })
    }

    /// Extract content from LLM response (OpenRouter-compatible)
    fn extract_llm_content(&self, api_response: &serde_json::Value) -> Result<String, String> {
        if let Some(err) = api_response.get("error") {
            return Err(format!("LLM returned error: {}", err));
        }

        let choice0 = api_response
            .get("choices")
            .and_then(|choices| choices.as_array())
            .and_then(|arr| arr.first())?;

        if let Some(content) = choice0
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
        {
            if let Some(s) = content.as_str() {
                return Ok(s.to_string());
            }
        }

        Err("Failed to extract content from LLM response".to_string())
    }

    /// Classify using keyword matching (fallback)
    fn classify_with_keywords(
        &self,
        preview: &str,
        file_path: &Path,
    ) -> Result<ClassificationResult, String> {
        let preview_lower = preview.to_lowercase();
        let file_name_lower = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();

        let combined = format!("{} {}", preview_lower, file_name_lower);

        // Score each domain
        let mut scores: HashMap<KnowledgeDomain, f64> = HashMap::new();
        scores.insert(KnowledgeDomain::Mind, 0.0);
        scores.insert(KnowledgeDomain::Body, 0.0);
        scores.insert(KnowledgeDomain::Heart, 0.0);
        scores.insert(KnowledgeDomain::Soul, 0.0);

        // Mind keywords
        let mind_keywords = [
            "spec", "procedure", "playbook", "technical", "code", "api", "config",
            "documentation", "how to", "tutorial", "guide", "manual",
        ];
        for keyword in &mind_keywords {
            if combined.contains(keyword) {
                *scores.get_mut(&KnowledgeDomain::Mind).unwrap() += 1.0;
            }
        }

        // Body keywords
        let body_keywords = [
            "log", "telemetry", "metric", "performance", "cpu", "memory", "disk",
            "system", "hardware", "state", "status", "monitor", "alert",
        ];
        for keyword in &body_keywords {
            if combined.contains(keyword) {
                *scores.get_mut(&KnowledgeDomain::Body).unwrap() += 1.0;
            }
        }

        // Heart keywords
        let heart_keywords = [
            "user", "persona", "preference", "personal", "interaction", "feedback",
            "customer", "client", "profile",
        ];
        for keyword in &heart_keywords {
            if combined.contains(keyword) {
                *scores.get_mut(&KnowledgeDomain::Heart).unwrap() += 1.0;
            }
        }

        // Soul keywords
        let soul_keywords = [
            "security", "audit", "compliance", "governance", "ethics", "safety",
            "policy", "guardrail", "risk", "threat", "vulnerability", "breach",
            "leadership", "corporate", "governance",
        ];
        for keyword in &soul_keywords {
            if combined.contains(keyword) {
                *scores.get_mut(&KnowledgeDomain::Soul).unwrap() += 1.0;
            }
        }

        // Find domain with highest score
        let (domain, &score) = scores
            .iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .ok_or_else(|| "Failed to determine domain".to_string())?;

        let total_score: f64 = scores.values().sum();
        let confidence = if total_score > 0.0 {
            score / total_score
        } else {
            0.5 // Default confidence if no keywords matched
        };

        Ok(ClassificationResult {
            domain: *domain,
            confidence,
            reasoning: format!("Keyword-based classification (score: {:.2})", score),
        })
    }

    /// Extract preview text (first N tokens, approximate)
    fn extract_preview(&self, content: &str, max_tokens: usize) -> String {
        // Simple token approximation: split on whitespace
        let tokens: Vec<&str> = content.split_whitespace().collect();
        let take_count = tokens.len().min(max_tokens);
        tokens[..take_count].join(" ")
    }

    /// Chunk content based on domain-specific chunk sizes
    fn chunk_content(
        &self,
        content: &str,
        domain: KnowledgeDomain,
    ) -> Result<Vec<String>, String> {
        let chunk_size = match domain {
            KnowledgeDomain::Body => 256,  // Small chunks for precise log matching
            KnowledgeDomain::Soul => 1024, // Large chunks to preserve ethical context
            KnowledgeDomain::Mind => 512,  // Medium chunks for technical docs
            KnowledgeDomain::Heart => 512, // Medium chunks for user context
        };

        // Simple token-based chunking (split on whitespace)
        let tokens: Vec<&str> = content.split_whitespace().collect();
        let mut chunks = Vec::new();

        for chunk_tokens in tokens.chunks(chunk_size) {
            chunks.push(chunk_tokens.join(" "));
        }

        if chunks.is_empty() {
            chunks.push(content.to_string());
        }

        Ok(chunks)
    }

    /// Ensure Qdrant collection exists
    async fn ensure_collection(&self, collection_name: &str) -> Result<(), String> {
        // Check if collection exists
        let collections = self
            .qdrant_client
            .list_collections()
            .await
            .map_err(|e| format!("Failed to list collections: {}", e))?;

        let exists = collections
            .collections
            .iter()
            .any(|c| c.name == collection_name);

        if exists {
            return Ok(());
        }

        // Create collection
        info!(
            collection = collection_name,
            embedding_dim = self.embedding_dim,
            "Creating Qdrant collection"
        );

        let hnsw_config = HnswConfigDiff {
            m: Some(16),
            ef_construct: Some(100),
            full_scan_threshold: None,
            max_indexing_threads: None,
            on_disk: None,
            payload_m: None,
        };

        let sparse_vectors_config = SparseVectorsConfig {
            map: [(
                "text-sparse".to_string(),
                SparseVectorParams {
                    index: Some(qdrant_client::qdrant::SparseIndexParams {
                        full_scan_threshold: Some(10000),
                        on_disk: None,
                    }),
                },
            )]
            .iter()
            .cloned()
            .collect(),
        };

        let create_collection = CreateCollection {
            collection_name: collection_name.to_string(),
            vectors_config: Some(VectorsConfig {
                config: Some(Config::Params(VectorParams {
                    size: self.embedding_dim as u64,
                    distance: Distance::Cosine as i32,
                    hnsw_config: Some(hnsw_config),
                    ..Default::default()
                })),
            }),
            sparse_vectors_config: Some(sparse_vectors_config),
            ..Default::default()
        };

        self.qdrant_client
            .create_collection(create_collection)
            .await
            .map_err(|e| format!("Failed to create collection: {}", e))?;

        Ok(())
    }

    /// Upsert a chunk into Qdrant
    async fn upsert_chunk(
        &self,
        collection_name: &str,
        chunk: &str,
        file_path: &Path,
        chunk_idx: usize,
        domain: KnowledgeDomain,
    ) -> Result<(), String> {
        // Generate embedding
        let embedding = self.generate_embedding(chunk).await?;

        // Generate sparse vector
        let sparse_vector = self.generate_sparse_vector(chunk);

        // Create point ID
        let point_id = Uuid::new_v4();
        let point_id_value = PointId {
            point_id_options: Some(PointIdOptions::Uuid(point_id.to_string())),
        };

        // Build payload
        let mut payload: HashMap<String, qdrant_client::qdrant::Value> = HashMap::new();
        payload.insert(
            "content".to_string(),
            qdrant_string_value(chunk.to_string()),
        );
        payload.insert(
            "file_path".to_string(),
            qdrant_string_value(file_path.display().to_string()),
        );
        payload.insert(
            "file_name".to_string(),
            qdrant_string_value(
                file_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string(),
            ),
        );
        payload.insert(
            "chunk_index".to_string(),
            qdrant_string_value(chunk_idx.to_string()),
        );
        payload.insert(
            "domain".to_string(),
            qdrant_string_value(format!("{:?}", domain)),
        );
        payload.insert(
            "ingested_at".to_string(),
            qdrant_string_value(chrono::Utc::now().to_rfc3339()),
        );

        // Create point
        let point = PointStruct {
            id: Some(point_id_value),
            vectors: Some(qdrant_client::qdrant::Vectors {
                vectors_options: Some(
                    qdrant_client::qdrant::vectors::VectorsOptions::Dense(
                        qdrant_client::qdrant::DenseVector { data: embedding },
                    ),
                ),
            }),
            payload,
        };

        // Upsert
        self.qdrant_client
            .upsert_points(UpsertPoints {
                collection_name: collection_name.to_string(),
                points: vec![point],
                ..Default::default()
            })
            .await
            .map_err(|e| format!("Failed to upsert point: {}", e))?;

        Ok(())
    }

    /// Generate embedding using fastembed (same pattern as phoenix_routes)
    async fn generate_embedding(&self, text: &str) -> Result<Vec<f32>, String> {
        use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};
        use std::sync::{Arc, Mutex};
        use std::sync::OnceLock;

        static EMBEDDING_MODEL: OnceLock<Option<Arc<Mutex<TextEmbedding>>>> = OnceLock::new();

        let model_opt = EMBEDDING_MODEL.get_or_init(|| {
            let model_name = std::env::var("EMBEDDING_MODEL_NAME")
                .unwrap_or_else(|_| "all-MiniLM-L6-v2".to_string());

            let model_type: EmbeddingModel = match model_name.as_str() {
                "all-MiniLM-L6-v2" | "sentence-transformers/all-MiniLM-L6-v2" => {
                    EmbeddingModel::AllMiniLML6V2
                }
                "BAAI/bge-small-en-v1.5" => EmbeddingModel::BGESmallENV15,
                "BAAI/bge-base-en-v1.5" => EmbeddingModel::BGEBaseENV15,
                _ => EmbeddingModel::AllMiniLML6V2,
            };

            let init_options = TextInitOptions::new(model_type).with_show_download_progress(false);

            match TextEmbedding::try_new(init_options) {
                Ok(model) => {
                    info!("Embedding model initialized for ingestor");
                    Some(Arc::new(Mutex::new(model)))
                }
                Err(e) => {
                    warn!(error = %e, "Failed to initialize embedding model");
                    None
                }
            }
        });

        if let Some(model_arc) = model_opt {
            const MAX_TEXT_CHARS: usize = 8000; // Allow longer text for chunks
            let truncated_text = if text.len() > MAX_TEXT_CHARS {
                &text[..MAX_TEXT_CHARS]
            } else {
                text
            };

            match tokio::task::spawn_blocking({
                let model_arc = model_arc.clone();
                let text = truncated_text.to_string();
                move || {
                    let model = model_arc.lock().unwrap();
                    model.embed(vec![text.as_str()], None)
                }
            })
            .await
            {
                Ok(Ok(embeddings)) => {
                    if let Some(embedding) = embeddings.first() {
                        if embedding.len() == self.embedding_dim {
                            return Ok(embedding.clone());
                        }
                    }
                }
                Ok(Err(e)) => {
                    warn!(error = %e, "Embedding generation failed");
                }
                Err(e) => {
                    warn!(error = %e, "Task join error during embedding");
                }
            }
        }

        // Fallback to hash-based embedding
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut embedding = vec![0.0f32; self.embedding_dim];
        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        let hash = hasher.finish();

        for i in 0..self.embedding_dim {
            let hash_val = ((hash >> (i % 64)) & 0xFFFF) as f32 / 65536.0;
            embedding[i] = hash_val;
        }

        Ok(embedding)
    }

    /// Generate sparse vector for hybrid search
    fn generate_sparse_vector(&self, text: &str) -> qdrant_client::qdrant::SparseVector {
        use std::collections::HashMap;
        use qdrant_client::qdrant::{SparseIndices, SparseVector};

        let tokens: Vec<String> = text
            .to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();

        let mut term_freq: HashMap<String, f32> = HashMap::new();
        for token in &tokens {
            *term_freq.entry(token.clone()).or_insert(0.0) += 1.0;
        }

        let mut indices = Vec::new();
        let mut values = Vec::new();

        for (token, freq) in term_freq {
            let hash = token
                .as_bytes()
                .iter()
                .fold(0u64, |acc, &b| acc.wrapping_mul(31).wrapping_add(b as u64));
            let index = (hash % 10000) as u32;
            let value = freq.sqrt();

            indices.push(index);
            values.push(value);
        }

        SparseVector {
            indices: Some(SparseIndices { data: indices }),
            values,
        }
    }
}

/// Helper to convert string to Qdrant Value
fn qdrant_string_value(s: String) -> qdrant_client::qdrant::Value {
    qdrant_client::qdrant::Value {
        kind: Some(qdrant_client::qdrant::value::Kind::StringValue(s)),
    }
}
