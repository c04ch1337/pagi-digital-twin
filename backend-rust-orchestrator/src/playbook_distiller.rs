use std::collections::HashMap;
use std::path::PathBuf;
use chrono::{DateTime, Utc};
use tonic::transport::Channel;
use tracing::{info, warn, error};
use regex::Regex;
use crate::memory_client::{
    memory_service_client::MemoryServiceClient,
    ListMemoriesRequest, MemoryResult,
};
use crate::tools::git::GitOperations;
use std::sync::Arc;
use crate::foundry::compliance_monitor::ComplianceMonitor;

/// Playbook distillation service that analyzes episodic memory and generates Markdown playbooks
pub struct PlaybookDistiller {
    memory_client: MemoryServiceClient<Channel>,
    playbooks_dir: PathBuf,
    repo_path: PathBuf,
    http_client: reqwest::Client,
    openrouter_url: String,
    openrouter_api_key: String,
    openrouter_model: String,
    privacy_filter_enabled: bool,
    compliance_monitor: Option<Arc<ComplianceMonitor>>,
}

#[derive(Debug, Clone)]
struct EpisodicMemory {
    id: String,
    timestamp: String,
    agent_id: String,
    agent_name: String,
    task: String,
    outcome: String,
    result: String,
}

impl PlaybookDistiller {
    pub fn new(
        memory_client: MemoryServiceClient<Channel>,
        playbooks_dir: PathBuf,
        repo_path: PathBuf,
        http_client: reqwest::Client,
        openrouter_url: String,
        openrouter_api_key: String,
        openrouter_model: String,
        privacy_filter_enabled: bool,
    ) -> Self {
        Self {
            memory_client,
            playbooks_dir,
            repo_path,
            http_client,
            openrouter_url,
            openrouter_api_key,
            openrouter_model,
            privacy_filter_enabled,
            compliance_monitor: None,
        }
    }

    /// Set the compliance monitor for compliance-gated pushes
    pub fn set_compliance_monitor(&mut self, compliance_monitor: Arc<ComplianceMonitor>) {
        self.compliance_monitor = Some(compliance_monitor);
    }

    /// Analyze episodic memories from the last week and generate playbooks
    /// Automatically commits and pushes to GitHub after generation
    pub async fn distill_playbooks(&self) -> Result<Vec<String>, String> {
        info!("Starting playbook distillation from episodic memory");

        // Query episodic memories from the last week
        let memories = self.fetch_recent_episodic_memories().await?;
        
        if memories.is_empty() {
            info!("No episodic memories found for playbook generation");
            return Ok(vec![]);
        }

        info!(
            total_memories = memories.len(),
            "Fetched episodic memories for analysis"
        );

        // Filter to only successful completions for playbook generation
        let successful_memories: Vec<&EpisodicMemory> = memories
            .iter()
            .filter(|m| m.outcome == "Success")
            .collect();

        if successful_memories.is_empty() {
            info!("No successful task completions found for playbook generation");
            return Ok(vec![]);
        }

        // Group memories by task type/pattern
        let grouped = self.group_memories_by_pattern(&memories);
        
        // Generate playbooks for each group
        let mut generated_playbooks = Vec::new();
        for (pattern, group_memories) in grouped {
            if let Ok(playbook_name) = self.generate_playbook(&pattern, &group_memories).await {
                generated_playbooks.push(playbook_name);
            }
        }

        if !generated_playbooks.is_empty() {
            // Check compliance before pushing (90%+ threshold for last 5 missions)
            let should_push = if let Some(ref compliance_monitor) = self.compliance_monitor {
                self.check_compliance_before_push(&memories, compliance_monitor).await
            } else {
                warn!("Compliance monitor not set - allowing push without compliance check");
                true
            };

            if !should_push {
                warn!(
                    "Playbook push blocked: One or more agents have compliance score below 90% in last 5 missions"
                );
                return Ok(generated_playbooks);
            }

            // Automatically commit and push to GitHub
            info!("Committing and pushing playbooks to GitHub");
            let commit_message = format!(
                "AI generated playbook update: {} playbook(s) from {} episodic memories",
                generated_playbooks.len(),
                memories.len()
            );

            if let Err(e) = GitOperations::commit_and_push_playbooks(
                &self.repo_path,
                &self.playbooks_dir,
                &commit_message,
                "origin",
                "main",
                "Orchestrator",
                "orchestrator@digital-twin.local",
            ).await {
                warn!(
                    error = %e,
                    "Failed to push playbooks to GitHub (playbooks were still generated locally)"
                );
            } else {
                info!(
                    playbooks_count = generated_playbooks.len(),
                    "Successfully pushed playbooks to GitHub"
                );
            }
        }

        info!(
            playbooks_generated = generated_playbooks.len(),
            "Playbook distillation completed"
        );

        Ok(generated_playbooks)
    }

    /// Fetch episodic memories from the last week (or all if testing)
    pub async fn fetch_recent_episodic_memories(&self) -> Result<Vec<EpisodicMemory>, String> {
        let request = tonic::Request::new(ListMemoriesRequest {
            namespace: "episodic_memory".to_string(),
            page: 1,
            page_size: 1000, // Get a large batch
            twin_id: "orchestrator".to_string(),
        });

        let mut memory_client = self.memory_client.clone();
        let response = memory_client
            .list_memories(request)
            .await
            .map_err(|e| format!("Failed to query episodic memory: {}", e))?;

        let resp = response.into_inner();
        let now = Utc::now();
        let week_ago = now - chrono::Duration::days(7);

        let mut recent_memories = Vec::new();
        for memory in resp.memories {
            // Parse timestamp
            if let Ok(timestamp) = DateTime::parse_from_rfc3339(&memory.timestamp) {
                if timestamp >= week_ago {
                    // Extract metadata
                    let agent_id = memory.metadata.get("agent_id")
                        .cloned()
                        .unwrap_or_default();
                    let agent_name = memory.metadata.get("agent_name")
                        .cloned()
                        .unwrap_or_default();
                    let outcome = memory.metadata.get("outcome")
                        .cloned()
                        .unwrap_or_default();
                    let task = memory.metadata.get("task")
                        .cloned()
                        .unwrap_or_default();

                    recent_memories.push(EpisodicMemory {
                        id: memory.id,
                        timestamp: memory.timestamp,
                        agent_id,
                        agent_name,
                        task,
                        outcome,
                        result: memory.content,
                    });
                }
            }
        }

        // Sort by timestamp (newest first)
        recent_memories.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        Ok(recent_memories)
    }

    /// Group memories by task pattern (simplified: group by task prefix/keywords)
    fn group_memories_by_pattern(
        &self,
        memories: &[EpisodicMemory],
    ) -> HashMap<String, Vec<EpisodicMemory>> {
        let mut groups: HashMap<String, Vec<EpisodicMemory>> = HashMap::new();

        for memory in memories {
            // Extract a pattern from the task (simplified: use first few words)
            let pattern = self.extract_pattern(&memory.task);
            groups.entry(pattern).or_insert_with(Vec::new).push(memory.clone());
        }

        groups
    }

    /// Extract a pattern/keyword from a task description
    fn extract_pattern(&self, task: &str) -> String {
        // Simple pattern extraction: use first significant words
        let words: Vec<&str> = task
            .split_whitespace()
            .take(3)
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
            .filter(|w| !w.is_empty())
            .collect();
        
        if words.is_empty() {
            "general".to_string()
        } else {
            words.join("_").to_lowercase()
        }
    }

    /// Generate a Markdown playbook from a group of memories using LLM distillation
    async fn generate_playbook(
        &self,
        pattern: &str,
        memories: &[EpisodicMemory],
    ) -> Result<String, String> {
        // Separate successes and failures
        let successes: Vec<&EpisodicMemory> = memories
            .iter()
            .filter(|m| m.outcome == "Success")
            .collect();
        let failures: Vec<&EpisodicMemory> = memories
            .iter()
            .filter(|m| m.outcome == "Failure")
            .collect();

        if successes.is_empty() && failures.is_empty() {
            return Err("No memories to generate playbook from".to_string());
        }

        // Prepare logs for LLM distillation in Thought/Action/Observation format
        let mut logs_text = String::new();
        logs_text.push_str("## Successful Task Trajectories (Thought/Action/Observation)\n\n");
        for success in successes.iter().take(20) {
            logs_text.push_str(&format!(
                "**Agent:** {}\n**Task:** {}\n\n**Thought/Action/Observation Trajectory:**\n{}\n\n---\n\n",
                success.agent_name, success.task, success.result
            ));
        }
        
        if !failures.is_empty() {
            logs_text.push_str("\n## Failed Task Attempts (Lessons Learned)\n\n");
            for failure in failures.iter().take(10) {
                logs_text.push_str(&format!(
                    "**Agent:** {}\n**Task:** {}\n\n**Failed Trajectory:**\n{}\n\n---\n\n",
                    failure.agent_name, failure.task, failure.result
                ));
            }
        }

        // Apply privacy filter if enabled (uses placeholders instead of redaction)
        let filtered_logs = if self.privacy_filter_enabled {
            self.apply_privacy_filter_with_placeholders(&logs_text)
        } else {
            logs_text
        };

        // Extract task name from pattern
        let task_name = pattern.replace('_', " ");

        // Distillation prompt for LLM - Blue Flame Knowledge Architect
        let distillation_prompt = format!(
            "You are the 'Blue Flame' Knowledge Architect. Analyze the provided successful task trajectories (Thought/Action/Observation) and distill them into a structured Phoenix Playbook.\n\n\
            **Mission:**\n\
            Transform these agent execution logs into a reusable, structured playbook that future agents can follow.\n\n\
            **Requirements:**\n\
            - Format: Output valid Markdown\n\
            - Structure: Follow the exact format below\n\
            - Anonymity: Ensure no specific IPs, PIDs, or local file paths are included. Use placeholders like <LOCAL_IP> or <TARGET_PID>\n\n\
            **Required Structure:**\n\
            ```markdown\n\
            # {} Playbook\n\n\
            ## Objective: What was solved?\n\n\
            [Clear description of what problem this playbook solves]\n\n\
            ## Step-by-Step: The optimal sequence of tools used\n\n\
            [Numbered steps showing the optimal approach based on successful completions]\n\n\
            ## Pitfalls: What didn't work and why\n\n\
            [Document failures and why they occurred, based on failed attempts]\n\n\
            ## Ferrellgas Context: How this task aligns with 'People-First' or 'IT-Strategic' values\n\n\
            [Explain how this task supports Ferrellgas values: People First, Strategic IT, Family Values, or Leadership Awareness]\n\
            ```\n\n\
            **Task Pattern:** {}\n\
            **Total Memories:** {} ({} successes, {} failures)\n\n\
            **Task Trajectories:**\n\n{}",
            task_name,
            pattern.replace('_', " "),
            memories.len(),
            successes.len(),
            failures.len(),
            filtered_logs
        );

        // Call LLM for distillation
        let distilled_content = self.distill_with_llm(&distillation_prompt).await?;

        // Generate playbook name
        let playbook_name = format!("{}.md", pattern);
        let playbook_path = self.playbooks_dir.join(&playbook_name);

        // The LLM output should already contain the full playbook structure
        // Add metadata header with YAML frontmatter including compliance scores
        let mut content = String::new();
        content.push_str("---\n");
        content.push_str(&format!("generated: {}\n", Utc::now().format("%Y-%m-%d %H:%M:%S UTC")));
        content.push_str(&format!("source_memories: {}\n", memories.len()));
        content.push_str(&format!("successes: {}\n", successes.len()));
        content.push_str(&format!("failures: {}\n", failures.len()));
        content.push_str(&format!("success_rate: {:.1}%\n", 
            (successes.len() as f64 / memories.len() as f64) * 100.0));
        
        // Add compliance scores for agents involved
        if let Some(ref compliance_monitor) = self.compliance_monitor {
            let agent_ids: std::collections::HashSet<String> = memories
                .iter()
                .map(|m| m.agent_id.clone())
                .collect();
            
            let mut compliance_scores = Vec::new();
            for agent_id in agent_ids {
                let history = compliance_monitor.get_agent_history(&agent_id).await;
                let recent_missions: Vec<_> = history.iter().rev().take(5).collect();
                if !recent_missions.is_empty() {
                    let avg_score: f64 = recent_missions
                        .iter()
                        .map(|r| r.score)
                        .sum::<f64>() / recent_missions.len() as f64;
                    compliance_scores.push(format!("  {}: {:.1}%", agent_id, avg_score));
                }
            }
            
            if !compliance_scores.is_empty() {
                content.push_str("compliance_scores:\n");
                content.push_str(&compliance_scores.join("\n"));
                content.push_str("\n");
            }
        }
        
        content.push_str("distilled_by: Blue Flame Knowledge Architect\n");
        content.push_str("---\n\n");
        content.push_str(&distilled_content);

        // Write playbook file
        tokio::fs::create_dir_all(&self.playbooks_dir)
            .await
            .map_err(|e| format!("Failed to create playbooks directory: {}", e))?;

        tokio::fs::write(&playbook_path, &content)
            .await
            .map_err(|e| format!("Failed to write playbook file: {}", e))?;

        info!(
            playbook = %playbook_name,
            path = %playbook_path.display(),
            "Generated playbook using LLM distillation"
        );

        Ok(playbook_name)
    }

    /// Check compliance scores for all agents involved in memories
    /// Returns true if all agents have 90%+ compliance in last 5 missions
    async fn check_compliance_before_push(
        &self,
        memories: &[EpisodicMemory],
        compliance_monitor: &ComplianceMonitor,
    ) -> bool {
        // Get unique agent IDs from memories
        let agent_ids: std::collections::HashSet<String> = memories
            .iter()
            .map(|m| m.agent_id.clone())
            .collect();

        if agent_ids.is_empty() {
            warn!("No agent IDs found in memories - allowing push");
            return true;
        }

        info!(
            agent_count = agent_ids.len(),
            "Checking compliance scores for agents before playbook push"
        );

        // Check each agent's compliance
        for agent_id in agent_ids {
            let history = compliance_monitor.get_agent_history(&agent_id).await;
            
            // Get last 5 missions
            let recent_missions: Vec<_> = history
                .iter()
                .rev()
                .take(5)
                .collect();

            if recent_missions.is_empty() {
                warn!(
                    agent_id = %agent_id,
                    "No compliance history found for agent - allowing push"
                );
                continue;
            }

            // Calculate average compliance score for last 5 missions
            let avg_score: f64 = recent_missions
                .iter()
                .map(|r| r.score)
                .sum::<f64>() / recent_missions.len() as f64;

            if avg_score < 90.0 {
                warn!(
                    agent_id = %agent_id,
                    avg_score = avg_score,
                    "Agent compliance score below 90% threshold - blocking push"
                );
                return false;
            }

            info!(
                agent_id = %agent_id,
                avg_score = avg_score,
                "Agent compliance check passed"
            );
        }

        info!("All agents passed compliance check (90%+ threshold)");
        true
    }

    /// Distill logs into playbook using LLM
    async fn distill_with_llm(&self, prompt: &str) -> Result<String, String> {
        let system_prompt = "You are the 'Blue Flame' Knowledge Architect, a sophisticated AI that transforms agent execution logs into structured, reusable playbooks. Your role is to analyze Thought/Action/Observation trajectories and distill them into clear, actionable Markdown playbooks that align with Ferrellgas organizational values. Output only the Markdown playbook content following the exact structure specified, with no additional commentary or explanations outside the playbook format.";

        let payload = serde_json::json!({
            "model": self.openrouter_model,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": prompt}
            ],
            "temperature": 0.2  // Lower temperature for more structured, consistent output
        });

        let response = self.http_client
            .post(&self.openrouter_url)
            .header("Authorization", format!("Bearer {}", self.openrouter_api_key))
            .header("Content-Type", "application/json")
            .header("HTTP-Referer", "ferrellgas-agi-digital-twin")
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("OpenRouter API request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(format!(
                "OpenRouter API returned error status {}: {}",
                status, error_text
            ));
        }

        let api_response: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse OpenRouter response: {}", e))?;

        // Extract content from response
        let choice0 = api_response
            .get("choices")
            .and_then(|choices| choices.as_array())
            .and_then(|arr| arr.first());

        if let Some(content) = choice0
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|v| v.as_str())
        {
            Ok(content.to_string())
        } else {
            Err("Failed to extract content from LLM response".to_string())
        }
    }

    /// Apply privacy filter to redact sensitive information
    fn apply_privacy_filter(&self, text: &str) -> String {
        let mut filtered = text.to_string();

        // Redact IP addresses (IPv4)
        let ip_regex = Regex::new(r"\b(?:\d{1,3}\.){3}\d{1,3}\b").unwrap();
        filtered = ip_regex.replace_all(&filtered, "[REDACTED_IP]").to_string();

        // Redact common username patterns
        let username_regex = Regex::new(r"\b(?:[A-Z][a-z]+[A-Z][a-z]+|jameymilner|ferrellgas)\b").unwrap();
        filtered = username_regex.replace_all(&filtered, "[REDACTED_USER]").to_string();

        // Redact email addresses
        let email_regex = Regex::new(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b").unwrap();
        filtered = email_regex.replace_all(&filtered, "[REDACTED_EMAIL]").to_string();

        // Redact file paths that might contain usernames
        let path_regex = Regex::new(r"(?:/home/|C:\\Users\\)[A-Za-z0-9_\\-]+").unwrap();
        filtered = path_regex.replace_all(&filtered, "[REDACTED_PATH]").to_string();

        // Redact internal server names (common patterns)
        let server_regex = Regex::new(r"\b(?:[a-z0-9-]+\.(?:ferrellgas|internal|local))\b").unwrap();
        filtered = server_regex.replace_all(&filtered, "[REDACTED_SERVER]").to_string();

        filtered
    }

    /// Apply privacy filter with placeholders (for playbook generation)
    /// Uses placeholders like <LOCAL_IP> and <TARGET_PID> instead of redaction tags
    fn apply_privacy_filter_with_placeholders(&self, text: &str) -> String {
        let mut filtered = text.to_string();

        // Replace IP addresses with placeholder
        let ip_regex = Regex::new(r"\b(?:\d{1,3}\.){3}\d{1,3}\b").unwrap();
        filtered = ip_regex.replace_all(&filtered, "<LOCAL_IP>").to_string();

        // Replace PIDs with placeholder (common patterns: "PID: 1234", "pid=1234", etc.)
        let pid_regex = Regex::new(r"\b(?:PID|pid)[\s:=]+(\d+)\b").unwrap();
        filtered = pid_regex.replace_all(&filtered, "PID: <TARGET_PID>").to_string();
        
        // Also catch standalone PIDs in common contexts
        let standalone_pid_regex = Regex::new(r"\b(?:process|Process)\s+(\d+)\b").unwrap();
        filtered = standalone_pid_regex.replace_all(&filtered, "process <TARGET_PID>").to_string();

        // Replace usernames with placeholder
        let username_regex = Regex::new(r"\b(?:[A-Z][a-z]+[A-Z][a-z]+|jameymilner)\b").unwrap();
        filtered = username_regex.replace_all(&filtered, "<USERNAME>").to_string();

        // Replace email addresses with placeholder
        let email_regex = Regex::new(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b").unwrap();
        filtered = email_regex.replace_all(&filtered, "<EMAIL>").to_string();

        // Replace file paths with placeholders
        let path_regex = Regex::new(r"(?:/home/|C:\\Users\\)[A-Za-z0-9_\\-]+").unwrap();
        filtered = path_regex.replace_all(&filtered, "<USER_HOME>").to_string();
        
        // Replace specific file paths with generic placeholders
        let specific_path_regex = Regex::new(r"(?:/var/log/|/etc/|C:\\Program Files\\)[A-Za-z0-9_\\/.-]+").unwrap();
        filtered = specific_path_regex.replace_all(&filtered, "<SYSTEM_PATH>").to_string();

        // Replace internal server names with placeholder
        let server_regex = Regex::new(r"\b(?:[a-z0-9-]+\.(?:ferrellgas|internal|local))\b").unwrap();
        filtered = server_regex.replace_all(&filtered, "<INTERNAL_SERVER>").to_string();

        // Replace hostnames with placeholder
        let hostname_regex = Regex::new(r"\b(?:[a-z0-9-]+\.(?:com|net|org|local))\b").unwrap();
        filtered = hostname_regex.replace_all(&filtered, "<HOSTNAME>").to_string();

        filtered
    }

    /// Get the playbooks directory path
    pub fn playbooks_dir(&self) -> &PathBuf {
        &self.playbooks_dir
    }

    /// Get the repository path
    pub fn repo_path(&self) -> &PathBuf {
        &self.repo_path
    }
}
