use std::path::{Path, PathBuf};
use tokio::fs;
use tonic::transport::Channel;
use tracing::{error, info, warn};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use notify::event::{AccessKind, AccessMode};
use std::sync::Arc;
use tokio::sync::mpsc;

// Import orchestrator client module
use crate::orchestrator_client::request_summarization;

// Include generated proto clients for memory service
pub mod memory_client {
    tonic::include_proto!("memory");
}

use memory_client::memory_service_client::MemoryServiceClient;
use memory_client::{CommitMemoryRequest, CommitMemoryResponse};
use serde::Serialize;

#[derive(Serialize)]
struct SummaryJson {
    summary: String,
    key_decisions: Vec<String>,
    follow_up_tasks: Vec<String>,
}

/// Process a transcript file: summarize it, save summary JSON, and commit to memory
async fn process_transcript(
    transcript_path: &Path,
    twin_id: &str,
    timestamp: u128,
    orchestrator_addr: &str,
    memory_client: &mut MemoryServiceClient<Channel>,
    storage_dir: &Path,
) -> Result<(), String> {
    info!(
        transcript_path = %transcript_path.display(),
        twin_id = %twin_id,
        "Processing transcript for summarization"
    );

    // Read the transcript text
    let transcript_text = fs::read_to_string(transcript_path)
        .await
        .map_err(|e| format!("Failed to read transcript file {}: {}", transcript_path.display(), e))?;

    if transcript_text.trim().is_empty() {
        return Err("Transcript file is empty".to_string());
    }

    // Call orchestrator's summarize_transcript endpoint using the client module
    let summarize_response = request_summarization(orchestrator_addr.to_string(), transcript_text)
        .await
        .map_err(|e| format!("Orchestrator summarize_transcript failed: {}", e))?;

    info!(
        twin_id = %twin_id,
        summary_length = summarize_response.summary.len(),
        decisions_count = summarize_response.key_decisions.len(),
        tasks_count = summarize_response.follow_up_tasks.len(),
        "Received summary from orchestrator"
    );

    // Convert SummarizeResponse to serializable struct
    let summary_json_struct = SummaryJson {
        summary: summarize_response.summary.clone(),
        key_decisions: summarize_response.key_decisions.clone(),
        follow_up_tasks: summarize_response.follow_up_tasks.clone(),
    };

    // Serialize to JSON
    let summary_json = serde_json::to_string_pretty(&summary_json_struct)
        .map_err(|e| format!("Failed to serialize summary to JSON: {}", e))?;

    // Save summary JSON to disk: [filename].summary.json
    let transcript_filename = transcript_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown.txt");
    let base_name = transcript_filename
        .rsplit_once('.')
        .map(|(base, _)| base)
        .unwrap_or(transcript_filename);
    let summary_filename = format!("{}.summary.json", base_name);
    let recordings_dir = storage_dir.join("recordings");
    let summary_path = recordings_dir.join(&summary_filename);

    fs::write(&summary_path, &summary_json)
        .await
        .map_err(|e| format!("Failed to write summary JSON to {}: {}", summary_path.display(), e))?;

    info!(
        summary_path = %summary_path.display(),
        "Saved summary JSON to disk"
    );

    // Format content for Neural Archive commit
    let mut memory_content = format!("Summary: {}\n\n", summarize_response.summary);

    if !summarize_response.key_decisions.is_empty() {
        memory_content.push_str("Key Decisions:\n");
        for (i, decision) in summarize_response.key_decisions.iter().enumerate() {
            memory_content.push_str(&format!("{}. {}\n", i + 1, decision));
        }
        memory_content.push_str("\n");
    }

    if !summarize_response.follow_up_tasks.is_empty() {
        memory_content.push_str("Follow-up Tasks:\n");
        for (i, task) in summarize_response.follow_up_tasks.iter().enumerate() {
            memory_content.push_str(&format!("{}. {}\n", i + 1, task));
        }
    }

    // Commit to Memory Service using "insights" namespace
    let commit_request = tonic::Request::new(CommitMemoryRequest {
        content: memory_content,
        namespace: "insights".to_string(),
        twin_id: twin_id.to_string(),
        memory_type: "RAGSource".to_string(),
        risk_level: "Low".to_string(),
        metadata: std::collections::HashMap::new(),
    });

    let commit_response: CommitMemoryResponse = memory_client
        .commit_memory(commit_request)
        .await
        .map_err(|e| format!("Memory service commit_memory failed: {}", e))?
        .into_inner();

    if !commit_response.success {
        return Err(format!(
            "Memory commit failed: {}",
            commit_response.error_message
        ));
    }

    info!(
        memory_id = %commit_response.memory_id,
        twin_id = %twin_id,
        namespace = "insights",
        "Committed summary to Neural Archive"
    );

    Ok(())
}

/// File-watcher based transcription worker that watches for transcript files and processes them
pub async fn start_transcription_watcher(
    storage_dir: PathBuf,
    orchestrator_addr: String,
    memory_addr: String,
) -> Result<(), Box<dyn std::error::Error>> {
    info!(
        orchestrator_addr = %orchestrator_addr,
        memory_addr = %memory_addr,
        storage_dir = %storage_dir.display(),
        "Starting file-watcher transcription worker"
    );

    let recordings_dir = storage_dir.join("recordings");
    
    // Ensure recordings directory exists
    fs::create_dir_all(&recordings_dir).await?;

    // Create a channel to receive file events from the watcher
    let (tx, mut rx) = mpsc::unbounded_channel::<Result<Event, notify::Error>>();

    // Create the file watcher with a callback that sends events to the channel
    let tx_clone = tx.clone();
    let mut watcher = RecommendedWatcher::new(
        move |result: Result<Event, notify::Error>| {
            if tx_clone.send(result).is_err() {
                // Channel closed, watcher will stop
            }
        },
        Config::default(),
    )
    .map_err(|e| format!("Failed to create file watcher: {}", e))?;

    // Watch the recordings directory recursively
    watcher
        .watch(&recordings_dir, RecursiveMode::NonRecursive)
        .map_err(|e| format!("Failed to watch directory {}: {}", recordings_dir.display(), e))?;

    info!(
        directory = %recordings_dir.display(),
        "File watcher started, monitoring for new transcript files"
    );

    // Track processed files to avoid duplicate processing
    let mut processed_files: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();

    // Process existing .txt files that don't have corresponding .summary.json files (replay)
    info!("Scanning for existing unprocessed transcripts...");
    let mut entries = fs::read_dir(&recordings_dir).await?;
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("txt") {
            let base_name = path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            
            // Check if summary already exists
            let summary_path = recordings_dir.join(format!("{}.summary.json", base_name));
            if fs::metadata(&summary_path).await.is_ok() {
                info!(
                    transcript = %path.display(),
                    "Skipping already processed transcript"
                );
                continue;
            }

            // Extract twin_id and timestamp from filename: rec_{twin_id}_{timestamp}.txt
            if let Some((twin_id, timestamp_str)) = base_name
                .strip_prefix("rec_")
                .and_then(|s| s.rsplit_once('_'))
            {
                if let Ok(timestamp) = timestamp_str.parse::<u128>() {
                    let storage_dir_clone = storage_dir.clone();
                    let path_clone = path.clone();
                    let orchestrator_addr_clone = orchestrator_addr.clone();
                    let memory_addr_clone = memory_addr.clone();

                    // Process in background
                    tokio::spawn(async move {
                        // Create memory service connection for this task
                        let mut memory_client = match MemoryServiceClient::connect(memory_addr_clone).await {
                            Ok(client) => client,
                            Err(e) => {
                                error!(error = %e, "Failed to connect to Memory service for transcript processing");
                                return;
                            }
                        };

                        match process_transcript(
                            &path_clone,
                            twin_id,
                            timestamp,
                            &orchestrator_addr_clone,
                            &mut memory_client,
                            &storage_dir_clone,
                        )
                        .await
                        {
                            Ok(_) => {
                                info!(
                                    transcript_path = %path_clone.display(),
                                    "Successfully processed transcript (replay)"
                                );
                            }
                            Err(e) => {
                                error!(
                                    transcript_path = %path_clone.display(),
                                    error = %e,
                                    "Failed to process transcript (replay)"
                                );
                            }
                        }
                    });

                    processed_files.insert(path);
                }
            }
        }
    }

    info!("Finished replay scan, now watching for new files...");

    // Main event loop: process new transcript files as they are created
    loop {
        tokio::select! {
            Some(event_result) = rx.recv() => {
                let event = match event_result {
                    Ok(event) => event,
                    Err(e) => {
                        warn!(error = %e, "File watcher error");
                        continue;
                    }
                };
                
                // Filter: We only care about files that have finished being written/closed
                if let EventKind::Access(AccessKind::Close(AccessMode::Write)) = event.kind {
                    for path in event.paths {
                        // Only process .txt files
                        if path.extension().and_then(|s| s.to_str()) == Some("txt") {
                            // Logic: Trigger gRPC Summarization Call here
                            println!("New transcript detected: {:?}", path);
                            
                            // Skip if already processed
                            if processed_files.contains(&path) {
                                continue;
                            }

                            // Check if file is fully written (not a temporary file)
                            if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                                // Skip temporary files
                                if file_name.starts_with('.') || file_name.ends_with(".tmp") {
                                    continue;
                                }

                                // Extract twin_id and timestamp from filename: rec_{twin_id}_{timestamp}.txt
                                let base_name = path.file_stem()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or("");

                                if let Some((twin_id, timestamp_str)) = base_name
                                    .strip_prefix("rec_")
                                    .and_then(|s| s.rsplit_once('_'))
                                {
                                    if let Ok(timestamp) = timestamp_str.parse::<u128>() {
                                        // Mark as processed to avoid duplicate processing
                                        processed_files.insert(path.clone());

                                        let storage_dir_clone = storage_dir.clone();
                                        let path_clone = path.clone();
                                        let orchestrator_addr_clone = orchestrator_addr.clone();
                                        let memory_addr_clone = memory_addr.clone();

                                        // Process in background
                                        tokio::spawn(async move {
                                            // Create memory service connection for this task
                                            let mut memory_client = match MemoryServiceClient::connect(memory_addr_clone).await {
                                                Ok(client) => client,
                                                Err(e) => {
                                                    error!(error = %e, "Failed to connect to Memory service for transcript processing");
                                                    return;
                                                }
                                            };

                                            match process_transcript(
                                                &path_clone,
                                                twin_id,
                                                timestamp,
                                                &orchestrator_addr_clone,
                                                &mut memory_client,
                                                &storage_dir_clone,
                                            )
                                            .await
                                            {
                                                Ok(_) => {
                                                    info!(
                                                        transcript_path = %path_clone.display(),
                                                        "Successfully processed transcript"
                                                    );
                                                }
                                                Err(e) => {
                                                    error!(
                                                        transcript_path = %path_clone.display(),
                                                        error = %e,
                                                        "Failed to process transcript"
                                                    );
                                                }
                                            }
                                        });
                                    } else {
                                        warn!(
                                            filename = %base_name,
                                            "Could not parse timestamp from transcript filename"
                                        );
                                    }
                                } else {
                                    warn!(
                                        filename = %base_name,
                                        "Transcript filename does not match expected pattern rec_<twin_id>_<timestamp>.txt"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
