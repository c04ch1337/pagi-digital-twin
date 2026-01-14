//! Phoenix Governance Report Generator
//!
//! This module generates comprehensive governance reports documenting all
//! strategic overrides, consensus conflicts, and their impact on the mesh.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json;
use tokio::process::Command as TokioCommand;
use tracing::{error, info, warn};

use crate::network::consensus::PhoenixConsensus;
use crate::network::memory_exchange::PhoenixMemoryExchangeServiceImpl;

/// Override record extracted from git log
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverrideRecord {
    pub commit_hash: String,
    pub override_commit_hash: String,
    pub timestamp: String,
    pub rationale: String,
    pub agent_id: Option<String>,
}

/// Conflict profile showing which nodes voted against
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictProfile {
    pub node_id: String,
    pub compliance_score: f64,
    pub approved: bool,
    pub timestamp: String,
}

/// Governance report entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceReportEntry {
    pub agent_id: String,
    pub commit_hash: String,
    pub override_timestamp: String,
    pub rationale: String,
    pub conflict_profile: Vec<ConflictProfile>,
    pub redacted_count_at_override: usize,
    pub knowledge_fragments_since: usize,
    pub impact_summary: String,
}

/// Strategic recommendation for an override entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategicRecommendation {
    pub entry_index: usize,
    pub recommendation: String,
}

/// Full governance report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceReport {
    pub generated_at: String,
    pub total_overrides: usize,
    pub entries: Vec<GovernanceReportEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strategic_recommendations: Option<Vec<StrategicRecommendation>>,
}

/// Extract override records from git log
pub async fn extract_override_records(
    repo_path: &PathBuf,
) -> Result<Vec<OverrideRecord>, String> {
    if !repo_path.exists() {
        return Err(format!("Repository path does not exist: {}", repo_path.display()));
    }

    if !repo_path.join(".git").exists() {
        return Err(format!("Path is not a git repository: {}", repo_path.display()));
    }

    info!(repo_path = %repo_path.display(), "Extracting override records from git log");

    // Use git log to find all commits with [PHOENIX-OVERRIDE] in the message
    let output = Command::new("git")
        .arg("log")
        .arg("--all")
        .arg("--grep=[PHOENIX-OVERRIDE]")
        .arg("--format=%H|%ai|%s|%b")
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to execute git log: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Git log failed: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut records = Vec::new();

    for line in stdout.lines() {
        if line.trim().is_empty() {
            continue;
        }

        // Parse format: commit_hash|timestamp|subject|body
        let parts: Vec<&str> = line.splitn(4, '|').collect();
        if parts.len() < 3 {
            warn!(line = %line, "Skipping malformed git log line");
            continue;
        }

        let override_commit_hash = parts[0].trim().to_string();
        let timestamp_str = parts[1].trim().to_string();
        let subject = parts.get(2).map(|s| s.trim()).unwrap_or("");
        let body = parts.get(3).map(|s| s.trim()).unwrap_or("");

        // Extract the original commit hash and rationale from the commit message
        let (commit_hash, rationale) = parse_override_message(subject, body);

        // Try to extract agent ID from the commit message or subject
        let agent_id = extract_agent_id(subject, body);

        records.push(OverrideRecord {
            commit_hash: commit_hash.clone(),
            override_commit_hash,
            timestamp: timestamp_str,
            rationale,
            agent_id,
        });
    }

    info!(count = records.len(), "Extracted override records from git log");
    Ok(records)
}

/// Parse override commit message to extract commit hash and rationale
fn parse_override_message(subject: &str, body: &str) -> (String, String) {
    // Expected format: "[PHOENIX-OVERRIDE] Strategic override for commit {hash}\n\nRationale: {rationale}"
    let full_message = format!("{}\n{}", subject, body);
    
    // Try to extract commit hash from subject or body
    let commit_hash = if let Some(start) = full_message.find("commit ") {
        let after_commit = &full_message[start + 7..];
        if let Some(end) = after_commit.find(|c: char| c == '\n' || c == ' ' || c == '\r') {
            after_commit[..end].trim().to_string()
        } else if after_commit.len() > 0 && after_commit.len() <= 40 {
            after_commit.trim().to_string()
        } else {
            "unknown".to_string()
        }
    } else {
        "unknown".to_string()
    };

    // Extract rationale
    let rationale = if let Some(start) = full_message.find("Rationale:") {
        full_message[start + 10..].trim().to_string()
    } else if let Some(start) = full_message.find("rationale:") {
        full_message[start + 10..].trim().to_string()
    } else {
        // Fallback: use body if it exists, otherwise subject
        if !body.trim().is_empty() {
            body.trim().to_string()
        } else {
            subject.trim().to_string()
        }
    };

    (commit_hash, rationale)
}

/// Extract agent ID from commit message
fn extract_agent_id(subject: &str, body: &str) -> Option<String> {
    let full_message = format!("{} {}", subject, body).to_lowercase();
    
    // Look for patterns like "agent: {id}" or "agent_id: {id}"
    for pattern in &["agent:", "agent_id:", "agent id:"] {
        if let Some(start) = full_message.find(pattern) {
            let after_pattern = &full_message[start + pattern.len()..];
            let id = after_pattern
                .split_whitespace()
                .next()
                .map(|s| s.trim().to_string());
            if let Some(ref id_str) = id {
                if !id_str.is_empty() && id_str.len() < 100 {
                    return id;
                }
            }
        }
    }
    
    None
}

/// Generate governance report
pub async fn generate_governance_report(
    repo_path: &PathBuf,
    consensus: &PhoenixConsensus,
    memory_exchange: &PhoenixMemoryExchangeServiceImpl,
) -> Result<GovernanceReport, String> {
    info!("Generating Phoenix Governance Report");

    // Extract override records from git log
    let override_records = extract_override_records(repo_path).await?;

    let mut entries = Vec::new();

    for record in override_records {
        // Get consensus votes for the commit
        let votes = consensus.get_votes_for_commit(&record.commit_hash).await;
        
        let conflict_profile: Vec<ConflictProfile> = if let Some(vote_list) = votes {
            vote_list
                .into_iter()
                .map(|v| ConflictProfile {
                    node_id: v.node_id,
                    compliance_score: v.compliance_score,
                    approved: v.approved,
                    timestamp: v.timestamp,
                })
                .collect()
        } else {
            Vec::new()
        };

        // Get redacted count at time of override (approximate from memory exchange stats)
        // Note: This is an approximation since we don't store historical redacted counts
        let redacted_count_at_override = 0; // TODO: Track historical redacted counts

        // Calculate knowledge fragments exchanged since override
        // This is an approximation based on current topic frequencies
        let topic_frequencies = memory_exchange.get_topic_frequencies().await;
        let knowledge_fragments_since = topic_frequencies
            .values()
            .sum::<usize>();

        // Generate impact summary
        let disapproving_nodes: Vec<&str> = conflict_profile
            .iter()
            .filter(|c| !c.approved)
            .map(|c| c.node_id.as_str())
            .collect();
        
        let impact_summary = if disapproving_nodes.is_empty() {
            "Override applied with no node conflicts. All nodes approved the agent.".to_string()
        } else {
            format!(
                "Override bypassed {} disapproving node(s): {}. Average compliance score of disapproving nodes: {:.1}%",
                disapproving_nodes.len(),
                disapproving_nodes.join(", "),
                conflict_profile
                    .iter()
                    .filter(|c| !c.approved)
                    .map(|c| c.compliance_score)
                    .sum::<f64>()
                    / (disapproving_nodes.len() as f64).max(1.0)
            )
        };

        let agent_id = record.agent_id.unwrap_or_else(|| {
            // Try to extract from commit hash or use default
            format!("agent-{}", &record.commit_hash[..8.min(record.commit_hash.len())])
        });

        entries.push(GovernanceReportEntry {
            agent_id,
            commit_hash: record.commit_hash.clone(),
            override_timestamp: record.timestamp,
            rationale: record.rationale,
            conflict_profile,
            redacted_count_at_override,
            knowledge_fragments_since,
            impact_summary,
        });
    }

    // Sort entries by timestamp (most recent first)
    entries.sort_by(|a, b| {
        // Parse timestamps and compare
        let time_a = DateTime::parse_from_rfc3339(&a.override_timestamp)
            .or_else(|_| DateTime::parse_from_str(&a.override_timestamp, "%Y-%m-%d %H:%M:%S %z"))
            .unwrap_or_else(|_| Utc::now().into());
        let time_b = DateTime::parse_from_rfc3339(&b.override_timestamp)
            .or_else(|_| DateTime::parse_from_str(&b.override_timestamp, "%Y-%m-%d %H:%M:%S %z"))
            .unwrap_or_else(|_| Utc::now().into());
        time_b.cmp(&time_a) // Reverse order (newest first)
    });

    // Call Python analyzer for strategic recommendations
    let strategic_recommendations = call_strategic_analyzer(&entries).await;
    
    Ok(GovernanceReport {
        generated_at: Utc::now().to_rfc3339(),
        total_overrides: entries.len(),
        entries,
        strategic_recommendations,
    })
}

/// Call Python strategic analyzer script
async fn call_strategic_analyzer(
    entries: &[GovernanceReportEntry],
) -> Option<Vec<StrategicRecommendation>> {
    if entries.is_empty() {
        return None;
    }

    // Build temporary report JSON
    let temp_report = GovernanceReport {
        generated_at: Utc::now().to_rfc3339(),
        total_overrides: entries.len(),
        entries: entries.to_vec(),
        strategic_recommendations: None,
    };

    let report_json = match serde_json::to_string(&temp_report) {
        Ok(json) => json,
        Err(e) => {
            warn!(error = %e, "Failed to serialize report for analyzer");
            return None;
        }
    };

    // Find script path (relative to workspace root)
    let script_path = PathBuf::from("scripts/phoenix_analyzer.py");
    let workspace_root = std::env::current_dir().ok()?;
    let full_script_path = workspace_root.join(&script_path);

    if !full_script_path.exists() {
        warn!(
            script_path = %full_script_path.display(),
            "Strategic analyzer script not found, skipping AI analysis"
        );
        return None;
    }

    info!("Calling strategic analyzer script");

    // Execute Python script
    let output = match TokioCommand::new("python3")
        .arg(&full_script_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(mut child) => {
            // Write JSON to stdin
            if let Some(stdin) = child.stdin.take() {
                use tokio::io::AsyncWriteExt;
                if let Err(e) = tokio::io::AsyncWriteExt::write_all(&mut stdin, report_json.as_bytes()).await {
                    warn!(error = %e, "Failed to write to analyzer stdin");
                    let _ = child.kill().await;
                    return None;
                }
            }

            match child.wait_with_output().await {
                Ok(output) => output,
                Err(e) => {
                    warn!(error = %e, "Failed to wait for analyzer process");
                    return None;
                }
            }
        }
        Err(e) => {
            warn!(error = %e, "Failed to spawn analyzer process");
            return None;
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!(
            exit_code = output.status.code(),
            stderr = %stderr,
            "Analyzer script failed"
        );
        return None;
    }

    // Parse analyzer output
    let stdout = String::from_utf8_lossy(&output.stdout);
    let analyzer_result: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(result) => result,
        Err(e) => {
            warn!(
                error = %e,
                stdout = %stdout,
                "Failed to parse analyzer output"
            );
            return None;
        }
    };

    let recommendations_map = analyzer_result
        .get("recommendations")?
        .as_object()?;

    let mut recommendations = Vec::new();
    for (idx_str, rec_value) in recommendations_map {
        if let (Ok(idx), Some(rec_text)) = (idx_str.parse::<usize>(), rec_value.as_str()) {
            recommendations.push(StrategicRecommendation {
                entry_index: idx,
                recommendation: rec_text.to_string(),
            });
        }
    }

    if recommendations.is_empty() {
        return None;
    }

    info!(
        count = recommendations.len(),
        "Strategic recommendations generated"
    );
    Some(recommendations)
}

/// Generate Markdown report from governance report
pub fn generate_markdown_report(report: &GovernanceReport) -> String {
    let mut markdown = String::new();

    markdown.push_str("# Phoenix Governance Report\n\n");
    markdown.push_str(&format!("**Generated:** {}\n\n", report.generated_at));
    markdown.push_str(&format!("**Total Strategic Overrides:** {}\n\n", report.total_overrides));
    markdown.push_str("---\n\n");

    if report.entries.is_empty() {
        markdown.push_str("No strategic overrides have been recorded.\n");
        return markdown;
    }

    for (idx, entry) in report.entries.iter().enumerate() {
        markdown.push_str(&format!("## Override #{}\n\n", idx + 1));
        markdown.push_str(&format!("**Agent ID:** `{}`\n\n", entry.agent_id));
        markdown.push_str(&format!("**Commit Hash:** `{}`\n\n", entry.commit_hash));
        markdown.push_str(&format!("**Override Timestamp:** {}\n\n", entry.override_timestamp));
        markdown.push_str(&format!("**Rationale:**\n\n{}\n\n", entry.rationale));
        
        markdown.push_str("### Conflict Profile\n\n");
        if entry.conflict_profile.is_empty() {
            markdown.push_str("No consensus votes recorded for this commit.\n\n");
        } else {
            markdown.push_str("| Node ID | Compliance Score | Approved | Timestamp |\n");
            markdown.push_str("|---------|------------------|----------|-----------|\n");
            for conflict in &entry.conflict_profile {
                markdown.push_str(&format!(
                    "| `{}` | {:.1}% | {} | {} |\n",
                    conflict.node_id,
                    conflict.compliance_score,
                    if conflict.approved { "✅ Yes" } else { "❌ No" },
                    conflict.timestamp
                ));
            }
            markdown.push_str("\n");
        }

        markdown.push_str("### Impact Analysis\n\n");
        markdown.push_str(&format!("**Redacted Entities at Override:** {}\n\n", entry.redacted_count_at_override));
        markdown.push_str(&format!("**Knowledge Fragments Exchanged Since:** {}\n\n", entry.knowledge_fragments_since));
        markdown.push_str(&format!("**Impact Summary:**\n\n{}\n\n", entry.impact_summary));
        
        // Add strategic recommendation if available
        if let Some(ref recommendations) = report.strategic_recommendations {
            if let Some(rec) = recommendations.iter().find(|r| r.entry_index == idx) {
                markdown.push_str("### Strategic Recommendation\n\n");
                markdown.push_str(&format!("{}\n\n", rec.recommendation));
            }
        }
        
        markdown.push_str("---\n\n");
    }

    markdown.push_str("## Report Metadata\n\n");
    markdown.push_str("This report was generated by the Phoenix Governance System.\n");
    markdown.push_str("All strategic overrides are recorded in git history with `[PHOENIX-OVERRIDE]` tags.\n");

    markdown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_override_message() {
        let subject = "[PHOENIX-OVERRIDE] Strategic override for commit abc123";
        let body = "Rationale: High-performance agent needed for critical operation";
        let (commit_hash, rationale) = parse_override_message(subject, body);
        assert_eq!(commit_hash, "abc123");
        assert!(rationale.contains("High-performance"));
    }

    #[test]
    fn test_extract_agent_id() {
        let subject = "[PHOENIX-OVERRIDE] Override for agent: nmap-scanner";
        let body = "";
        let agent_id = extract_agent_id(subject, body);
        assert_eq!(agent_id, Some("nmap-scanner".to_string()));
    }
}
