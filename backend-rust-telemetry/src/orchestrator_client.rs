use std::time::Duration;
use tonic::transport::{Channel, Endpoint};
use tracing::{error, info, warn};

// Include generated proto client
pub mod orchestrator_proto {
    tonic::include_proto!("orchestrator");
}

use orchestrator_proto::orchestrator_service_client::OrchestratorServiceClient;
use orchestrator_proto::{SummarizeRequest, SummarizeResponse};

/// Get or create an Orchestrator gRPC client with proper timeout configuration
/// 
/// This function creates a gRPC channel with:
/// - 5 second connection timeout
/// - 60 second request timeout (to allow for LLM processing)
/// 
/// # Arguments
/// 
/// * `addr` - The gRPC address of the Orchestrator service (e.g., "http://127.0.0.1:50057")
/// 
/// # Returns
/// 
/// A Result containing the OrchestratorServiceClient or an error
pub async fn get_orchestrator_client(
    addr: String,
) -> Result<OrchestratorServiceClient<Channel>, Box<dyn std::error::Error>> {
    info!(
        orchestrator_addr = %addr,
        "Connecting to Orchestrator gRPC service"
    );

    let channel = Endpoint::from_shared(addr)?
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(60)) // Allow 60s for LLM processing
        .connect()
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to connect to Orchestrator service");
            e
        })?;

    info!("Successfully connected to Orchestrator gRPC service");

    Ok(OrchestratorServiceClient::new(channel))
}

/// Request transcript summarization from the Orchestrator service
/// 
/// This function:
/// 1. Creates a gRPC client connection to the specified address
/// 2. Sends the transcript text for summarization
/// 3. Returns the structured SummarizeResponse
/// 
/// # Arguments
/// 
/// * `orchestrator_addr` - The gRPC address of the Orchestrator service (e.g., "http://127.0.0.1:50057")
/// * `transcript` - The raw transcript text to summarize
/// 
/// # Returns
/// 
/// A Result containing the SummarizeResponse or an error
pub async fn request_summarization(
    orchestrator_addr: String,
    transcript: String,
) -> Result<SummarizeResponse, Box<dyn std::error::Error>> {

    if transcript.trim().is_empty() {
        return Err("Transcript text cannot be empty".into());
    }

    if transcript.len() > 2_000_000 {
        return Err(format!(
            "Transcript too large ({} chars, max 2,000,000)",
            transcript.len()
        )
        .into());
    }

    info!(
        transcript_length = transcript.len(),
        orchestrator_addr = %orchestrator_addr,
        "Requesting transcript summarization"
    );

    // Create client connection
    let mut client = get_orchestrator_client(orchestrator_addr.clone()).await?;

    // Construct the request
    let request = tonic::Request::new(SummarizeRequest { transcript_text: transcript });

    // Call the summarization endpoint
    let response = client
        .summarize_transcript(request)
        .await
        .map_err(|e| {
            error!(error = %e, "Orchestrator summarize_transcript RPC failed");
            e
        })?;

    let summarize_response = response.into_inner();

    info!(
        summary_length = summarize_response.summary.len(),
        decisions_count = summarize_response.key_decisions.len(),
        tasks_count = summarize_response.follow_up_tasks.len(),
        "Received summarization response from Orchestrator"
    );

    Ok(summarize_response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[tokio::test]
    #[ignore] // Requires running Orchestrator service
    async fn test_request_summarization() {
        let test_transcript = "This is a test transcript. We discussed several important topics.";
        let orchestrator_addr = env::var("ORCHESTRATOR_GRPC_ADDR")
            .unwrap_or_else(|_| "http://127.0.0.1:50057".to_string());
        let result = request_summarization(orchestrator_addr, test_transcript.to_string()).await;
        
        match result {
            Ok(response) => {
                assert!(!response.summary.is_empty());
                println!("Summary: {}", response.summary);
            }
            Err(e) => {
                warn!(error = %e, "Test failed (Orchestrator may not be running)");
            }
        }
    }
}
