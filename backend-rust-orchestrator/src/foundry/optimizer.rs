//! Phoenix Guardrail Optimizer
//!
//! This module analyzes Strategic Recommendations from GovernanceReports
//! and generates draft rules that can be applied to prevent future conflicts.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{error, info, warn};
use uuid::Uuid;
use crate::api::reports::{GovernanceReport, StrategicRecommendation};

/// A draft rule generated from strategic recommendations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DraftRule {
    pub id: String,
    pub rule_type: RuleType,
    pub description: String,
    pub proposed_change: String,
    pub source_recommendations: Vec<usize>, // Entry indices that led to this rule
    pub confidence: f64, // 0.0-1.0, based on how many recommendations suggest similar changes
}

/// Type of rule modification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RuleType {
    /// Python regex pattern for scrubber
    PythonRegex {
        pattern: String,
        replacement: Option<String>,
        target: String, // e.g., "privacy_scrubber", "tone_filter"
    },
    /// Rust filter modification
    RustFilter {
        module: String,
        function: String,
        modification: String,
    },
    /// Configuration update
    ConfigUpdate {
        key: String,
        value: String,
    },
}

/// Response containing all draft rules
#[derive(Debug, Serialize)]
pub struct DraftRulesResponse {
    pub drafts: Vec<DraftRule>,
    pub total_recommendations_analyzed: usize,
}

/// Generate draft rules from a GovernanceReport
pub async fn generate_draft_rules(
    report: &GovernanceReport,
) -> Result<Vec<DraftRule>, String> {
    let mut draft_rules: Vec<DraftRule> = Vec::new();
    let mut rule_groups: HashMap<String, Vec<usize>> = HashMap::new();

    // Extract strategic recommendations
    let recommendations = match &report.strategic_recommendations {
        Some(recs) => recs,
        None => {
            info!("No strategic recommendations found in governance report");
            return Ok(Vec::new());
        }
    };

    info!(
        count = recommendations.len(),
        "Analyzing strategic recommendations for rule generation"
    );

    // Group recommendations by type of change they suggest
    for rec in recommendations {
        let entry = report.entries.get(rec.entry_index).ok_or_else(|| {
            format!("Entry index {} out of bounds", rec.entry_index)
        })?;

        // Parse recommendation text to identify rule type
        let rec_lower = rec.recommendation.to_lowercase();
        
        // Look for patterns suggesting scrubber updates
        if rec_lower.contains("scrubber") || rec_lower.contains("privacy") {
            let key = extract_scrubber_key(&rec.recommendation);
            rule_groups.entry(key).or_insert_with(Vec::new).push(rec.entry_index);
        }
        // Look for patterns suggesting filter updates
        else if rec_lower.contains("filter") || rec_lower.contains("compliance") {
            let key = extract_filter_key(&rec.recommendation);
            rule_groups.entry(key).or_insert_with(Vec::new).push(rec.entry_index);
        }
        // Look for configuration updates
        else if rec_lower.contains("config") || rec_lower.contains("setting") {
            let key = extract_config_key(&rec.recommendation);
            rule_groups.entry(key).or_insert_with(Vec::new).push(rec.entry_index);
        }
    }

    // Generate draft rules from grouped recommendations
    for (rule_key, entry_indices) in rule_groups {
        let confidence = entry_indices.len() as f64 / recommendations.len() as f64;
        
        // Get the first recommendation for this group to extract details
        let first_rec = recommendations.iter()
            .find(|r| entry_indices.contains(&r.entry_index))
            .ok_or_else(|| "No recommendation found for rule group".to_string())?;
        
        let draft_rule = DraftRule {
            id: format!("draft_{}", Uuid::new_v4().to_string().replace("-", "")[..8].to_string()),
            rule_type: infer_rule_type(&first_rec.recommendation, &rule_key)?,
            description: format!("Auto-generated from {} recommendation(s)", entry_indices.len()),
            proposed_change: generate_proposed_change(first_rec, &report.entries[first_rec.entry_index])?,
            source_recommendations: entry_indices,
            confidence,
        };
        
        draft_rules.push(draft_rule);
    }

    info!(
        count = draft_rules.len(),
        "Generated draft rules from strategic recommendations"
    );

    Ok(draft_rules)
}

/// Extract a key for grouping scrubber-related recommendations
fn extract_scrubber_key(recommendation: &str) -> String {
    let rec_lower = recommendation.to_lowercase();
    
    if rec_lower.contains("privacy") {
        "privacy_scrubber".to_string()
    } else if rec_lower.contains("diagnostic") || rec_lower.contains("test") {
        "diagnostic_scrubber".to_string()
    } else {
        "general_scrubber".to_string()
    }
}

/// Extract a key for grouping filter-related recommendations
fn extract_filter_key(recommendation: &str) -> String {
    let rec_lower = recommendation.to_lowercase();
    
    if rec_lower.contains("tone") {
        "tone_filter".to_string()
    } else if rec_lower.contains("security") {
        "security_filter".to_string()
    } else {
        "compliance_filter".to_string()
    }
}

/// Extract a key for grouping config-related recommendations
fn extract_config_key(recommendation: &str) -> String {
    let rec_lower = recommendation.to_lowercase();
    
    if rec_lower.contains("threshold") {
        "compliance_threshold".to_string()
    } else if rec_lower.contains("mode") {
        "operation_mode".to_string()
    } else {
        "general_config".to_string()
    }
}

/// Infer the rule type from a recommendation
fn infer_rule_type(recommendation: &str, rule_key: &str) -> Result<RuleType, String> {
    let rec_lower = recommendation.to_lowercase();
    
    // Check for regex pattern suggestions
    if rec_lower.contains("regex") || rec_lower.contains("pattern") {
        // Try to extract a pattern from the recommendation
        let pattern = extract_regex_pattern(recommendation)
            .unwrap_or_else(|| r".*".to_string());
        
        return Ok(RuleType::PythonRegex {
            pattern,
            replacement: None,
            target: rule_key.to_string(),
        });
    }
    
    // Check for Rust filter modifications
    if rec_lower.contains("rust") || rec_lower.contains("filter") {
        return Ok(RuleType::RustFilter {
            module: "compliance".to_string(),
            function: format!("{}_filter", rule_key.replace("_", "")),
            modification: recommendation.to_string(),
        });
    }
    
    // Default to config update
    Ok(RuleType::ConfigUpdate {
        key: rule_key.to_string(),
        value: "auto_configured".to_string(),
    })
}

/// Extract a regex pattern from recommendation text (simple heuristic)
fn extract_regex_pattern(recommendation: &str) -> Option<String> {
    // Look for patterns like "allow X" or "exclude Y"
    let rec_lower = recommendation.to_lowercase();
    
    if rec_lower.contains("localhost") || rec_lower.contains("127.0.0.1") {
        return Some(r"127\.0\.0\.1|localhost".to_string());
    }
    
    if rec_lower.contains("diagnostic") || rec_lower.contains("test") {
        return Some(r"(test|diagnostic|debug).*".to_string());
    }
    
    None
}

/// Generate a proposed change description
fn generate_proposed_change(
    recommendation: &StrategicRecommendation,
    entry: &crate::api::reports::GovernanceReportEntry,
) -> Result<String, String> {
    let mut change = String::new();
    
    change.push_str(&format!("Based on override #{} for agent {}:\n", 
        recommendation.entry_index + 1, entry.agent_id));
    change.push_str(&format!("Rationale: {}\n", entry.rationale));
    change.push_str(&format!("Recommendation: {}", recommendation.recommendation));
    
    Ok(change)
}

/// Apply a draft rule (placeholder - actual implementation would modify code/config)
pub async fn apply_draft_rule(
    rule: &DraftRule,
    agents_repo_path: &PathBuf,
) -> Result<String, String> {
    info!(
        rule_id = %rule.id,
        rule_type = ?rule.rule_type,
        "Applying draft rule"
    );
    
    // In a full implementation, this would:
    // 1. Parse the rule type
    // 2. Locate the target file (scrubber script, Rust module, config file)
    // 3. Apply the modification
    // 4. Commit the change to git
    // 5. Trigger a compliance test
    
    match &rule.rule_type {
        RuleType::PythonRegex { pattern, target, .. } => {
            info!(
                pattern = %pattern,
                target = %target,
                "Would apply Python regex rule"
            );
            Ok(format!("Python regex rule applied: {} -> {}", target, pattern))
        }
        RuleType::RustFilter { module, function, .. } => {
            info!(
                module = %module,
                function = %function,
                "Would apply Rust filter modification"
            );
            Ok(format!("Rust filter modified: {}::{}", module, function))
        }
        RuleType::ConfigUpdate { key, value } => {
            info!(
                key = %key,
                value = %value,
                "Would apply config update"
            );
            Ok(format!("Config updated: {} = {}", key, value))
        }
    }
}
