use std::path::{Path, PathBuf};
use tokio::fs;
use tonic::transport::Channel;
use tracing::{error, info, warn};

// Include generated proto clients
pub mod orchestrator_client {
    tonic::include_proto!("orchestrator");
}

pub mod memory_client {
    tonic::include_proto!("memory");
}

use orchestrator_client::orchestrator_service_client::OrchestratorServiceClient;
use orchestrator_client::{SummarizeRequest, SummarizeResponse};
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
pub async fn process_transcript(
    transcript_path: &Path,
    twin_id: &str,
    timestamp: u128,
    orchestrator_client: &mut OrchestratorServiceClient<Channel>,
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

    // Call orchestrator's summarize_transcript endpoint
    let summarize_request = tonic::Request::new(SummarizeRequest {
        transcript_text: transcript_text.clone(),
    });

    let summarize_response: SummarizeResponse = orchestrator_client
        .summarize_transcript(summarize_request)
        .await
        .map_err(|e| format!("Orchestrator summarize_transcript failed: {}", e))?
        .into_inner();

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

    // Save summary JSON to disk: rec_{twin_id}_{timestamp}.summary.json
    let summary_filename = format!("rec_{twin_id}_{timestamp}.summary.json");
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

/// Transcription worker that watches for transcript files and processes them
pub async fn transcription_worker(
    storage_dir: PathBuf,
    orchestrator_addr: String,
    memory_addr: String,
) -> Result<(), Box<dyn std::error::Error>> {
    info!(
        orchestrator_addr = %orchestrator_addr,
        memory_addr = %memory_addr,
        "Starting transcription worker"
    );

    // Connect to gRPC services
    let mut orchestrator_client = OrchestratorServiceClient::connect(orchestrator_addr.clone())
        .await
        .map_err(|e| format!("Failed to connect to Orchestrator service: {}", e))?;

    let mut memory_client = MemoryServiceClient::connect(memory_addr.clone())
        .await
        .map_err(|e| format!("Failed to connect to Memory service: {}", e))?;

    info!("Connected to Orchestrator and Memory gRPC services");

    let recordings_dir = storage_dir.join("recordings");
    
    // Ensure recordings directory exists
    fs::create_dir_all(&recordings_dir).await?;

    // Watch for new .txt files in the recordings directory
    // This is a simple polling approach; in production you might use a file watcher library
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(2));
    let mut processed_files: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();

    loop {
        interval.tick().await;

        // Scan for .txt files
        let mut entries = match fs::read_dir(&recordings_dir).await {
            Ok(entries) => entries,
            Err(e) => {
                warn!(error = %e, "Failed to read recordings directory");
                continue;
            }
        };

        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            
            // Only process .txt files that haven't been processed yet
            if path.extension().and_then(|s| s.to_str()) == Some("txt") 
                && !processed_files.contains(&path) 
            {
                // Extract twin_id and timestamp from filename: rec_{twin_id}_{timestamp}.txt
                let filename = path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");

                if let Some((twin_id, timestamp_str)) = filename
                    .strip_prefix("rec_")
                    .and_then(|s| s.strip_suffix(".txt"))
                    .and_then(|s| s.rsplit_once('_'))
                {
                    if let Ok(timestamp) = timestamp_str.parse::<u128>() {
                        let storage_dir_clone = storage_dir.clone();
                        let path_clone = path.clone();
                        let twin_id = twin_id.to_string();
                        let orchestrator_addr_clone = orchestrator_addr.clone();
                        let memory_addr_clone = memory_addr.clone();

                        // Process in background to avoid blocking the watcher
                        tokio::spawn(async move {
                            // Create new connections for this task
                            let mut orchestrator_client = match OrchestratorServiceClient::connect(orchestrator_addr_clone).await {
                                Ok(client) => client,
                                Err(e) => {
                                    error!(error = %e, "Failed to connect to Orchestrator service for transcript processing");
                                    return;
                                }
                            };

                            let mut memory_client = match MemoryServiceClient::connect(memory_addr_clone).await {
                                Ok(client) => client,
                                Err(e) => {
                                    error!(error = %e, "Failed to connect to Memory service for transcript processing");
                                    return;
                                }
                            };

                            match process_transcript(
                                &path_clone,
                                &twin_id,
                                timestamp,
                                &mut orchestrator_client,
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

                        processed_files.insert(path);
                    } else {
                        warn!(
                            filename = %filename,
                            "Could not parse timestamp from transcript filename"
                        );
                    }
                } else {
                    warn!(
                        filename = %filename,
                        "Transcript filename does not match expected pattern rec_<twin_id>_<timestamp>.txt"
                    );
                }
            }
        }
    }
}
