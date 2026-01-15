//! Knowledge Domain Router
//!
//! Maps the Mind/Body/Heart/Soul metaphor domains to backend namespaces and Qdrant collections.
//! Enables parallel multi-domain queries for comprehensive knowledge retrieval.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Knowledge domain enum representing the four core domains
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KnowledgeDomain {
    /// Mind: Intellectual/Technical - Logic, technical specs, and verified procedures
    Mind,
    /// Body: Physical/Telemetry - Real-time hardware state and system logs
    Body,
    /// Heart: Emotional/Personal - User context, persona biases, and interaction history
    Heart,
    /// Soul: Ethical/Governance - Guardrails, leadership wisdom, and safety audits
    Soul,
}

impl KnowledgeDomain {
    /// Get all domains
    pub fn all() -> Vec<Self> {
        vec![Self::Mind, Self::Body, Self::Heart, Self::Soul]
    }

    /// Get display name
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Mind => "Mind",
            Self::Body => "Body",
            Self::Heart => "Heart",
            Self::Soul => "Soul",
        }
    }

    /// Get UI label
    pub fn ui_label(&self) -> &'static str {
        match self {
            Self::Mind => "Intellectual",
            Self::Body => "Physical",
            Self::Heart => "Emotional",
            Self::Soul => "Ethical",
        }
    }
}

/// Domain routing configuration mapping domains to backend resources
#[derive(Debug, Clone)]
pub struct DomainRouter {
    /// Maps domains to Memory Service namespaces
    namespace_map: HashMap<KnowledgeDomain, Vec<String>>,
    /// Maps domains to Qdrant collection names
    collection_map: HashMap<KnowledgeDomain, Vec<String>>,
}

impl DomainRouter {
    /// Create a new domain router with default mappings
    pub fn new() -> Self {
        let mut namespace_map = HashMap::new();
        let mut collection_map = HashMap::new();

        // Mind: Technical/Logical knowledge
        namespace_map.insert(
            KnowledgeDomain::Mind,
            vec!["system_config".to_string(), "default".to_string()],
        );
        collection_map.insert(
            KnowledgeDomain::Mind,
            vec!["global_playbooks".to_string()],
        );

        // Body: Physical/Telemetry data
        namespace_map.insert(
            KnowledgeDomain::Body,
            vec!["incident_response".to_string()],
        );
        collection_map.insert(
            KnowledgeDomain::Body,
            vec!["telemetry".to_string(), "agent_logs".to_string()],
        );

        // Heart: Personal/User context
        namespace_map.insert(
            KnowledgeDomain::Heart,
            vec!["user_preferences".to_string()],
        );
        collection_map.insert(
            KnowledgeDomain::Heart,
            vec!["agent_identities".to_string()],
        );

        // Soul: Ethical/Governance
        namespace_map.insert(
            KnowledgeDomain::Soul,
            vec!["corporate_context".to_string()],
        );
        collection_map.insert(
            KnowledgeDomain::Soul,
            vec!["quarantine_list".to_string()],
        );

        Self {
            namespace_map,
            collection_map,
        }
    }

    /// Get namespaces for a domain
    pub fn get_namespaces(&self, domain: KnowledgeDomain) -> Vec<String> {
        self.namespace_map
            .get(&domain)
            .cloned()
            .unwrap_or_default()
    }

    /// Get collections for a domain
    pub fn get_collections(&self, domain: KnowledgeDomain) -> Vec<String> {
        self.collection_map
            .get(&domain)
            .cloned()
            .unwrap_or_default()
    }

    /// Get all namespaces for multiple domains
    pub fn get_namespaces_for_domains(&self, domains: &[KnowledgeDomain]) -> Vec<String> {
        let mut namespaces = Vec::new();
        for domain in domains {
            namespaces.extend(self.get_namespaces(*domain));
        }
        // Remove duplicates while preserving order
        let mut seen = std::collections::HashSet::new();
        namespaces.retain(|ns| seen.insert(ns.clone()));
        namespaces
    }

    /// Get all collections for multiple domains
    pub fn get_collections_for_domains(&self, domains: &[KnowledgeDomain]) -> Vec<String> {
        let mut collections = Vec::new();
        for domain in domains {
            collections.extend(self.get_collections(*domain));
        }
        // Remove duplicates while preserving order
        let mut seen = std::collections::HashSet::new();
        collections.retain(|coll| seen.insert(coll.clone()));
        collections
    }

    /// Determine which domains are relevant to a query based on keywords
    pub fn infer_domains_from_query(&self, query: &str) -> Vec<KnowledgeDomain> {
        let query_lower = query.to_lowercase();
        let mut domains = Vec::new();

        // Mind keywords: technical, logic, procedure, spec, playbook, tool
        if query_lower.contains("how") || query_lower.contains("procedure")
            || query_lower.contains("playbook") || query_lower.contains("tool")
            || query_lower.contains("spec") || query_lower.contains("technical")
        {
            domains.push(KnowledgeDomain::Mind);
        }

        // Body keywords: system, hardware, telemetry, log, state, performance
        if query_lower.contains("system") || query_lower.contains("hardware")
            || query_lower.contains("telemetry") || query_lower.contains("log")
            || query_lower.contains("state") || query_lower.contains("performance")
            || query_lower.contains("cpu") || query_lower.contains("memory")
        {
            domains.push(KnowledgeDomain::Body);
        }

        // Heart keywords: user, persona, preference, personal, interaction
        if query_lower.contains("user") || query_lower.contains("persona")
            || query_lower.contains("preference") || query_lower.contains("personal")
            || query_lower.contains("interaction")
        {
            domains.push(KnowledgeDomain::Heart);
        }

        // Soul keywords: safety, ethics, governance, policy, guardrail, audit, security, danger
        if query_lower.contains("safety") || query_lower.contains("ethics")
            || query_lower.contains("governance") || query_lower.contains("policy")
            || query_lower.contains("guardrail") || query_lower.contains("audit")
            || query_lower.contains("compliance") || query_lower.contains("risk")
            || query_lower.contains("security") || query_lower.contains("danger")
            || query_lower.contains("threat") || query_lower.contains("breach")
            || query_lower.contains("vulnerability") || query_lower.contains("attack")
        {
            domains.push(KnowledgeDomain::Soul);
        }

        // If no domains inferred, default to all
        if domains.is_empty() {
            domains = KnowledgeDomain::all();
        }

        domains
    }
}

impl Default for DomainRouter {
    fn default() -> Self {
        Self::new()
    }
}

/// Context synthesizer that labels retrieved data by domain
pub struct ContextSynthesizer;

impl ContextSynthesizer {
    /// Label content with domain context
    pub fn label_content(domain: KnowledgeDomain, content: &str) -> String {
        let domain_label = match domain {
            KnowledgeDomain::Mind => "### Mind (Intellectual) ###",
            KnowledgeDomain::Body => "### Body (Physical) ###",
            KnowledgeDomain::Heart => "### Heart (Emotional) ###",
            KnowledgeDomain::Soul => "### Soul (Ethical) ###",
        };
        format!("{}\n{}", domain_label, content)
    }

    /// Merge multiple domain results into a structured context
    pub fn synthesize_context(results: &[(KnowledgeDomain, String)]) -> String {
        let mut synthesized = String::new();
        let mut domain_contents: HashMap<KnowledgeDomain, Vec<String>> = HashMap::new();

        // Group results by domain
        for (domain, content) in results {
            domain_contents
                .entry(*domain)
                .or_insert_with(Vec::new)
                .push(content.clone());
        }

        // Output in order: Mind, Body, Heart, Soul
        for domain in [KnowledgeDomain::Mind, KnowledgeDomain::Body, KnowledgeDomain::Heart, KnowledgeDomain::Soul] {
            if let Some(contents) = domain_contents.get(&domain) {
                synthesized.push_str(&Self::label_content(domain, &contents.join("\n\n")));
                synthesized.push_str("\n\n");
            }
        }

        synthesized.trim().to_string()
    }
}

/// Persona-based domain weighting
/// Different personas may prioritize different domains
pub fn get_persona_domain_weights(
    persona_name: Option<&str>,
) -> HashMap<KnowledgeDomain, f64> {
    let mut weights = HashMap::new();
    
    // Default weights (all equal)
    for domain in KnowledgeDomain::all() {
        weights.insert(domain, 1.0);
    }

    // Adjust weights based on persona
    if let Some(name) = persona_name {
        let name_lower = name.to_lowercase();
        
        // "The Skeptic" prioritizes Soul (Governance) and Body (Telemetry)
        if name_lower.contains("skeptic") {
            *weights.get_mut(&KnowledgeDomain::Soul).unwrap() = 1.5;
            *weights.get_mut(&KnowledgeDomain::Body).unwrap() = 1.3;
            *weights.get_mut(&KnowledgeDomain::Mind).unwrap() = 0.8;
            *weights.get_mut(&KnowledgeDomain::Heart).unwrap() = 0.7;
        }
        
        // "The Architect" prioritizes Mind (Technical) and Soul (Governance)
        if name_lower.contains("architect") {
            *weights.get_mut(&KnowledgeDomain::Mind).unwrap() = 1.5;
            *weights.get_mut(&KnowledgeDomain::Soul).unwrap() = 1.2;
            *weights.get_mut(&KnowledgeDomain::Body).unwrap() = 0.9;
            *weights.get_mut(&KnowledgeDomain::Heart).unwrap() = 0.8;
        }
        
        // "The Speedster" prioritizes Body (Performance) and Mind (Efficiency)
        if name_lower.contains("speedster") {
            *weights.get_mut(&KnowledgeDomain::Body).unwrap() = 1.5;
            *weights.get_mut(&KnowledgeDomain::Mind).unwrap() = 1.3;
            *weights.get_mut(&KnowledgeDomain::Heart).unwrap() = 0.7;
            *weights.get_mut(&KnowledgeDomain::Soul).unwrap() = 0.8;
        }
    }

    weights
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_keywords_trigger_soul_domain() {
        let router = DomainRouter::new();
        
        // Test security-related keywords
        let security_queries = vec![
            "security breach detected",
            "dangerous vulnerability found",
            "safety compliance check",
            "audit the system for risks",
            "threat assessment needed",
            "governance policy violation",
            "security alert triggered",
        ];
        
        for query in security_queries {
            let domains = router.infer_domains_from_query(query);
            assert!(
                domains.contains(&KnowledgeDomain::Soul),
                "Query '{}' should trigger Soul domain but got: {:?}",
                query,
                domains
            );
        }
    }

    #[test]
    fn test_danger_keywords_trigger_soul_domain() {
        let router = DomainRouter::new();
        
        // Test danger-related keywords
        let danger_queries = vec![
            "danger in the system",
            "dangerous operation detected",
            "security danger alert",
            "potential danger identified",
        ];
        
        for query in danger_queries {
            let domains = router.infer_domains_from_query(query);
            assert!(
                domains.contains(&KnowledgeDomain::Soul),
                "Query '{}' should trigger Soul domain but got: {:?}",
                query,
                domains
            );
        }
    }

    #[test]
    fn test_mind_domain_keywords() {
        let router = DomainRouter::new();
        
        let mind_queries = vec![
            "how to configure the system",
            "technical procedure for setup",
            "playbook for deployment",
            "tool specification",
        ];
        
        for query in mind_queries {
            let domains = router.infer_domains_from_query(query);
            assert!(
                domains.contains(&KnowledgeDomain::Mind),
                "Query '{}' should trigger Mind domain but got: {:?}",
                query,
                domains
            );
        }
    }

    #[test]
    fn test_body_domain_keywords() {
        let router = DomainRouter::new();
        
        let body_queries = vec![
            "system performance status",
            "hardware telemetry data",
            "cpu and memory usage",
            "agent logs analysis",
        ];
        
        for query in body_queries {
            let domains = router.infer_domains_from_query(query);
            assert!(
                domains.contains(&KnowledgeDomain::Body),
                "Query '{}' should trigger Body domain but got: {:?}",
                query,
                domains
            );
        }
    }

    #[test]
    fn test_heart_domain_keywords() {
        let router = DomainRouter::new();
        
        let heart_queries = vec![
            "user preferences",
            "agent persona settings",
            "personal interaction history",
        ];
        
        for query in heart_queries {
            let domains = router.infer_domains_from_query(query);
            assert!(
                domains.contains(&KnowledgeDomain::Heart),
                "Query '{}' should trigger Heart domain but got: {:?}",
                query,
                domains
            );
        }
    }

    #[test]
    fn test_domain_attribution_calculation() {
        // Test attribution from results with scores
        let results = vec![
            (KnowledgeDomain::Mind, 0.8),
            (KnowledgeDomain::Mind, 0.6),
            (KnowledgeDomain::Body, 0.4),
            (KnowledgeDomain::Soul, 0.9),
            (KnowledgeDomain::Soul, 0.7),
        ];
        
        let attribution = get_source_attribution(&results);
        
        // Total score: 0.8 + 0.6 + 0.4 + 0.9 + 0.7 = 3.4
        // Mind: (0.8 + 0.6) / 3.4 * 100 = 41.18%
        // Body: 0.4 / 3.4 * 100 = 11.76%
        // Soul: (0.9 + 0.7) / 3.4 * 100 = 47.06%
        // Heart: 0%
        
        assert!((attribution.mind - 41.18).abs() < 1.0, "Mind attribution should be ~41.18%, got {}", attribution.mind);
        assert!((attribution.body - 11.76).abs() < 1.0, "Body attribution should be ~11.76%, got {}", attribution.body);
        assert!((attribution.soul - 47.06).abs() < 1.0, "Soul attribution should be ~47.06%, got {}", attribution.soul);
        assert!(attribution.heart < 1.0, "Heart attribution should be ~0%, got {}", attribution.heart);
    }

    #[test]
    fn test_domain_attribution_from_counts() {
        let domains = vec![
            KnowledgeDomain::Mind,
            KnowledgeDomain::Mind,
            KnowledgeDomain::Body,
            KnowledgeDomain::Soul,
        ];
        
        let attribution = DomainAttribution::from_counts(&domains);
        
        // 4 total: Mind=2, Body=1, Soul=1, Heart=0
        assert!((attribution.mind - 50.0).abs() < 0.1, "Mind should be 50%, got {}", attribution.mind);
        assert!((attribution.body - 25.0).abs() < 0.1, "Body should be 25%, got {}", attribution.body);
        assert!((attribution.soul - 25.0).abs() < 0.1, "Soul should be 25%, got {}", attribution.soul);
        assert!(attribution.heart < 0.1, "Heart should be 0%, got {}", attribution.heart);
    }

    #[test]
    fn test_empty_attribution() {
        let results: Vec<(KnowledgeDomain, f64)> = vec![];
        let attribution = get_source_attribution(&results);
        
        assert_eq!(attribution.mind, 0.0);
        assert_eq!(attribution.body, 0.0);
        assert_eq!(attribution.heart, 0.0);
        assert_eq!(attribution.soul, 0.0);
    }
}

/// Domain attribution result showing contribution percentages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainAttribution {
    /// Percentage contribution from Mind domain (0.0-100.0)
    pub mind: f64,
    /// Percentage contribution from Body domain (0.0-100.0)
    pub body: f64,
    /// Percentage contribution from Heart domain (0.0-100.0)
    pub heart: f64,
    /// Percentage contribution from Soul domain (0.0-100.0)
    pub soul: f64,
}

impl DomainAttribution {
    /// Create attribution from domain results with scores
    pub fn from_results(results: &[(KnowledgeDomain, f64)]) -> Self {
        let mut total_score = 0.0;
        let mut domain_scores: HashMap<KnowledgeDomain, f64> = HashMap::new();

        // Sum scores by domain
        for (domain, score) in results {
            *domain_scores.entry(*domain).or_insert(0.0) += score;
            total_score += score;
        }

        // Calculate percentages
        let mind = if total_score > 0.0 {
            domain_scores.get(&KnowledgeDomain::Mind).copied().unwrap_or(0.0) / total_score * 100.0
        } else {
            0.0
        };

        let body = if total_score > 0.0 {
            domain_scores.get(&KnowledgeDomain::Body).copied().unwrap_or(0.0) / total_score * 100.0
        } else {
            0.0
        };

        let heart = if total_score > 0.0 {
            domain_scores.get(&KnowledgeDomain::Heart).copied().unwrap_or(0.0) / total_score * 100.0
        } else {
            0.0
        };

        let soul = if total_score > 0.0 {
            domain_scores.get(&KnowledgeDomain::Soul).copied().unwrap_or(0.0) / total_score * 100.0
        } else {
            0.0
        };

        Self { mind, body, heart, soul }
    }

    /// Create attribution from domain counts (simple count-based)
    pub fn from_counts(results: &[KnowledgeDomain]) -> Self {
        let mut counts: HashMap<KnowledgeDomain, usize> = HashMap::new();
        for domain in results {
            *counts.entry(*domain).or_insert(0) += 1;
        }

        let total = results.len() as f64;
        if total == 0.0 {
            return Self { mind: 0.0, body: 0.0, heart: 0.0, soul: 0.0 };
        }

        Self {
            mind: (counts.get(&KnowledgeDomain::Mind).copied().unwrap_or(0) as f64 / total) * 100.0,
            body: (counts.get(&KnowledgeDomain::Body).copied().unwrap_or(0) as f64 / total) * 100.0,
            heart: (counts.get(&KnowledgeDomain::Heart).copied().unwrap_or(0) as f64 / total) * 100.0,
            soul: (counts.get(&KnowledgeDomain::Soul).copied().unwrap_or(0) as f64 / total) * 100.0,
        }
    }
}

/// Calculate domain attribution from query results
/// Returns a DomainAttribution struct with percentage contributions from each domain
pub fn get_source_attribution(
    results: &[(KnowledgeDomain, f64)],
) -> DomainAttribution {
    DomainAttribution::from_results(results)
}
