//! Ingestion Stress Test
//!
//! Generates 100 dummy files with high-variance content (logs, ethics docs, technical specs)
//! and batch-drops them into data/ingest/ to monitor UI performance and ingestion throughput.
//!
//! Run with: cargo test --test ingestion_stress_test -- --ignored --nocapture

use std::fs;
use std::path::PathBuf;

/// Generate test files with varied content types
fn generate_test_files(output_dir: &PathBuf, count: usize) -> Result<Vec<PathBuf>, std::io::Error> {
    fs::create_dir_all(output_dir)?;
    
    let mut files = Vec::new();
    
    // Content templates for different domains
    let mind_templates = vec![
        "API Configuration Guide\n\nThis document describes the REST API endpoints for the system.\nEndpoints:\n- GET /api/v1/status\n- POST /api/v1/execute\n- PUT /api/v1/update\n\nTechnical specifications:\n- Protocol: HTTP/1.1\n- Authentication: Bearer token\n- Rate limit: 100 requests/minute",
        "System Architecture Documentation\n\nComponent Overview:\n- Frontend: React/TypeScript\n- Backend: Rust/Axum\n- Database: Qdrant vector store\n- Message Queue: Redis\n\nDeployment:\n- Docker containers\n- Kubernetes orchestration\n- CI/CD pipeline with GitHub Actions",
        "Code Playbook: Error Handling\n\n1. Catch exceptions at the boundary\n2. Log with context\n3. Return structured errors\n4. Retry with exponential backoff\n\nExample:\n```rust\nmatch result {\n    Ok(v) => v,\n    Err(e) => {\n        log::error!(\"Operation failed: {}\", e);\n        return Err(Error::Internal);\n    }\n}\n```",
    ];
    
    let body_templates = vec![
        "System Telemetry Log\n\nTimestamp: 2024-01-15T10:30:00Z\nCPU Usage: 45.2%\nMemory Usage: 2.3GB / 8GB\nNetwork I/O: 125MB/s\nDisk I/O: 50MB/s\nProcess Count: 142\n\nPerformance Metrics:\n- Response Time: 120ms avg\n- Throughput: 850 req/s\n- Error Rate: 0.01%",
        "Hardware Status Report\n\nCPU: Intel Xeon E5-2680 v4 @ 2.40GHz\nCores: 14 physical, 28 logical\nTemperature: 65°C\nFan Speed: 2400 RPM\n\nMemory:\n- Total: 64GB DDR4\n- Used: 32GB\n- Available: 32GB\n\nStorage:\n- SSD: 1TB (75% used)\n- HDD: 4TB (50% used)",
        "Agent Execution Log\n\nAgent ID: agent-12345\nStart Time: 2024-01-15T10:00:00Z\nEnd Time: 2024-01-15T10:05:00Z\nStatus: COMPLETED\n\nActions Taken:\n1. Read configuration file\n2. Execute tool: network_scan\n3. Store results in memory\n\nResource Usage:\n- CPU: 12% peak\n- Memory: 512MB peak",
    ];
    
    let heart_templates = vec![
        "User Preference Profile\n\nUser ID: user-789\nPreferences:\n- Theme: Dark mode\n- Language: English\n- Timezone: UTC-5\n- Notification Level: Medium\n\nInteraction History:\n- Total Sessions: 45\n- Average Session Duration: 25 minutes\n- Preferred Agents: Technical Support, Security Analyst",
        "Agent Persona Configuration\n\nPersona: Friendly Assistant\nVoice Tone: Professional but warm\nResponse Style: Detailed explanations\n\nPersonalization:\n- Remembers user's common tasks\n- Adapts to user's technical level\n- Provides contextual suggestions\n\nFeedback Score: 4.8/5.0",
        "User Feedback Collection\n\nSession ID: session-abc123\nRating: 5/5\nComments: \"Very helpful and responsive\"\n\nImprovement Suggestions:\n- Faster response times\n- More examples in explanations\n\nUser Satisfaction: High",
    ];
    
    let soul_templates = vec![
        "Security Audit Report\n\nDate: 2024-01-15\nAuditor: Security Team\n\nFindings:\n- Critical: 2 vulnerabilities found\n- High: 5 configuration issues\n- Medium: 12 compliance gaps\n\nRecommendations:\n1. Patch CVE-2024-0001 immediately\n2. Update firewall rules\n3. Review access controls\n\nCompliance Status: PARTIAL",
        "Governance Policy Document\n\nPolicy: Data Privacy and Protection\nVersion: 2.1\nEffective Date: 2024-01-01\n\nRequirements:\n- All data must be encrypted at rest\n- Access logs must be retained for 90 days\n- PII must be redacted in logs\n- Regular security audits required\n\nViolation Penalties:\n- First offense: Warning\n- Repeat offense: Suspension",
        "Ethical Guidelines for AI Agents\n\nPrinciple 1: Transparency\n- All decisions must be explainable\n- Users must understand agent reasoning\n\nPrinciple 2: Privacy\n- No unauthorized data collection\n- User consent required for sharing\n\nPrinciple 3: Safety\n- No destructive actions without approval\n- Risk assessment required for all operations\n\nPrinciple 4: Accountability\n- All actions logged\n- Audit trail maintained",
    ];
    
    // Generate files with varied content (distribute evenly across domains)
    for i in 0..count {
        let domain_type = i % 4;
        let template_set = match domain_type {
            0 => &mind_templates,
            1 => &body_templates,
            2 => &heart_templates,
            _ => &soul_templates,
        };
        
        let template_idx = (i / 4) % template_set.len();
        let template = template_set[template_idx];
        let domain_name = match domain_type {
            0 => "mind",
            1 => "body",
            2 => "heart",
            _ => "soul",
        };
        
        let filename = format!("stress_test_{}_{:03}.txt", domain_name, i);
        let file_path = output_dir.join(&filename);
        
        // Add some variation to content
        let content = format!("{}\n\n---\nTest File: {}\nGenerated: Stress Test\nDomain: {}\n", 
            template, filename, domain_name);
        
        fs::write(&file_path, content)?;
        files.push(file_path);
    }
    
    Ok(files)
}

#[test]
#[ignore] // Ignore by default - run with: cargo test --test ingestion_stress_test -- --ignored
fn test_generate_stress_files() {
    // This test generates files and drops them into the ingest directory
    // The actual ingestor will pick them up via file watching
    // For a full integration test, use the API endpoint: POST /api/knowledge/ingest
    
    let test_dir = PathBuf::from("data/ingest/stress_test");
    let _ = fs::remove_dir_all(&test_dir); // Clean up previous test
    
    println!("Generating 100 test files...");
    let test_files = generate_test_files(&test_dir, 100)
        .expect("Failed to generate test files");
    
    println!("Generated {} test files in {}", test_files.len(), test_dir.display());
    println!("\nFiles are ready for ingestion. Monitor the UI dashboard for progress.");
    println!("\nTo trigger ingestion via API:");
    println!("  curl -X POST http://localhost:8182/api/knowledge/ingest -H 'Content-Type: application/json' -d '{{}}'");
    println!("\nOr wait for the file watcher to detect them automatically.");
    
    // Verify files were created
    assert_eq!(test_files.len(), 100, "Should generate 100 test files");
    
    // Verify file distribution
    let mind_count = test_files.iter().filter(|f| f.to_string_lossy().contains("mind")).count();
    let body_count = test_files.iter().filter(|f| f.to_string_lossy().contains("body")).count();
    let heart_count = test_files.iter().filter(|f| f.to_string_lossy().contains("heart")).count();
    let soul_count = test_files.iter().filter(|f| f.to_string_lossy().contains("soul")).count();
    
    println!("\nFile distribution:");
    println!("  Mind: {} files", mind_count);
    println!("  Body: {} files", body_count);
    println!("  Heart: {} files", heart_count);
    println!("  Soul: {} files", soul_count);
    
    // Cleanup can be done manually after testing
    // let _ = fs::remove_dir_all(&test_dir);
    
    println!("\n✅ Test file generation passed!");
    println!("Note: Run ingestion manually or via API to test full pipeline");
}

#[test]
#[ignore]
fn test_generate_performance_test_files() {
    // Generate files for each domain separately for performance metrics testing
    let test_dir = PathBuf::from("data/ingest/perf_test");
    let _ = fs::remove_dir_all(&test_dir);
    
    let domain_dirs = vec![
        ("mind", test_dir.join("mind")),
        ("body", test_dir.join("body")),
        ("heart", test_dir.join("heart")),
        ("soul", test_dir.join("soul")),
    ];
    
    println!("Generating performance test files for each domain...");
    
    for (domain, dir) in &domain_dirs {
        let files = generate_test_files(dir, 10).unwrap();
        println!("  {}: {} files", domain, files.len());
    }
    
    println!("\nTo test performance metrics:");
    println!("1. Start the orchestrator");
    println!("2. Trigger ingestion via API: POST /api/knowledge/ingest");
    println!("3. Check /api/knowledge/ingest/status for performance_metrics field");
    println!("4. Monitor the UI dashboard for time-to-ingest metrics per domain");
    
    println!("\n✅ Performance metrics test setup complete!");
}
