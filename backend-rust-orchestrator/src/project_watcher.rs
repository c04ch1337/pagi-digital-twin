use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use notify::{RecommendedWatcher, Watcher, Event, EventKind, RecursiveMode, Config};
use tokio::sync::mpsc;
use tracing::{info, warn, error};
use tonic::transport::Channel;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};

// Import memory client types from main.rs
use crate::memory_client::memory_service_client::MemoryServiceClient;
use crate::memory_client::{CommitMemoryRequest, CommitMemoryResponse};
use tonic::Request;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileProcessingEvent {
    pub project_id: String,
    pub project_name: String,
    pub file_path: String,
    pub file_name: String,
    pub file_type: String,
    pub status: String, // "success", "error", "skipped"
    pub error_message: Option<String>,
    pub memory_id: Option<String>,
    pub namespace: String,
    pub timestamp: String, // ISO 8601
    pub file_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileProcessingStats {
    pub project_id: String,
    pub project_name: String,
    pub total_processed: u64,
    pub successful: u64,
    pub failed: u64,
    pub skipped: u64,
    pub last_processed: Option<String>, // ISO 8601 timestamp
    pub recent_events: Vec<FileProcessingEvent>, // Last 50 events
}

pub struct ProjectWatcher {
    watchers: Arc<RwLock<HashMap<String, RecommendedWatcher>>>,
    watch_paths: Arc<RwLock<HashMap<String, PathBuf>>>,
    project_names: Arc<RwLock<HashMap<String, String>>>, // project_id -> project_name
    memory_client: Arc<RwLock<Option<MemoryServiceClient<Channel>>>>, // Optional memory client for auto-committing
    processed_files: Arc<RwLock<HashMap<String, u64>>>, // file_path -> last_modified timestamp to avoid reprocessing
    processing_events: Arc<RwLock<Vec<FileProcessingEvent>>>, // Processing history (keep last 1000)
    processing_stats: Arc<RwLock<HashMap<String, (u64, u64, u64)>>>, // project_id -> (total, success, failed)
}

// Manual Clone implementation since RecommendedWatcher doesn't implement Clone
impl Clone for ProjectWatcher {
    fn clone(&self) -> Self {
        Self {
            watchers: Arc::clone(&self.watchers),
            watch_paths: Arc::clone(&self.watch_paths),
            project_names: Arc::clone(&self.project_names),
            memory_client: Arc::clone(&self.memory_client),
            processed_files: Arc::clone(&self.processed_files),
            processing_events: Arc::clone(&self.processing_events),
            processing_stats: Arc::clone(&self.processing_stats),
        }
    }
}

impl ProjectWatcher {
    pub fn new() -> Self {
        Self {
            watchers: Arc::new(RwLock::new(HashMap::new())),
            watch_paths: Arc::new(RwLock::new(HashMap::new())),
            project_names: Arc::new(RwLock::new(HashMap::new())),
            memory_client: Arc::new(RwLock::new(None)),
            processed_files: Arc::new(RwLock::new(HashMap::new())),
            processing_events: Arc::new(RwLock::new(Vec::new())),
            processing_stats: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Record a file processing event
    async fn record_event(&self, event: FileProcessingEvent) {
        let mut events = self.processing_events.write().await;
        events.push(event.clone());
        
        // Keep only last 1000 events
        if events.len() > 1000 {
            events.remove(0);
        }

        // Update statistics
        let mut stats = self.processing_stats.write().await;
        let (total, success, failed) = stats
            .entry(event.project_id.clone())
            .or_insert((0, 0, 0));
        *total += 1;
        match event.status.as_str() {
            "success" => *success += 1,
            "error" => *failed += 1,
            _ => {}
        }
    }

    /// Get processing statistics for all projects or a specific project
    pub async fn get_processing_stats(&self, project_id: Option<&str>) -> Vec<FileProcessingStats> {
        let events = self.processing_events.read().await;
        let stats = self.processing_stats.read().await;
        let names = self.project_names.read().await;

        let mut result = Vec::new();

        // Get project IDs to process
        let project_ids: Vec<String> = if let Some(pid) = project_id {
            vec![pid.to_string()]
        } else {
            stats.keys().cloned().collect()
        };

        for pid in project_ids {
            let project_name = names.get(&pid).cloned().unwrap_or_else(|| pid.clone());
            let (total, success, failed) = stats.get(&pid).copied().unwrap_or((0, 0, 0));
            let skipped = total - success - failed;

            // Get recent events for this project (last 50)
            let recent: Vec<FileProcessingEvent> = events
                .iter()
                .rev()
                .filter(|e| e.project_id == pid)
                .take(50)
                .cloned()
                .collect();

            let last_processed = recent.first().map(|e| e.timestamp.clone());

            result.push(FileProcessingStats {
                project_id: pid,
                project_name,
                total_processed: total,
                successful: success,
                failed,
                skipped,
                last_processed,
                recent_events: recent.into_iter().rev().collect(), // Reverse to get chronological order
            });
        }

        result
    }

    /// Set the memory client for automatic file processing
    pub async fn set_memory_client(&self, client: MemoryServiceClient<Channel>) {
        let mut mc = self.memory_client.write().await;
        *mc = Some(client);
    }

    /// Detect file type based on extension and content
    fn detect_file_type(file_path: &Path) -> (&'static str, &'static str) {
        let ext = file_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        
        match ext.as_str() {
            "log" | "txt" => ("log", "RAGSource"),
            "eml" | "msg" => ("email", "RAGSource"),
            "json" | "xml" | "csv" => ("data", "RAGSource"),
            "pdf" => ("document", "RAGSource"),
            _ => ("file", "RAGSource"),
        }
    }

    /// Detect critical security patterns in file content
    /// Returns (is_critical, risk_level, detected_patterns)
    fn detect_critical_patterns(content: &str) -> (bool, &'static str, Vec<&'static str>) {
        let critical_patterns = vec![
            ("CRITICAL", "Critical"),
            ("ALERT", "High"),
            ("SECURITY BREACH", "Critical"),
            ("UNAUTHORIZED ACCESS", "High"),
            ("MALWARE DETECTED", "Critical"),
            ("IOC", "High"), // Indicator of Compromise
            ("C2", "High"), // Command and Control
            ("BEACON", "High"),
            ("EXPLOIT", "High"),
            ("VULNERABILITY", "Medium"),
            ("FAILED LOGIN", "Medium"),
            ("BRUTE FORCE", "High"),
            ("SQL INJECTION", "High"),
            ("XSS", "High"),
            ("PHISHING", "High"),
        ];

        let mut detected = Vec::new();
        let content_upper = content.to_uppercase();
        let mut highest_risk = "Low";

        for (pattern, risk) in critical_patterns {
            if content_upper.contains(pattern) {
                detected.push(pattern);
                // Determine highest risk level
                match (highest_risk, risk) {
                    ("Critical", _) => {}
                    (_, "Critical") => highest_risk = "Critical",
                    ("High", _) => {}
                    (_, "High") => highest_risk = "High",
                    ("Medium", _) => {}
                    (_, "Medium") => highest_risk = "Medium",
                    _ => {}
                }
            }
        }

        let is_critical = !detected.is_empty();
        (is_critical, highest_risk, detected)
    }

    /// Process a detected file: read content and commit to memory
    async fn process_file(
        &self,
        file_path: &Path,
        project_id: &str,
        project_name: &str,
    ) -> Result<(), String> {
        // Check if we've already processed this file recently
        let file_path_str = file_path.to_string_lossy().to_string();
        let metadata = tokio::fs::metadata(file_path).await
            .map_err(|e| {
                // Record error event
                let event = FileProcessingEvent {
                    project_id: project_id.to_string(),
                    project_name: project_name.to_string(),
                    file_path: file_path_str.clone(),
                    file_name: file_path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string(),
                    file_type: "unknown".to_string(),
                    status: "error".to_string(),
                    error_message: Some(format!("Failed to get file metadata: {}", e)),
                    memory_id: None,
                    namespace: format!("{}_logs", project_id.replace("-", "_")),
                    timestamp: Utc::now().to_rfc3339(),
                    file_size: 0,
                };
                let watcher = self.clone();
                tokio::spawn(async move {
                    watcher.record_event(event).await;
                });
                format!("Failed to get file metadata: {}", e)
            })?;
        
        let file_size = metadata.len();
        let modified = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        {
            let processed = self.processed_files.read().await;
            if let Some(&last_processed) = processed.get(&file_path_str) {
                // Only reprocess if file was modified after last processing
                if modified <= last_processed {
                    info!(
                        file = %file_path.display(),
                        "File already processed, skipping"
                    );
                    // Record skipped event
                    let event = FileProcessingEvent {
                        project_id: project_id.to_string(),
                        project_name: project_name.to_string(),
                        file_path: file_path_str.clone(),
                        file_name: file_path.file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unknown")
                            .to_string(),
                        file_type: "unknown".to_string(),
                        status: "skipped".to_string(),
                        error_message: Some("File already processed".to_string()),
                        memory_id: None,
                        namespace: format!("{}_logs", project_id.replace("-", "_")),
                        timestamp: Utc::now().to_rfc3339(),
                        file_size,
                    };
                    let watcher = self.clone();
                    tokio::spawn(async move {
                        watcher.record_event(event).await;
                    });
                    return Ok(());
                }
            }
        }

        // Read file content (limit to 1MB to avoid memory issues)
        let content = match tokio::fs::read(file_path).await {
            Ok(bytes) => {
                if bytes.len() > 1_000_000 {
                    warn!(
                        file = %file_path.display(),
                        size = bytes.len(),
                        "File too large, truncating to 1MB"
                    );
                    String::from_utf8_lossy(&bytes[..1_000_000]).to_string()
                } else {
                    String::from_utf8_lossy(&bytes).to_string()
                }
            }
            Err(e) => {
                return Err(format!("Failed to read file: {}", e));
            }
        };

        if content.trim().is_empty() {
            warn!(file = %file_path.display(), "File is empty, skipping");
            // Record skipped event
            let event = FileProcessingEvent {
                project_id: project_id.to_string(),
                project_name: project_name.to_string(),
                file_path: file_path_str.clone(),
                file_name: file_path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string(),
                file_type: "unknown".to_string(),
                status: "skipped".to_string(),
                error_message: Some("File is empty".to_string()),
                memory_id: None,
                namespace: format!("{}_logs", project_id.replace("-", "_")),
                timestamp: Utc::now().to_rfc3339(),
                file_size,
            };
            let watcher = self.clone();
            tokio::spawn(async move {
                watcher.record_event(event).await;
            });
            return Ok(());
        }

        // Detect file type
        let (file_type, memory_type) = Self::detect_file_type(file_path);
        
        // Detect critical patterns
        let (is_critical, risk_level, detected_patterns) = Self::detect_critical_patterns(&content);
        
        // Create namespace from project_id (e.g., "rapid7-siem" -> "rapid7_siem_logs")
        let namespace = format!("{}_logs", project_id.replace("-", "_"));

        // Prepare content for memory commit
        let file_name = file_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        
        let mut memory_content = format!(
            "File: {}\nSource: {}\nType: {}\n\nContent:\n{}",
            file_name,
            project_name,
            file_type,
            content
        );

        // Add critical alert header if patterns detected
        if is_critical {
            let patterns_str = detected_patterns.join(", ");
            memory_content = format!(
                "🚨 CRITICAL SECURITY ALERT 🚨\nRisk Level: {}\nDetected Patterns: {}\n\n{}",
                risk_level,
                patterns_str,
                memory_content
            );
            warn!(
                project_id = %project_id,
                project_name = %project_name,
                file = %file_path.display(),
                risk_level = %risk_level,
                patterns = %patterns_str,
                "Critical security patterns detected in file"
            );
        }

        // Commit to memory if client is available
        let memory_client_opt = self.memory_client.read().await.clone();
        if let Some(memory_client) = memory_client_opt {
            let mut client = memory_client;
            let request = Request::new(CommitMemoryRequest {
                content: memory_content,
                namespace: namespace.clone(),
                twin_id: "twin-aegis".to_string(), // The Blue Flame orchestrator
                memory_type: memory_type.to_string(),
                risk_level: risk_level.to_string(),
                metadata: {
                    let mut meta = HashMap::new();
                    meta.insert("source".to_string(), "file_watcher".to_string());
                    meta.insert("project_id".to_string(), project_id.to_string());
                    meta.insert("project_name".to_string(), project_name.to_string());
                    meta.insert("file_path".to_string(), file_path_str.clone());
                    meta.insert("file_type".to_string(), file_type.to_string());
                    meta.insert("file_name".to_string(), file_name.to_string());
                    meta.insert("is_critical".to_string(), is_critical.to_string());
                    if !detected_patterns.is_empty() {
                        meta.insert("detected_patterns".to_string(), detected_patterns.join(", "));
                    }
                    meta
                },
            });

            match client.commit_memory(request).await {
                Ok(response) => {
                    let resp: CommitMemoryResponse = response.into_inner();
                    if resp.success {
                        info!(
                            project_id = %project_id,
                            project_name = %project_name,
                            file = %file_path.display(),
                            memory_id = %resp.memory_id,
                            namespace = %namespace,
                            "File processed and committed to memory"
                        );
                        
                        // Mark as processed
                        {
                            let mut processed = self.processed_files.write().await;
                            processed.insert(file_path_str.clone(), modified);
                        }

                        // Record success event
                        let event = FileProcessingEvent {
                            project_id: project_id.to_string(),
                            project_name: project_name.to_string(),
                            file_path: file_path_str.clone(),
                            file_name: file_name.to_string(),
                            file_type: file_type.to_string(),
                            status: "success".to_string(),
                            error_message: None,
                            memory_id: Some(resp.memory_id),
                            namespace: namespace.clone(),
                            timestamp: Utc::now().to_rfc3339(),
                            file_size,
                        };
                        let watcher = self.clone();
                        tokio::spawn(async move {
                            watcher.record_event(event).await;
                        });
                    } else {
                        warn!(
                            file = %file_path.display(),
                            error = %resp.error_message,
                            "Failed to commit file to memory"
                        );
                        // Record error event
                        let event = FileProcessingEvent {
                            project_id: project_id.to_string(),
                            project_name: project_name.to_string(),
                            file_path: file_path_str.clone(),
                            file_name: file_name.to_string(),
                            file_type: file_type.to_string(),
                            status: "error".to_string(),
                            error_message: Some(resp.error_message),
                            memory_id: None,
                            namespace: namespace.clone(),
                            timestamp: Utc::now().to_rfc3339(),
                            file_size,
                        };
                        let watcher = self.clone();
                        tokio::spawn(async move {
                            watcher.record_event(event).await;
                        });
                    }
                }
                Err(e) => {
                    warn!(
                        file = %file_path.display(),
                        error = %e,
                        "gRPC call failed when committing file to memory"
                    );
                    // Record error event
                    let event = FileProcessingEvent {
                        project_id: project_id.to_string(),
                        project_name: project_name.to_string(),
                        file_path: file_path_str.clone(),
                        file_name: file_name.to_string(),
                        file_type: file_type.to_string(),
                        status: "error".to_string(),
                        error_message: Some(format!("gRPC error: {}", e)),
                        memory_id: None,
                        namespace: namespace.clone(),
                        timestamp: Utc::now().to_rfc3339(),
                        file_size,
                    };
                    let watcher = self.clone();
                    tokio::spawn(async move {
                        watcher.record_event(event).await;
                    });
                }
            }
        } else {
            info!(
                project_id = %project_id,
                file = %file_path.display(),
                "File detected but memory client not configured, skipping auto-commit"
            );
            // Record skipped event
            let event = FileProcessingEvent {
                project_id: project_id.to_string(),
                project_name: project_name.to_string(),
                file_path: file_path_str.clone(),
                file_name: file_name.to_string(),
                file_type: file_type.to_string(),
                status: "skipped".to_string(),
                error_message: Some("Memory client not configured".to_string()),
                memory_id: None,
                namespace: namespace.clone(),
                timestamp: Utc::now().to_rfc3339(),
                file_size,
            };
            let watcher = self.clone();
            tokio::spawn(async move {
                watcher.record_event(event).await;
            });
        }

        Ok(())
    }

    /// Register a project folder to watch
    pub async fn watch_project_folder(
        &self,
        project_id: &str,
        project_name: &str,
        watch_path: &str,
    ) -> Result<(), String> {
        let path = PathBuf::from(watch_path);
        
        // Validate path exists
        if !path.exists() {
            return Err(format!("Path does not exist: {}", watch_path));
        }
        
        if !path.is_dir() {
            return Err(format!("Path is not a directory: {}", watch_path));
        }

        // Remove existing watcher if any
        self.unwatch_project(project_id).await;

        // Create channel for file events
        let (tx, mut rx) = mpsc::unbounded_channel::<Result<Event, notify::Error>>();
        
        // Create watcher
        let tx_clone = tx.clone();
        let mut watcher = RecommendedWatcher::new(
            move |result: Result<Event, notify::Error>| {
                if tx_clone.send(result).is_err() {
                    // Channel closed
                }
            },
            Config::default(),
        )
        .map_err(|e| format!("Failed to create watcher: {}", e))?;

        // Start watching (recursive to catch subdirectories)
        watcher
            .watch(&path, RecursiveMode::Recursive)
            .map_err(|e| format!("Failed to watch directory: {}", e))?;

        // Store watcher and path
        {
            let mut watchers = self.watchers.write().await;
            watchers.insert(project_id.to_string(), watcher);
        }
        {
            let mut paths = self.watch_paths.write().await;
            paths.insert(project_id.to_string(), path.clone());
        }
        {
            let mut names = self.project_names.write().await;
            names.insert(project_id.to_string(), project_name.to_string());
        }

        info!(
            project_id = %project_id,
            project_name = %project_name,
            path = %path.display(),
            "Started watching project folder"
        );

        // Spawn task to process file events
        let project_id_clone = project_id.to_string();
        let project_name_clone = project_name.to_string();
        let watcher_self = self.clone();
        tokio::spawn(async move {
            while let Some(event_result) = rx.recv().await {
                match event_result {
                    Ok(event) => {
                        // Process file events (new files, modifications)
                        if let EventKind::Create(_) | EventKind::Modify(_) = event.kind {
                            for file_path in event.paths {
                                if file_path.is_file() {
                                    info!(
                                        project_id = %project_id_clone,
                                        project_name = %project_name_clone,
                                        file = %file_path.display(),
                                        "New file detected in project folder"
                                    );
                                    
                                    // Small delay to avoid processing files that are still being written
                                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                                    
                                    // Process the file (read, extract, commit to memory)
                                    if let Err(e) = watcher_self.process_file(
                                        &file_path,
                                        &project_id_clone,
                                        &project_name_clone,
                                    ).await {
                                        warn!(
                                            project_id = %project_id_clone,
                                            file = %file_path.display(),
                                            error = %e,
                                            "Failed to process file"
                                        );
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!(
                            project_id = %project_id_clone,
                            error = %e,
                            "File watcher error"
                        );
                    }
                }
            }
        });

        Ok(())
    }

    /// Stop watching a project folder
    pub async fn unwatch_project(&self, project_id: &str) {
        let mut watchers = self.watchers.write().await;
        if let Some(mut watcher) = watchers.remove(project_id) {
            if let Some(path) = self.watch_paths.read().await.get(project_id) {
                let _ = watcher.unwatch(path);
            }
            info!(project_id = %project_id, "Stopped watching project folder");
        }
        {
            let mut paths = self.watch_paths.write().await;
            paths.remove(project_id);
        }
        {
            let mut names = self.project_names.write().await;
            names.remove(project_id);
        }
    }

    /// Get all watched paths
    pub async fn get_watched_paths(&self) -> HashMap<String, PathBuf> {
        self.watch_paths.read().await.clone()
    }

    /// Get all project watch configurations
    pub async fn get_all_configs(&self) -> HashMap<String, (String, PathBuf)> {
        let paths = self.watch_paths.read().await;
        let names = self.project_names.read().await;
        let mut result = HashMap::new();
        for (id, path) in paths.iter() {
            if let Some(name) = names.get(id) {
                result.insert(id.clone(), (name.clone(), path.clone()));
            }
        }
        result
    }
}
