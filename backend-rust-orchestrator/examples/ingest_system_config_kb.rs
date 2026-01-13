//! Script to ingest System Configuration Knowledge Base into Qdrant Memory Service
//!
//! This script reads structured system configuration documentation and commits it to the
//! `system_config` namespace in the memory service (Qdrant) for RAG retrieval by The Blue Flame orchestrator.
//!
//! Usage:
//!   cargo run --example ingest_system_config_kb

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

    // Define the system configuration KB chunks
    let chunks = vec![
        SystemConfigChunk {
            title: "PAGI Digital Twin Platform Architecture",
            content: r#"The PAGI Digital Twin platform consists of multiple microservices: Gateway (port 8181) - HTTP gateway and reverse proxy, Orchestrator (port 8182) - The Blue Flame AI orchestrator for decision-making, Memory Service (gRPC 50052) - Qdrant vector database for knowledge storage, Tools Service (gRPC 50054) - Secure sandbox for tool execution, Telemetry Service (port 8281) - System monitoring and media recording, Build Service - Compiles Rust code into executable tools. Services communicate via gRPC (internal) and HTTP/REST (external). The frontend connects to the Gateway which proxies requests to appropriate services. All services support cross-platform operation (Linux, Windows, macOS). Configuration is managed via environment variables and .env files."#,
            metadata: HashMap::from([
                ("source".to_string(), "system_config_kb".to_string()),
                ("priority".to_string(), "high".to_string()),
                ("category".to_string(), "architecture".to_string()),
                ("component".to_string(), "platform".to_string()),
            ]),
        },
        SystemConfigChunk {
            title: "Service Port Configuration",
            content: r#"Default service ports: Gateway HTTP (8181), Orchestrator HTTP (8182), Telemetry Service (8281-8284 for SSE streams), Memory Service gRPC (50052), Tools Service gRPC (50054), Qdrant (6334 HTTP, 6335 gRPC). Ports can be overridden via environment variables: GATEWAY_HTTP_PORT, ORCHESTRATOR_HTTP_PORT, TELEMETRY_HTTP_PORT, MEMORY_GRPC_ADDR, TOOLS_GRPC_ADDR, QDRANT_URL. The frontend uses VITE_GATEWAY_URL (default: http://127.0.0.1:8181) to connect to the Gateway. All services should be accessible on localhost for development, with proper firewall rules for production deployments."#,
            metadata: HashMap::from([
                ("source".to_string(), "system_config_kb".to_string()),
                ("priority".to_string(), "high".to_string()),
                ("category".to_string(), "networking".to_string()),
                ("component".to_string(), "ports".to_string()),
            ]),
        },
        SystemConfigChunk {
            title: "Memory Service Configuration",
            content: r#"Memory Service supports Qdrant backend (default) or in-memory fallback. Configuration: MEMORY_BACKEND (qdrant or in_memory), QDRANT_URL (default: http://127.0.0.1:6334), QDRANT_REQUIRED (true/false - if false, allows in-memory fallback when Qdrant unavailable), MEMORY_GRPC_URL (default: http://127.0.0.1:50052). Namespaces (collections) are created automatically on first use. Supported memory types: Episodic (conversations/events), Semantic (facts/concepts), RAGSource (documents/KBs), Reflection (analysis outputs). Risk levels: Low, Medium, High, Critical. The service provides gRPC endpoints for CommitMemory and QueryMemory operations."#,
            metadata: HashMap::from([
                ("source".to_string(), "system_config_kb".to_string()),
                ("priority".to_string(), "high".to_string()),
                ("category".to_string(), "service_config".to_string()),
                ("component".to_string(), "memory".to_string()),
            ]),
        },
        SystemConfigChunk {
            title: "Orchestrator LLM Configuration",
            content: r#"The Blue Flame orchestrator uses OpenRouter API for LLM planning. Required configuration: OPENROUTER_API_KEY (required), OPENROUTER_MODEL (default: google/gemini-2.0-flash-exp), OPENROUTER_URL (default: https://openrouter.ai/api/v1/chat/completions), LLM_PROVIDER (openrouter or mock). The system prompt is loaded from config/system_prompt.txt and can be updated via the self-improvement action or admin API. The orchestrator makes structured JSON decisions matching the LLMAction enum (ActionMemory, ActionTool, ActionResponse, etc.). All tool executions require human-in-the-loop (HITL) approval via the UI before execution."#,
            metadata: HashMap::from([
                ("source".to_string(), "system_config_kb".to_string()),
                ("priority".to_string(), "high".to_string()),
                ("category".to_string(), "service_config".to_string()),
                ("component".to_string(), "orchestrator".to_string()),
            ]),
        },
        SystemConfigChunk {
            title: "Tools Service Sandbox Configuration",
            content: r#"Tools Service executes tools in a secure sandbox environment. Configuration: TOOLS_GRPC_ADDR (default: http://127.0.0.1:50054), SANDBOX_NAMESPACE (isolated execution environment). Policy configuration controls which twins can execute which commands: twin-aegis (The Blue Flame) has unlimited access, twin-sentinel has restricted access to file operations and analysis tools. Tools are compiled Rust binaries executed with restricted permissions. The service supports tool_exec gRPC calls with tool name and arguments. All tool executions are logged for audit purposes. Cross-platform support ensures tools work on Linux, Windows, and macOS."#,
            metadata: HashMap::from([
                ("source".to_string(), "system_config_kb".to_string()),
                ("priority".to_string(), "high".to_string()),
                ("category".to_string(), "service_config".to_string()),
                ("component".to_string(), "tools".to_string()),
            ]),
        },
        SystemConfigChunk {
            title: "Telemetry Service Configuration",
            content: r#"Telemetry Service provides system monitoring and media recording. Configuration: TELEMETRY_HTTP_PORT (default: 8281), Storage paths: storage/assets/ (custom branding), storage/recordings/ (media files). SSE streams available on ports 8281-8284 for real-time metrics. Media recording supports voice, video, and screen capture with automatic transcription and AI-powered summarization. Custom assets (logos, favicons) can be uploaded via /api/assets/upload. The service provides endpoints for system snapshots, sync metrics (neural sync percentage), and media gallery management. All recordings are automatically processed for transcripts and summaries."#,
            metadata: HashMap::from([
                ("source".to_string(), "system_config_kb".to_string()),
                ("priority".to_string(), "high".to_string()),
                ("category".to_string(), "service_config".to_string()),
                ("component".to_string(), "telemetry".to_string()),
            ]),
        },
        SystemConfigChunk {
            title: "Environment Variable Reference",
            content: r#"Key environment variables: GATEWAY_HTTP_PORT (8181), ORCHESTRATOR_HTTP_PORT (8182), MEMORY_GRPC_ADDR (http://127.0.0.1:50052), TOOLS_GRPC_ADDR (http://127.0.0.1:50054), TELEMETRY_HTTP_PORT (8281), QDRANT_URL (http://127.0.0.1:6334), MEMORY_BACKEND (qdrant), QDRANT_REQUIRED (true/false), OPENROUTER_API_KEY (required), OPENROUTER_MODEL (google/gemini-2.0-flash-exp), OPENROUTER_URL, LLM_PROVIDER (openrouter). Frontend: VITE_GATEWAY_URL (http://127.0.0.1:8181), VITE_ORCHESTRATOR_URL. All services support .env file loading via dotenvy. Production deployments should use secure secret management instead of .env files."#,
            metadata: HashMap::from([
                ("source".to_string(), "system_config_kb".to_string()),
                ("priority".to_string(), "high".to_string()),
                ("category".to_string(), "reference".to_string()),
                ("component".to_string(), "environment".to_string()),
            ]),
        },
    ];

    // Ingest each chunk into the system_config namespace
    let namespace = "system_config";
    let twin_id = "twin-aegis"; // The Blue Flame orchestrator
    let memory_type = "RAGSource";
    let risk_level = "Low";

    info!(
        namespace = %namespace,
        chunks_count = chunks.len(),
        "Starting system configuration KB ingestion"
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
        "System configuration KB ingestion completed"
    );

    if error_count > 0 {
        Err(format!("Failed to ingest {} chunks", error_count).into())
    } else {
        Ok(())
    }
}

struct SystemConfigChunk {
    title: &'static str,
    content: &'static str,
    metadata: HashMap<String, String>,
}
