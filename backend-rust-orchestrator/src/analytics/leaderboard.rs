//! Leaderboard Engine - "Brain Analytics" for the Blue Flame Sovereign Loop
//!
//! This module implements the Economic & Visibility Layer that tracks agent ROI by:
//! 1. Querying Qdrant for successful task counts by agent_id
//! 2. Parsing Git commit history to count playbooks committed
//! 3. Calculating SovereignScore based on tasks, playbooks, and resource warnings
//! 4. Caching results for 5 minutes to avoid heavy queries on every UI refresh

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use chrono::Utc;
use git2::Repository;
use once_cell::sync::Lazy;
use qdrant_client::{
    qdrant::{ScrollPoints, Filter, Condition, FieldCondition, Match},
    Qdrant,
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{info, warn, error};

/// Cache entry with timestamp
struct CacheEntry {
    data: LeaderboardData,
    timestamp: Instant,
}

/// Global cache with 5-minute TTL
static CACHE: Lazy<Arc<RwLock<Option<CacheEntry>>>> = Lazy::new(|| Arc::new(RwLock::new(None)));

const CACHE_TTL_SECS: u64 = 300; // 5 minutes

/// Leaderboard data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderboardData {
    pub agents: Vec<AgentLeaderboardEntry>,
    pub generated_at: String,
}

/// Individual agent entry in the leaderboard
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentLeaderboardEntry {
    pub agent_id: String,
    pub agent_name: String,
    pub sovereign_score: i64,
    pub successful_tasks: u64,
    pub playbooks_committed: u64,
    pub resource_warnings: u64,
    pub badges: Vec<String>,
}

/// LeaderboardEngine - aggregates data from Qdrant and Git
pub struct LeaderboardEngine {
    qdrant_client: Arc<Qdrant>,
    git_repo_path: String,
}

impl LeaderboardEngine {
    /// Create a new LeaderboardEngine
    pub fn new(qdrant_client: Arc<Qdrant>, git_repo_path: String) -> Self {
        Self {
            qdrant_client,
            git_repo_path,
        }
    }

    /// Get leaderboard data (with caching)
    pub async fn get_leaderboard(&self) -> Result<LeaderboardData, String> {
        // Check cache first
        {
            let cache_guard = CACHE.read().await;
            if let Some(entry) = cache_guard.as_ref() {
                if entry.timestamp.elapsed().as_secs() < CACHE_TTL_SECS {
                    info!("Returning cached leaderboard data");
                    return Ok(entry.data.clone());
                }
            }
        }

        // Cache expired or missing, fetch fresh data
        info!("Fetching fresh leaderboard data");
        let data = self.fetch_leaderboard_data().await?;

        // Update cache
        {
            let mut cache_guard = CACHE.write().await;
            *cache_guard = Some(CacheEntry {
                data: data.clone(),
                timestamp: Instant::now(),
            });
        }

        Ok(data)
    }

    /// Fetch fresh leaderboard data from Qdrant and Git
    async fn fetch_leaderboard_data(&self) -> Result<LeaderboardData, String> {
        // 1. Query Qdrant for successful tasks by agent_id
        let task_counts = self.query_qdrant_successful_tasks().await?;
        
        // 2. Query Qdrant for resource warnings by agent_id
        let warning_counts = self.query_qdrant_resource_warnings().await?;
        
        // 3. Parse Git commits to count playbooks by agent name
        let commit_counts = self.query_git_commits().await?;

        // 4. Combine data and calculate scores
        let mut agent_map: HashMap<String, AgentLeaderboardEntry> = HashMap::new();

        // Initialize from task counts
        for (agent_id, count) in task_counts {
            agent_map.insert(agent_id.clone(), AgentLeaderboardEntry {
                agent_id: agent_id.clone(),
                agent_name: agent_id.clone(), // Will be updated if we find name in commits
                sovereign_score: 0,
                successful_tasks: count,
                playbooks_committed: 0,
                resource_warnings: *warning_counts.get(&agent_id).unwrap_or(&0),
                badges: Vec::new(),
            });
        }

        // Add agents from commits that might not be in task counts
        for (agent_name, commit_count) in commit_counts {
            // Try to match agent_name to agent_id (this is approximate)
            // In a real system, you'd have a mapping table
            let agent_id = agent_name.clone(); // Simplified: assume name == id for now
            let entry = agent_map.entry(agent_id.clone()).or_insert_with(|| {
                AgentLeaderboardEntry {
                    agent_id: agent_id.clone(),
                    agent_name: agent_name.clone(),
                    sovereign_score: 0,
                    successful_tasks: 0,
                    playbooks_committed: 0,
                    resource_warnings: *warning_counts.get(&agent_id).unwrap_or(&0),
                    badges: Vec::new(),
                }
            });
            entry.agent_name = agent_name;
            entry.playbooks_committed = commit_count;
        }

        // Calculate scores and badges
        let mut agents: Vec<AgentLeaderboardEntry> = agent_map.into_values().collect();
        for agent in &mut agents {
            // Calculate SovereignScore: (Successful_Tasks * 10) + (Playbooks_Committed * 50) - (Resource_Warnings * 5)
            agent.sovereign_score = (agent.successful_tasks as i64 * 10)
                + (agent.playbooks_committed as i64 * 50)
                - (agent.resource_warnings as i64 * 5);

            // Assign badges
            agent.badges = self.calculate_badges(agent);
        }

        // Sort by score (descending)
        agents.sort_by(|a, b| b.sovereign_score.cmp(&a.sovereign_score));

        Ok(LeaderboardData {
            agents,
            generated_at: Utc::now().to_rfc3339(),
        })
    }

    /// Query Qdrant for successful task counts by agent_id
    async fn query_qdrant_successful_tasks(&self) -> Result<HashMap<String, u64>, String> {
        let collection_name = "agent_logs";
        let mut counts: HashMap<String, u64> = HashMap::new();

        // Build filter for outcome == "Success"
        let filter = Some(Filter {
            must: vec![Condition {
                condition_one_of: Some(
                    qdrant_client::qdrant::condition::ConditionOneOf::Field(
                        FieldCondition {
                            key: "outcome".to_string(),
                            r#match: Some(Match {
                                match_value: Some(qdrant_client::qdrant::r#match::MatchValue::Keyword(
                                    "Success".to_string(),
                                )),
                            }),
                            ..Default::default()
                        },
                    ),
                ),
            }],
            ..Default::default()
        });

        let scroll_request = ScrollPoints {
            collection_name: collection_name.to_string(),
            filter,
            limit: Some(10000), // Process in batches
            offset: None,
            with_payload: Some(true.into()),
            with_vectors: Some(false.into()),
            ..Default::default()
        };

        let mut offset = None;
        loop {
            let mut scroll_req = scroll_request.clone();
            if let Some(off) = offset {
                scroll_req.offset = Some(off);
            }

            match self.qdrant_client.scroll(scroll_req).await {
                Ok(response) => {
                    let points = response.result;
                    if points.is_empty() {
                        break;
                    }

                    for point in points {
                        let payload = &point.payload;
                        // Extract agent_id from metadata
                        if let Some(agent_id) = payload.get("agent_id") {
                            use qdrant_client::qdrant::value::Kind;
                            if let Some(Kind::StringValue(s)) = agent_id.kind.as_ref() {
                                *counts.entry(s.clone()).or_insert(0) += 1;
                            }
                        }
                    }

                    // Check if there are more points
                    if let Some(next_offset) = response.next_page_offset {
                        offset = Some(next_offset);
                    } else {
                        break;
                    }
                }
                Err(e) => {
                    warn!(error = %e, "Failed to query Qdrant for successful tasks");
                    // Return partial results rather than failing completely
                    break;
                }
            }
        }

        info!(count = counts.len(), "Queried successful tasks from Qdrant");
        Ok(counts)
    }

    /// Query Qdrant for resource warning counts by agent_id
    async fn query_qdrant_resource_warnings(&self) -> Result<HashMap<String, u64>, String> {
        // Query the episodic_memory collection for ResourceWarning events
        // This is a simplified approach - in production, you might have a dedicated events collection
        let collection_name = "episodic_memory";
        let mut counts: HashMap<String, u64> = HashMap::new();

        // Build filter for content containing "ResourceWarning" or metadata with resource_type
        // Note: This is approximate - ideally you'd have a dedicated events collection
        let filter = Some(Filter {
            must: vec![Condition {
                condition_one_of: Some(
                    qdrant_client::qdrant::condition::ConditionOneOf::Field(
                        FieldCondition {
                            key: "metadata.resource_type".to_string(),
                            r#match: Some(Match {
                                match_value: Some(qdrant_client::qdrant::r#match::MatchValue::Keyword(
                                    "resource_warning".to_string(),
                                )),
                            }),
                            ..Default::default()
                        },
                    ),
                ),
            }],
            ..Default::default()
        });

        let scroll_request = ScrollPoints {
            collection_name: collection_name.to_string(),
            filter,
            limit: Some(10000),
            offset: None,
            with_payload: Some(true.into()),
            with_vectors: Some(false.into()),
            ..Default::default()
        };

        match self.qdrant_client.scroll(scroll_request).await {
            Ok(response) => {
                for point in response.result {
                    let payload = &point.payload;
                    if let Some(agent_id) = payload.get("agent_id") {
                        use qdrant_client::qdrant::value::Kind;
                        if let Some(Kind::StringValue(s)) = agent_id.kind.as_ref() {
                            *counts.entry(s.clone()).or_insert(0) += 1;
                        }
                    }
                }
            }
            Err(e) => {
                warn!(error = %e, "Failed to query Qdrant for resource warnings");
                // Return empty map on error
            }
        }

        info!(count = counts.len(), "Queried resource warnings from Qdrant");
        Ok(counts)
    }

    /// Query Git repository for commit counts by agent name
    async fn query_git_commits(&self) -> Result<HashMap<String, u64>, String> {
        let repo_path = Path::new(&self.git_repo_path);
        
        // Run git operations in a blocking task
        let repo_path = repo_path.to_path_buf();
        let result: Result<HashMap<String, u64>, String> = tokio::task::spawn_blocking(
            move || -> Result<HashMap<String, u64>, String> {
            let repo = Repository::open(&repo_path)
                .map_err(|e| format!("Failed to open git repository at {}: {}", repo_path.display(), e))?;

            let mut revwalk = repo.revwalk()
                .map_err(|e| format!("Failed to create revwalk: {}", e))?;
            
            revwalk.push_head()
                .map_err(|e| format!("Failed to push HEAD: {}", e))?;

            let mut counts: HashMap<String, u64> = HashMap::new();

            for oid in revwalk {
                let oid = oid.map_err(|e| format!("Failed to get commit OID: {}", e))?;
                let commit = repo.find_commit(oid)
                    .map_err(|e| format!("Failed to find commit: {}", e))?;

                // Get author name
                let author = commit.author();
                let author_name = author.name().unwrap_or("unknown");

                // Check if commit message contains "playbook" (indicating a playbook commit)
                let message = commit.message().unwrap_or("");
                if message.to_lowercase().contains("playbook") {
                    *counts.entry(author_name.to_string()).or_insert(0) += 1;
                }
            }

            Ok(counts)
        },
        )
        .await
        .map_err(|e| format!("Task join error: {}", e))?;

        match result {
            Ok(counts) => {
                info!(count = counts.len(), "Queried git commits");
                Ok(counts)
            }
            Err(e) => {
                warn!(error = %e, "Failed to query git commits");
                // Return empty map on error rather than failing completely
                Ok(HashMap::new())
            }
        }
    }

    /// Calculate badges for an agent based on their metrics
    fn calculate_badges(&self, agent: &AgentLeaderboardEntry) -> Vec<String> {
        let mut badges = Vec::new();

        // Top Architect: High playbook commits
        if agent.playbooks_committed >= 10 {
            badges.push("Top Architect".to_string());
        }

        // Security First: Low resource warnings relative to tasks
        if agent.resource_warnings == 0 && agent.successful_tasks > 0 {
            badges.push("Security First".to_string());
        }

        // High Performer: High sovereign score
        if agent.sovereign_score >= 500 {
            badges.push("High Performer".to_string());
        }

        // Efficiency Master: High tasks with low warnings
        if agent.successful_tasks >= 50 && agent.resource_warnings < 5 {
            badges.push("Efficiency Master".to_string());
        }

        badges
    }
}
