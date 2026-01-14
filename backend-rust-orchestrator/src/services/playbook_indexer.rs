//! Playbook Indexer Service
//!
//! Background worker that maintains an in-memory index of playbook files for fast search.
//! Uses the `notify` crate to watch for file changes and incrementally updates the index.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use notify::{Watcher, RecursiveMode, Event, EventKind};
use tracing::{error, info, warn};
use walkdir::WalkDir;

/// Indexed file entry with searchable content
#[derive(Debug, Clone)]
pub struct IndexedFile {
    pub path: PathBuf,
    pub content: String,
    pub lines: Vec<String>,
    pub last_modified: std::time::SystemTime,
}

/// In-memory index of playbook files
#[derive(Clone)]
pub struct PlaybookIndex {
    /// Map of file path to indexed content
    index: Arc<RwLock<HashMap<PathBuf, IndexedFile>>>,
    /// Root directory being indexed
    root_path: PathBuf,
    /// File extensions to index
    extensions: Vec<String>,
}

impl PlaybookIndex {
    /// Create a new playbook index
    pub fn new(root_path: PathBuf) -> Self {
        Self {
            index: Arc::new(RwLock::new(HashMap::new())),
            root_path,
            extensions: vec!["md".to_string(), "yaml".to_string(), "yml".to_string(), "py".to_string()],
        }
    }

    /// Build the initial index by scanning all files
    pub async fn build_index(&self) -> Result<usize, Box<dyn std::error::Error>> {
        info!(
            path = %self.root_path.display(),
            "Building playbook index"
        );

        let mut count = 0;
        let mut new_index = HashMap::new();

        // Walk directory and index all matching files
        let entries = WalkDir::new(&self.root_path)
            .into_iter()
            .filter_map(|e| e.ok());

        for entry in entries {
            let path = entry.path();
            
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if self.extensions.iter().any(|e| e.eq_ignore_ascii_case(ext)) {
                        match self.index_file(path).await {
                            Ok(Some(indexed)) => {
                                new_index.insert(path.to_path_buf(), indexed);
                                count += 1;
                            }
                            Ok(None) => {
                                // File couldn't be read, skip it
                            }
                            Err(e) => {
                                warn!(
                                    path = %path.display(),
                                    error = %e,
                                    "Failed to index file"
                                );
                            }
                        }
                    }
                }
            }
        }

        // Replace the entire index atomically
        *self.index.write().await = new_index;

        info!(
            files_indexed = count,
            "Playbook index built successfully"
        );

        Ok(count)
    }

    /// Index a single file
    async fn index_file(&self, path: &Path) -> Result<Option<IndexedFile>, Box<dyn std::error::Error>> {
        let metadata = tokio::fs::metadata(path).await?;
        let last_modified = metadata.modified()?;

        let content = match tokio::fs::read_to_string(path).await {
            Ok(content) => content,
            Err(e) => {
                warn!(
                    path = %path.display(),
                    error = %e,
                    "Failed to read file for indexing"
                );
                return Ok(None);
            }
        };

        let lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();

        Ok(Some(IndexedFile {
            path: path.to_path_buf(),
            content,
            lines,
            last_modified,
        }))
    }

    /// Update or add a file to the index
    pub async fn update_file(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        // Check if file matches our extensions
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if !self.extensions.iter().any(|e| e.eq_ignore_ascii_case(ext)) {
                return Ok(()); // Not a file we care about
            }
        } else {
            return Ok(()); // No extension
        }

        match self.index_file(path).await {
            Ok(Some(indexed)) => {
                let mut index = self.index.write().await;
                index.insert(path.to_path_buf(), indexed);
                info!(
                    path = %path.display(),
                    "Updated file in index"
                );
            }
            Ok(None) => {
                // File couldn't be read, remove from index if it exists
                let mut index = self.index.write().await;
                index.remove(path);
                warn!(
                    path = %path.display(),
                    "Removed file from index (could not read)"
                );
            }
            Err(e) => {
                error!(
                    path = %path.display(),
                    error = %e,
                    "Failed to update file in index"
                );
            }
        }

        Ok(())
    }

    /// Remove a file from the index
    pub async fn remove_file(&self, path: &Path) {
        let mut index = self.index.write().await;
        if index.remove(path).is_some() {
            info!(
                path = %path.display(),
                "Removed file from index"
            );
        }
    }

    /// Search the index for matching content
    pub async fn search(&self, query: &str) -> Vec<IndexedFile> {
        let query_lower = query.to_lowercase();
        let index = self.index.read().await;
        let mut results = Vec::new();

        for file in index.values() {
            // Search in content
            if file.content.to_lowercase().contains(&query_lower) {
                results.push(file.clone());
            }
        }

        results
    }

    /// Get all indexed files
    pub async fn get_all_files(&self) -> Vec<IndexedFile> {
        let index = self.index.read().await;
        index.values().cloned().collect()
    }

    /// Get a specific file by path
    pub async fn get_file(&self, path: &Path) -> Option<IndexedFile> {
        let index = self.index.read().await;
        index.get(path).cloned()
    }

    /// Get search results with line numbers
    pub async fn search_with_lines(&self, query: &str) -> Vec<(IndexedFile, Vec<(u32, String)>)> {
        let query_lower = query.to_lowercase();
        let index = self.index.read().await;
        let mut results = Vec::new();

        for file in index.values() {
            let mut matching_lines = Vec::new();
            
            for (line_num, line) in file.lines.iter().enumerate() {
                if line.to_lowercase().contains(&query_lower) {
                    matching_lines.push((line_num as u32 + 1, line.clone()));
                }
            }

            if !matching_lines.is_empty() {
                results.push((file.clone(), matching_lines));
            }
        }

        results
    }
}

/// Background worker that watches for file changes and updates the index
pub struct PlaybookIndexerWorker {
    index: PlaybookIndex,
}

impl PlaybookIndexerWorker {
    /// Create a new indexer worker
    pub fn new(root_path: PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        let index = PlaybookIndex::new(root_path);
        Ok(Self { index })
    }

    /// Get a reference to the index
    pub fn index(&self) -> &PlaybookIndex {
        &self.index
    }

    /// Start the background worker
    pub async fn start(self) -> Result<(), Box<dyn std::error::Error>> {
        // Build initial index
        self.index.build_index().await?;

        // Spawn the file watcher task
        let index_clone = self.index.clone();
        let root_path = self.index.root_path.clone();
        
        tokio::spawn(async move {
            let (tx, mut rx) = tokio::sync::mpsc::channel::<Event>(100);
            
            // Create a new watcher in the spawned task
            let mut watcher = match notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    let _ = tx.try_send(event);
                }
            }) {
                Ok(w) => w,
                Err(e) => {
                    error!(error = %e, "Failed to create file watcher");
                    return;
                }
            };

            if let Err(e) = watcher.watch(&root_path, RecursiveMode::Recursive) {
                error!(error = %e, "Failed to start watching directory");
                return;
            }

            info!(
                path = %root_path.display(),
                "Playbook indexer file watcher started"
            );

            // Process file change events
            while let Some(event) = rx.recv().await {
                match event.kind {
                    EventKind::Create(_) | EventKind::Modify(_) => {
                        for path in event.paths {
                            if path.is_file() {
                                if let Err(e) = index_clone.update_file(&path).await {
                                    warn!(
                                        path = %path.display(),
                                        error = %e,
                                        "Failed to update file in index"
                                    );
                                }
                            }
                        }
                    }
                    EventKind::Remove(_) => {
                        for path in event.paths {
                            index_clone.remove_file(&path).await;
                        }
                    }
                    _ => {
                        // Ignore other event types
                    }
                }
            }
        });

        info!("Playbook indexer worker started");
        Ok(())
    }
}
