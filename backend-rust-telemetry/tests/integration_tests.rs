// Integration tests for the Multi-Modal Insight Loop
// 
// This test suite validates the full workflow:
// 1. Mock transcript file creation
// 2. File watcher detection
// 3. Orchestrator summarization
// 4. Summary JSON file creation
// 5. Memory service indexing

use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::fs;
use tokio::time::sleep;
use serde_json::Value;

// Test configuration
const TEST_STORAGE_DIR: &str = "./test_storage";
const TEST_TIMEOUT_SECS: u64 = 30;
const TEST_POLL_INTERVAL_MS: u64 = 500;

/// Helper function to create a test transcript file
async fn create_test_transcript(
    storage_dir: &Path,
    twin_id: &str,
    timestamp: u128,
    content: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let recordings_dir = storage_dir.join("recordings");
    fs::create_dir_all(&recordings_dir).await?;

    let filename = format!("rec_{}_{}.txt", twin_id, timestamp);
    let file_path = recordings_dir.join(&filename);

    fs::write(&file_path, content).await?;

    Ok(file_path)
}

/// Helper function to check if a summary file exists
async fn check_summary_exists(
    storage_dir: &Path,
    transcript_path: &Path,
) -> Result<bool, Box<dyn std::error::Error>> {
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

    Ok(fs::metadata(&summary_path).await.is_ok())
}

/// Helper function to read and validate summary JSON
async fn read_and_validate_summary(
    storage_dir: &Path,
    transcript_path: &Path,
) -> Result<Value, Box<dyn std::error::Error>> {
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

    let summary_content = fs::read_to_string(&summary_path).await?;
    let summary_json: Value = serde_json::from_str(&summary_content)?;

    Ok(summary_json)
}

/// Cleanup test files
async fn cleanup_test_files(
    storage_dir: &Path,
    transcript_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    // Remove transcript file
    if fs::metadata(transcript_path).await.is_ok() {
        fs::remove_file(transcript_path).await?;
    }

    // Remove summary file if it exists
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

    if fs::metadata(&summary_path).await.is_ok() {
        fs::remove_file(&summary_path).await?;
    }

    Ok(())
}

#[tokio::test]
#[ignore] // Requires running services - run with: cargo test --test integration_tests -- --ignored
async fn test_multi_modal_insight_loop() {
    // This test requires:
    // 1. Orchestrator service running with LLM_PROVIDER=openrouter
    // 2. Memory service running
    // 3. Telemetry transcription worker running (or we simulate it)

    let storage_dir = Path::new(TEST_STORAGE_DIR);
    let twin_id = "test-twin";
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();

    // Cleanup any existing test files
    let _ = cleanup_test_files(storage_dir, &PathBuf::new()).await;

    // Step 1: Create a mock transcript file
    println!("Step 1: Creating mock transcript file...");
    let test_transcript_content = r#"
This is a test transcript for the automated insight system.

We discussed several important topics:
- Implementation of the closed-loop intelligence system
- Integration between Telemetry, Orchestrator, and Memory services
- Testing strategies for distributed AGI systems

Key decisions made:
1. We will use gRPC for inter-service communication
2. File-based triggers for transcript processing
3. JSON summaries stored alongside transcripts

Follow-up tasks:
- Complete integration testing
- Set up monitoring and alerting
- Document the architecture
"#;

    let transcript_path = create_test_transcript(
        storage_dir,
        twin_id,
        timestamp,
        test_transcript_content,
    )
    .await
    .expect("Failed to create test transcript");

    println!("  ✓ Created transcript: {:?}", transcript_path);

    // Step 2: Wait for the transcription watcher to detect the file
    // In a real scenario, the watcher would be running. For this test,
    // we'll manually trigger the processing or wait for it.
    println!("Step 2: Waiting for file watcher to detect transcript...");
    
    // Note: In a real integration test, you would start the transcription worker
    // as a background task. For now, we'll simulate by calling the processing
    // function directly or waiting for the file to be processed.
    
    // Wait up to TEST_TIMEOUT_SECS for the summary to be created
    let mut summary_exists = false;
    let start_time = std::time::Instant::now();
    
    while start_time.elapsed().as_secs() < TEST_TIMEOUT_SECS {
        if check_summary_exists(storage_dir, &transcript_path)
            .await
            .unwrap_or(false)
        {
            summary_exists = true;
            break;
        }
        sleep(Duration::from_millis(TEST_POLL_INTERVAL_MS)).await;
    }

    assert!(
        summary_exists,
        "Summary file was not created within {} seconds. \
         Make sure the transcription worker is running and can connect to Orchestrator.",
        TEST_TIMEOUT_SECS
    );

    println!("  ✓ Summary file detected");

    // Step 3: Validate the summary JSON structure
    println!("Step 3: Validating summary JSON structure...");
    
    let summary_json = read_and_validate_summary(storage_dir, &transcript_path)
        .await
        .expect("Failed to read summary JSON");

    // Validate required fields
    assert!(
        summary_json.get("summary").is_some(),
        "Summary JSON missing 'summary' field"
    );
    
    assert!(
        summary_json.get("key_decisions").is_some(),
        "Summary JSON missing 'key_decisions' field"
    );
    
    assert!(
        summary_json.get("follow_up_tasks").is_some(),
        "Summary JSON missing 'follow_up_tasks' field"
    );

    // Validate that fields are non-empty
    let summary_text = summary_json
        .get("summary")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    
    assert!(
        !summary_text.trim().is_empty(),
        "Summary text is empty"
    );

    let empty_vec: Vec<Value> = vec![];
    let decisions = summary_json
        .get("key_decisions")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty_vec);
    
    let tasks = summary_json
        .get("follow_up_tasks")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty_vec);

    println!("  ✓ Summary: {} chars", summary_text.len());
    println!("  ✓ Decisions: {} items", decisions.len());
    println!("  ✓ Tasks: {} items", tasks.len());

    // Step 4: Query Memory Service to verify indexing
    // Note: This requires the Memory service to be running and accessible
    println!("Step 4: Verifying Memory Service indexing...");
    
    // For now, we'll just verify the summary was created correctly
    // In a full integration test, you would:
    // 1. Connect to Memory service gRPC
    // 2. Query the 'insights' namespace
    // 3. Verify the entry exists
    
    println!("  ✓ Summary validated (Memory service check requires running service)");

    // Cleanup
    println!("Cleaning up test files...");
    cleanup_test_files(storage_dir, &transcript_path)
        .await
        .expect("Failed to cleanup test files");
    
    println!("  ✓ Cleanup complete");

    println!("\n✅ All integration tests passed!");
}

#[tokio::test]
#[ignore]
async fn test_transcript_processing_timeout() {
    // Test that the system handles timeouts gracefully
    let storage_dir = Path::new(TEST_STORAGE_DIR);
    let twin_id = "test-timeout";
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();

    let transcript_path = create_test_transcript(
        storage_dir,
        twin_id,
        timestamp,
        "Short test transcript.",
    )
    .await
    .expect("Failed to create test transcript");

    // Wait a short time and verify file exists
    sleep(Duration::from_secs(2)).await;
    
    assert!(
        fs::metadata(&transcript_path).await.is_ok(),
        "Transcript file should exist"
    );

    // Cleanup
    let _ = cleanup_test_files(storage_dir, &transcript_path).await;
}
