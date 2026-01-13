//! Script to ingest Incident Response Knowledge Base into Qdrant Memory Service
//!
//! This script reads structured incident response playbooks and procedures and commits them to the
//! `incident_response` namespace in the memory service (Qdrant) for RAG retrieval by The Blue Flame orchestrator.
//!
//! Usage:
//!   cargo run --example ingest_incident_response_kb

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

    // Define the incident response KB chunks
    let chunks = vec![
        IncidentResponseChunk {
            title: "Incident Response Lifecycle",
            content: r#"The incident response lifecycle follows six phases: 1) Preparation - establishing policies, procedures, and tools before an incident occurs, 2) Identification - detecting and confirming security incidents through monitoring, alerts, and analysis, 3) Containment - isolating affected systems to prevent further damage (short-term containment for immediate threats, long-term containment for system restoration), 4) Eradication - removing the threat from all affected systems, including malware removal, closing backdoors, and patching vulnerabilities, 5) Recovery - restoring systems to normal operations with monitoring to ensure the threat is eliminated, 6) Lessons Learned - post-incident review to improve processes and prevent future incidents. Each phase requires documentation, coordination with stakeholders, and adherence to organizational policies."#,
            metadata: HashMap::from([
                ("source".to_string(), "incident_response_kb".to_string()),
                ("priority".to_string(), "high".to_string()),
                ("category".to_string(), "playbook".to_string()),
                ("incident_type".to_string(), "general".to_string()),
                ("phase".to_string(), "lifecycle".to_string()),
            ]),
        },
        IncidentResponseChunk {
            title: "Malware Incident Response Playbook",
            content: r#"When malware is detected: 1) Immediately isolate the affected system from the network (disconnect network cable or disable network adapter), 2) Document all indicators of compromise (IOCs) including file hashes, process names, network connections, and registry modifications, 3) Capture memory dumps and disk images for forensic analysis, 4) Analyze the malware in an isolated sandbox environment to understand its behavior and capabilities, 5) Identify the infection vector (email attachment, USB drive, malicious website, etc.), 6) Search for the same IOCs across all systems in the environment, 7) Remove the malware using appropriate tools (antivirus, manual removal, or system reimage if necessary), 8) Patch vulnerabilities that allowed the infection, 9) Restore systems from clean backups if available, 10) Monitor for re-infection and update detection rules. All actions must be documented with timestamps and evidence preserved for potential legal proceedings."#,
            metadata: HashMap::from([
                ("source".to_string(), "incident_response_kb".to_string()),
                ("priority".to_string(), "high".to_string()),
                ("category".to_string(), "playbook".to_string()),
                ("incident_type".to_string(), "malware".to_string()),
                ("severity".to_string(), "critical".to_string()),
            ]),
        },
        IncidentResponseChunk {
            title: "Data Exfiltration Response",
            content: r#"When data exfiltration is suspected: 1) Immediately block outbound network connections from affected systems while preserving evidence, 2) Identify what data was accessed or exfiltrated by reviewing file access logs, database query logs, and network traffic captures, 3) Determine the scope of the breach (which systems, which data, which users), 4) Preserve all logs, network captures, and system images as evidence, 5) Notify legal and compliance teams for potential regulatory reporting requirements, 6) Assess the sensitivity and classification of exfiltrated data, 7) Review access controls and authentication mechanisms to identify how the attacker gained access, 8) Implement additional monitoring and access controls to prevent further exfiltration, 9) Coordinate with law enforcement if criminal activity is suspected, 10) Prepare breach notification communications if required by regulations. Time is critical - rapid containment is essential to limit data loss."#,
            metadata: HashMap::from([
                ("source".to_string(), "incident_response_kb".to_string()),
                ("priority".to_string(), "high".to_string()),
                ("category".to_string(), "playbook".to_string()),
                ("incident_type".to_string(), "data_breach".to_string()),
                ("severity".to_string(), "critical".to_string()),
            ]),
        },
        IncidentResponseChunk {
            title: "Ransomware Response Procedures",
            content: r#"Ransomware incidents require immediate action: 1) Isolate affected systems immediately to prevent encryption from spreading, 2) Identify the ransomware variant and encryption method, 3) Determine if decryption tools are available (check No More Ransom project and security vendor resources), 4) Assess backup availability and integrity - verify backups are not also encrypted, 5) Do NOT pay the ransom unless all other options are exhausted and critical business operations are at risk, 6) Document all ransom demands, cryptocurrency wallet addresses, and communication with attackers, 7) Preserve encrypted files and system images for potential decryption attempts or forensic analysis, 8) Restore systems from clean backups after ensuring the infection vector is eliminated, 9) Patch all vulnerabilities that allowed the ransomware to enter, 10) Implement additional security controls (application whitelisting, network segmentation, email filtering) to prevent recurrence. Recovery time depends on backup availability and system complexity."#,
            metadata: HashMap::from([
                ("source".to_string(), "incident_response_kb".to_string()),
                ("priority".to_string(), "high".to_string()),
                ("category".to_string(), "playbook".to_string()),
                ("incident_type".to_string(), "ransomware".to_string()),
                ("severity".to_string(), "critical".to_string()),
            ]),
        },
        IncidentResponseChunk {
            title: "Phishing Incident Response",
            content: r#"Phishing incident response steps: 1) Identify all recipients of the phishing email, 2) Determine if any recipients clicked links or opened attachments, 3) Block the malicious sender, URLs, and domains at the email gateway and firewall, 4) Scan all systems that interacted with the phishing email for malware or compromise, 5) Reset passwords for any accounts that may have been compromised, 6) Review email forwarding rules and account settings for unauthorized changes, 7) Check for unauthorized access to email accounts, file shares, or other resources, 8) Send follow-up communications to all recipients warning them about the phishing attempt, 9) Provide security awareness training on identifying phishing emails, 10) Update email filtering rules to catch similar phishing attempts. If credentials were entered, immediately revoke access and force password resets with MFA enrollment."#,
            metadata: HashMap::from([
                ("source".to_string(), "incident_response_kb".to_string()),
                ("priority".to_string(), "high".to_string()),
                ("category".to_string(), "playbook".to_string()),
                ("incident_type".to_string(), "phishing".to_string()),
                ("severity".to_string(), "medium".to_string()),
            ]),
        },
        IncidentResponseChunk {
            title: "Remediation and Recovery Procedures",
            content: r#"Post-incident remediation requires systematic recovery: 1) Verify all threats are eliminated through comprehensive scanning and analysis, 2) Patch all vulnerabilities that allowed the incident to occur, 3) Update security controls (firewall rules, IDS/IPS signatures, endpoint protection) based on lessons learned, 4) Restore systems from clean backups or rebuild from scratch if backups are compromised, 5) Change all potentially compromised credentials (passwords, API keys, certificates), 6) Review and update access controls to implement least privilege principles, 7) Enable additional logging and monitoring for the attack vectors used, 8) Test all restored systems to ensure they function correctly, 9) Gradually restore services with increased monitoring, 10) Conduct a post-incident review to identify improvements. Recovery should prioritize business-critical systems first, with full restoration only after security is verified."#,
            metadata: HashMap::from([
                ("source".to_string(), "incident_response_kb".to_string()),
                ("priority".to_string(), "high".to_string()),
                ("category".to_string(), "playbook".to_string()),
                ("incident_type".to_string(), "remediation".to_string()),
                ("phase".to_string(), "recovery".to_string()),
            ]),
        },
    ];

    // Ingest each chunk into the incident_response namespace
    let namespace = "incident_response";
    let twin_id = "twin-aegis"; // The Blue Flame orchestrator
    let memory_type = "RAGSource";
    let risk_level = "Critical";

    info!(
        namespace = %namespace,
        chunks_count = chunks.len(),
        "Starting incident response KB ingestion"
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
        "Incident response KB ingestion completed"
    );

    if error_count > 0 {
        Err(format!("Failed to ingest {} chunks", error_count).into())
    } else {
        Ok(())
    }
}

struct IncidentResponseChunk {
    title: &'static str,
    content: &'static str,
    metadata: HashMap<String, String>,
}
