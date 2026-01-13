//! Script to ingest Threat Intelligence Knowledge Base into Qdrant Memory Service
//!
//! This script reads structured threat intelligence data and commits it to the `threat_intel` namespace
//! in the memory service (Qdrant) for RAG retrieval by The Blue Flame orchestrator.
//!
//! Usage:
//!   cargo run --example ingest_threat_intel_kb

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

    // Define the threat intelligence KB chunks
    let chunks = vec![
        ThreatIntelChunk {
            title: "C2 Beaconing Indicators",
            content: r#"Command and Control (C2) beaconing is a common indicator of compromise (IOC) where malware establishes periodic communication with an external command server. Key indicators include: outbound network connections to suspicious IP addresses (especially on non-standard ports), regular intervals of network traffic (every 60 seconds, 5 minutes, etc.), DNS queries to known malicious domains, and encrypted traffic to unknown destinations. Detection methods include firewall log analysis, network flow monitoring, DNS query logging, and behavioral analysis of process network activity. When C2 beaconing is detected, immediate isolation of the affected system is recommended, followed by network traffic analysis to identify the full scope of the compromise."#,
            metadata: HashMap::from([
                ("source".to_string(), "threat_intel_kb".to_string()),
                ("priority".to_string(), "high".to_string()),
                ("category".to_string(), "ioc".to_string()),
                ("threat_type".to_string(), "c2_beaconing".to_string()),
                ("severity".to_string(), "critical".to_string()),
            ]),
        },
        ThreatIntelChunk {
            title: "Lateral Movement Indicators",
            content: r#"Lateral movement refers to techniques used by attackers to move through a network after initial compromise. Common indicators include: unexpected RDP/SSH connections between internal systems, SMB enumeration attempts, Pass-the-Hash attacks, credential dumping activities (LSASS memory access), and unusual service account activity. Detection requires monitoring authentication logs, network segmentation violations, privilege escalation attempts, and service account usage patterns. Administrative accounts should use MFA for all lateral RDP sessions. When lateral movement is detected, immediately isolate affected systems, revoke compromised credentials, and review all systems the attacker may have accessed."#,
            metadata: HashMap::from([
                ("source".to_string(), "threat_intel_kb".to_string()),
                ("priority".to_string(), "high".to_string()),
                ("category".to_string(), "ioc".to_string()),
                ("threat_type".to_string(), "lateral_movement".to_string()),
                ("severity".to_string(), "high".to_string()),
            ]),
        },
        ThreatIntelChunk {
            title: "Malware Analysis Framework",
            content: r#"Malware analysis follows a systematic approach: 1) Static analysis examines the binary without execution (file hashes, strings, imports, packer detection), 2) Dynamic analysis runs the sample in a controlled sandbox environment to observe behavior (network activity, file system changes, registry modifications, process creation), 3) Behavioral indicators include persistence mechanisms (scheduled tasks, registry run keys, service installation), data exfiltration attempts, and anti-analysis techniques (VM detection, debugger evasion). All suspicious binaries should be analyzed in an isolated sandbox environment before any remediation actions. Indicators of compromise (IOCs) including file hashes, IP addresses, domains, and behavioral patterns should be documented and shared with the security team for threat hunting across the environment."#,
            metadata: HashMap::from([
                ("source".to_string(), "threat_intel_kb".to_string()),
                ("priority".to_string(), "high".to_string()),
                ("category".to_string(), "analysis_framework".to_string()),
                ("threat_type".to_string(), "malware".to_string()),
                ("severity".to_string(), "high".to_string()),
            ]),
        },
        ThreatIntelChunk {
            title: "PowerShell Obfuscation Detection",
            content: r#"PowerShell is frequently used by attackers due to its powerful capabilities and ability to execute in memory. Obfuscation techniques include: base64 encoding, string concatenation, variable substitution, and use of aliases. Suspicious PowerShell indicators include: execution with -EncodedCommand flag, use of Invoke-Expression (IEX), downloading and executing scripts from remote URLs, bypassing execution policy, and running scripts with -WindowStyle Hidden. Detection requires PowerShell logging (Module Logging, Script Block Logging, Transcription), process monitoring, and command-line argument analysis. All PowerShell execution should be logged and monitored, with alerts configured for suspicious patterns. When obfuscated PowerShell is detected, immediately terminate the process, capture memory dumps, and analyze the script content in a sandbox."#,
            metadata: HashMap::from([
                ("source".to_string(), "threat_intel_kb".to_string()),
                ("priority".to_string(), "high".to_string()),
                ("category".to_string(), "ioc".to_string()),
                ("threat_type".to_string(), "powershell_obfuscation".to_string()),
                ("severity".to_string(), "high".to_string()),
            ]),
        },
        ThreatIntelChunk {
            title: "Zero-Trust Policy Framework",
            content: r#"Zero-trust security model assumes no implicit trust and requires verification for every access request. Core principles include: verify explicitly (authenticate and authorize based on all available data points), use least privilege access (limit user access with Just-In-Time and Just-Enough-Access policies), and assume breach (minimize blast radius and segment access). Implementation requires: network segmentation, identity and access management (IAM) with MFA, device compliance policies, application access controls, and continuous monitoring. All administrative accounts must use MFA for lateral movement and privileged access. Zero-trust policies should be enforced across all network segments, with no exceptions for internal traffic. Regular audits of access permissions and network segmentation are essential."#,
            metadata: HashMap::from([
                ("source".to_string(), "threat_intel_kb".to_string()),
                ("priority".to_string(), "high".to_string()),
                ("category".to_string(), "policy".to_string()),
                ("threat_type".to_string(), "zero_trust".to_string()),
                ("severity".to_string(), "medium".to_string()),
            ]),
        },
    ];

    // Ingest each chunk into the threat_intel namespace
    let namespace = "threat_intel";
    let twin_id = "twin-aegis"; // The Blue Flame orchestrator
    let memory_type = "RAGSource";
    let risk_level = "High";

    info!(
        namespace = %namespace,
        chunks_count = chunks.len(),
        "Starting threat intelligence KB ingestion"
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
        "Threat intelligence KB ingestion completed"
    );

    if error_count > 0 {
        Err(format!("Failed to ingest {} chunks", error_count).into())
    } else {
        Ok(())
    }
}

struct ThreatIntelChunk {
    title: &'static str,
    content: &'static str,
    metadata: HashMap<String, String>,
}
