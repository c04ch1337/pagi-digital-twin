//! Script to ingest Ferrellgas Leadership Knowledge Base into Qdrant Memory Service
//!
//! This script reads structured leadership data and commits it to the `corporate_context` namespace
//! in the memory service (Qdrant) for RAG retrieval by The Blue Flame orchestrator.
//!
//! Usage:
//!   cargo run --example ingest_leadership_kb

use std::collections::HashMap;
use std::env;
use tonic::Request;
use tracing::{info, error};

// Include the generated proto code from memory service (same pattern as main.rs)
pub mod memory_client {
    tonic::include_proto!("memory");
}

use memory_client::memory_service_client::MemoryServiceClient;
use memory_client::{CommitMemoryRequest, CommitMemoryResponse};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    // Load environment variables
    dotenvy::dotenv().ok();

    // Get memory service gRPC endpoint
    let memory_grpc_url = env::var("MEMORY_GRPC_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:50052".to_string());

    info!(url = %memory_grpc_url, "Connecting to Memory Service");

    // Create gRPC client
    let mut client = MemoryServiceClient::connect(memory_grpc_url).await?;

    info!("Successfully connected to Memory Service");

    // Define the leadership KB chunks
    let chunks = vec![
        LeadershipChunk {
            title: "James E. Ferrell - Chairman",
            content: r#"James E. Ferrell serves as Chairman, Chief Executive Officer, and President of Ferrellgas. He has extensive tenure and leadership experience in the propane and energy distribution industry. Under his leadership, Ferrellgas has maintained its position as a leading provider of propane and related energy solutions. His strategic vision has guided the company through various market conditions, emphasizing operational excellence and customer service. Twenty-six years after the company's founding in 1939, James E. Ferrell started turning the small-town shop into one of the nation's leading propane suppliers. Through almost a century of innovation, Mr. Ferrell has instilled the same original values and work ethic, leading not just employees but over 4,000 employee-owners to support the high-powered, technology-focused logistics company Ferrellgas is known to be today."#,
            metadata: HashMap::from([
                ("source".to_string(), "leadership_kb".to_string()),
                ("priority".to_string(), "high".to_string()),
                ("role".to_string(), "Chairman, CEO, and President".to_string()),
                ("person".to_string(), "James E. Ferrell".to_string()),
            ]),
        },
        LeadershipChunk {
            title: "Tamria A. Zertuche - CEO and President",
            content: r#"Tamria A. Zertuche serves as Chief Executive Officer and President of Ferrellgas. She has a distinguished background in Information Technology, having previously served as Senior Director of IT and Chief Information Officer (CIO) before ascending to the CEO role. This unique career path demonstrates the company's recognition of IT's strategic value in operational excellence. Her IT background provides deep understanding of technological infrastructure needs, making her an ideal executive to escalate technical resource requests, system optimization proposals, and infrastructure investments. Under her leadership, Ferrellgas continues to blend technological innovation with operational excellence, reflecting her belief that IT is fundamental to business success."#,
            metadata: HashMap::from([
                ("source".to_string(), "leadership_kb".to_string()),
                ("priority".to_string(), "high".to_string()),
                ("role".to_string(), "CEO and President".to_string()),
                ("person".to_string(), "Tamria A. Zertuche".to_string()),
                ("it_background".to_string(), "true".to_string()),
                ("previous_roles".to_string(), "Senior Director of IT, CIO".to_string()),
            ]),
        },
        LeadershipChunk {
            title: "Board of Directors and People-First Philosophy",
            content: r#"Ferrellgas operates under a People-First philosophy that prioritizes the safety, well-being, and empowerment of all employees. This core value guides decision-making at all levels of the organization. The Board of Directors, led by Chairman James E. Ferrell, supports this philosophy and ensures that all strategic initiatives align with employee welfare and operational safety. The leadership structure emphasizes that technology and operational improvements should always serve the people who make the company successful. When proposing technical solutions or resource allocations, it is essential to frame them in terms of how they support employee safety, operational efficiency, and overall organizational health."#,
            metadata: HashMap::from([
                ("source".to_string(), "leadership_kb".to_string()),
                ("priority".to_string(), "high".to_string()),
                ("category".to_string(), "organizational_philosophy".to_string()),
                ("philosophy".to_string(), "People-First".to_string()),
            ]),
        },
        LeadershipChunk {
            title: "Family Values and Company History",
            content: r#"Founded in 1939, Ferrellgas's roots were planted in the Northeast Kansas town of Atchison when A.C. Ferrell and his wife Mabel opened the family-owned business A.C. Ferrell Butane Gas Company. Twenty-six years later, A.C.'s son James E. Ferrell, current Chairman, Chief Executive Officer, and President, started turning the small-town shop into one of the nation's leading propane suppliers. Through almost a century of innovation, Mr. Ferrell has instilled the same original values and work ethic, leading not just employees but over 4,000 employee-owners to support the high-powered, technology-focused logistics company Ferrellgas is known to be today. The company's foundation is built on family values, hard work, and a commitment to excellence that has been passed down through generations, creating a culture where employees are treated as family and owners."#,
            metadata: HashMap::from([
                ("source".to_string(), "leadership_kb".to_string()),
                ("priority".to_string(), "high".to_string()),
                ("category".to_string(), "company_history".to_string()),
                ("founded".to_string(), "1939".to_string()),
                ("founders".to_string(), "A.C. Ferrell and Mabel Ferrell".to_string()),
                ("location".to_string(), "Atchison, Kansas".to_string()),
                ("employee_owners".to_string(), "4000+".to_string()),
                ("values".to_string(), "family values, work ethic, innovation".to_string()),
            ]),
        },
    ];

    // Ingest each chunk into the corporate_context namespace
    let namespace = "corporate_context";
    let twin_id = "twin-aegis"; // The Blue Flame orchestrator
    let memory_type = "RAGSource";
    let risk_level = "Low";

    info!(
        namespace = %namespace,
        chunks_count = chunks.len(),
        "Starting leadership KB ingestion"
    );

    let mut success_count = 0;
    let mut error_count = 0;

    for (index, chunk) in chunks.iter().enumerate() {
        info!(
            chunk_index = index + 1,
            total_chunks = chunks.len(),
            title = %chunk.title,
            "Ingesting chunk"
        );

        let request = Request::new(CommitMemoryRequest {
            content: chunk.content.to_string(),
            namespace: namespace.to_string(),
            twin_id: twin_id.to_string(),
            memory_type: memory_type.to_string(),
            risk_level: risk_level.to_string(),
            metadata: chunk.metadata.clone(),
        });

        match client.commit_memory(request).await {
            Ok(response) => {
                let resp: CommitMemoryResponse = response.into_inner();
                if resp.success {
                    info!(
                        memory_id = %resp.memory_id,
                        chunk_index = index + 1,
                        "Chunk ingested successfully"
                    );
                    success_count += 1;
                } else {
                    error!(
                        error_message = %resp.error_message,
                        chunk_index = index + 1,
                        "Chunk ingestion failed"
                    );
                    error_count += 1;
                }
            }
            Err(e) => {
                error!(
                    error = %e,
                    chunk_index = index + 1,
                    "gRPC call failed"
                );
                error_count += 1;
            }
        }
    }

    info!(
        success_count = success_count,
        error_count = error_count,
        total_chunks = chunks.len(),
        namespace = %namespace,
        "Leadership KB ingestion completed"
    );

    if error_count > 0 {
        Err(format!("Failed to ingest {} chunks", error_count).into())
    } else {
        Ok(())
    }
}

struct LeadershipChunk {
    title: &'static str,
    content: &'static str,
    metadata: HashMap<String, String>,
}
