use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, Mutex, RwLock};
use tracing::{error, info, warn};
use uuid::Uuid;
use sysinfo::{System, Pid};
use tonic::transport::Channel;

/// Hard guardrail: maximum number of concurrently active sub-agents.
///
/// Override via `ORCHESTRATOR_MAX_AGENTS`.
pub const DEFAULT_MAX_AGENTS: usize = 3;

/// Sub-agent runtime model: each agent runs as an in-process Tokio task.
///
/// NOTE: This is intentionally lightweight. If you later want OS-level isolation,
/// you can swap the worker loop to spawn a separate process/container.
pub struct AgentFactory {
    inner: Arc<RwLock<AgentRegistry>>,
    max_agents: usize,
    http_client: reqwest::Client,
    openrouter_url: String,
    openrouter_api_key: String,
    /// Model used for sub-agents (e.g., Gemini Flash).
    subagent_model: String,
    /// Message bus sender for publishing events.
    message_bus_tx: broadcast::Sender<crate::bus::PhoenixEvent>,
    /// Memory client for episodic memory logging
    memory_client: Option<Arc<Mutex<crate::memory_client::memory_service_client::MemoryServiceClient<Channel>>>>,
}

#[derive(Default)]
struct AgentRegistry {
    agents: HashMap<String, AgentHandle>,
}

struct AgentHandle {
    info: AgentInfo,
    task_tx: mpsc::Sender<AgentTask>,
    last_report: Arc<RwLock<Option<AgentReport>>>,
    logs: Arc<RwLock<Vec<String>>>,
    join: tokio::task::JoinHandle<()>,
    /// Track last activity timestamp for resource monitoring
    last_activity: Arc<RwLock<chrono::DateTime<chrono::Utc>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub agent_id: String,
    pub name: String,
    pub mission: String,
    pub permissions: Vec<String>,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentReport {
    pub agent_id: String,
    pub task: String,
    pub report: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSpawnResult {
    pub agent_id: String,
    pub status: String,
}

#[derive(Debug, Clone)]
struct AgentTask {
    task: String,
}

impl AgentFactory {
    pub fn new(
        http_client: reqwest::Client,
        openrouter_url: String,
        openrouter_api_key: String,
        subagent_model: String,
        max_agents: usize,
        message_bus_tx: broadcast::Sender<crate::bus::PhoenixEvent>,
        memory_client: Option<crate::memory_client::memory_service_client::MemoryServiceClient<Channel>>,
    ) -> Self {
        let factory = Self {
            inner: Arc::new(RwLock::new(AgentRegistry::default())),
            max_agents,
            http_client,
            openrouter_url,
            openrouter_api_key,
            subagent_model,
            message_bus_tx: message_bus_tx.clone(),
            memory_client: memory_client.map(|c| Arc::new(Mutex::new(c))),
        };

        // Start the resource monitor hook
        factory.start_monitor_hook();

        factory
    }

    /// Start the resource monitor hook that runs every 5 seconds.
    fn start_monitor_hook(&self) {
        let registry = Arc::clone(&self.inner);
        let bus_tx = self.message_bus_tx.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
            let mut system = System::new_all();
            let current_pid = Pid::from_u32(std::process::id());

            loop {
                interval.tick().await;
                
                // Refresh system information
                system.refresh_all();

                let guard = registry.read().await;

                // Get current process resource usage
                if let Some(proc) = system.process(current_pid) {
                    let memory_bytes = proc.memory();
                    let memory_mb = memory_bytes / 1024 / 1024; // Convert to MB
                    let cpu_percent = proc.cpu_usage();

                    // Check thresholds: 500MB RAM or 20% CPU
                    const MEMORY_THRESHOLD_MB: u64 = 500;
                    const CPU_THRESHOLD_PERCENT: f32 = 20.0;

                    let memory_exceeded = memory_mb > MEMORY_THRESHOLD_MB;
                    let cpu_exceeded = cpu_percent > CPU_THRESHOLD_PERCENT;

                    if memory_exceeded || cpu_exceeded {
                        // Publish warnings for all active agents
                        for (agent_id, handle) in guard.agents.iter() {
                            let resource_type = if memory_exceeded && cpu_exceeded {
                                "memory_and_cpu"
                            } else if memory_exceeded {
                                "memory"
                            } else {
                                "cpu"
                            };

                            let current_value = if memory_exceeded && cpu_exceeded {
                                format!("{}MB RAM, {:.1}% CPU", memory_mb, cpu_percent)
                            } else if memory_exceeded {
                                format!("{}MB", memory_mb)
                            } else {
                                format!("{:.1}%", cpu_percent)
                            };

                            let threshold = if memory_exceeded && cpu_exceeded {
                                format!("{}MB RAM, {}% CPU", MEMORY_THRESHOLD_MB, CPU_THRESHOLD_PERCENT)
                            } else if memory_exceeded {
                                format!("{}MB", MEMORY_THRESHOLD_MB)
                            } else {
                                format!("{}%", CPU_THRESHOLD_PERCENT)
                            };

                            let event = crate::bus::PhoenixEvent::ResourceWarning {
                                agent_id: agent_id.clone(),
                                agent_name: handle.info.name.clone(),
                                resource_type: resource_type.to_string(),
                                current_value,
                                threshold,
                                timestamp: chrono::Utc::now().to_rfc3339(),
                            };

                            if let Err(e) = bus_tx.send(event) {
                                warn!("Failed to publish ResourceWarning: {}", e);
                            } else {
                                warn!(
                                    agent_id = %agent_id,
                                    agent_name = %handle.info.name,
                                    resource_type = %resource_type,
                                    "Resource warning published - triggering HITL prompt"
                                );
                            }
                        }

                        // Trigger HITL (Human-in-the-Loop) prompt
                        // This would typically send a notification to the UI
                        // For now, we log it and the UI can subscribe to ResourceWarning events
                        if memory_exceeded || cpu_exceeded {
                            warn!(
                                memory_mb = memory_mb,
                                cpu_percent = cpu_percent,
                                "HITL: Resource limits exceeded. Consider terminating agents."
                            );
                        }
                    }
                }
            }
        });
    }

    pub fn max_agents(&self) -> usize {
        self.max_agents
    }

    pub async fn list_agents(&self) -> Vec<AgentInfo> {
        let guard = self.inner.read().await;
        guard
            .agents
            .values()
            .map(|h| h.info.clone())
            .collect()
    }

    pub async fn get_logs(&self, agent_id: &str) -> Result<Vec<String>, String> {
        let logs = {
            let guard = self.inner.read().await;
            let h = guard
                .agents
                .get(agent_id)
                .ok_or_else(|| "agent_id not found".to_string())?;
            Arc::clone(&h.logs)
        };
        let out = logs.read().await.clone();
        Ok(out)
    }

    pub async fn spawn_agent(
        &self,
        name: String,
        mission: String,
        permissions: Vec<String>,
        inherited_system_prompt: String,
    ) -> Result<AgentSpawnResult, String> {
        let name = name.trim().to_string();
        let mission = mission.trim().to_string();
        if name.is_empty() {
            return Err("name must not be empty".to_string());
        }
        if mission.is_empty() {
            return Err("mission must not be empty".to_string());
        }

        let mut guard = self.inner.write().await;
        if guard.agents.len() >= self.max_agents {
            return Err(format!(
                "agent quota exceeded (max_agents={}); refuse spawn",
                self.max_agents
            ));
        }

        let agent_id = Uuid::new_v4().to_string();
        let created_at = chrono::Utc::now().to_rfc3339();

        let (task_tx, mut task_rx) = mpsc::channel::<AgentTask>(32);
        let last_report: Arc<RwLock<Option<AgentReport>>> = Arc::new(RwLock::new(None));
        let logs: Arc<RwLock<Vec<String>>> = Arc::new(RwLock::new(Vec::new()));
        let last_activity: Arc<RwLock<chrono::DateTime<chrono::Utc>>> = 
            Arc::new(RwLock::new(chrono::Utc::now()));
        
        let info = AgentInfo {
            agent_id: agent_id.clone(),
            name,
            mission,
            permissions,
            status: "idle".to_string(),
            created_at,
        };

        // Publish AgentHandshake event
        let bus_tx = self.message_bus_tx.clone();
        let handshake_event = crate::bus::PhoenixEvent::AgentHandshake {
            agent_id: agent_id.clone(),
            agent_name: info.name.clone(),
            mission: info.mission.clone(),
            timestamp: info.created_at.clone(),
        };
        let _ = bus_tx.send(handshake_event);

        // Clone what the worker loop needs.
        let http_client = self.http_client.clone();
        let openrouter_url = self.openrouter_url.clone();
        let openrouter_api_key = self.openrouter_api_key.clone();
        let model = self.subagent_model.clone();
        let agent_info = info.clone();
        let report_slot = Arc::clone(&last_report);
        let log_slot = Arc::clone(&logs);
        let activity_slot = Arc::clone(&last_activity);
        let bus_tx = self.message_bus_tx.clone();
        let memory_client = self.memory_client.clone();

        let join = tokio::spawn(async move {
            {
                let mut lg = log_slot.write().await;
                lg.push(format!(
                    "agent_boot: id={} name={} created_at={}",
                    agent_info.agent_id, agent_info.name, agent_info.created_at
                ));
                lg.push(format!("mission: {}", agent_info.mission));
                lg.push(format!("permissions: {:?}", agent_info.permissions));
            }

            while let Some(task) = task_rx.recv().await {
                let task_text = task.task;
                {
                    let mut lg = log_slot.write().await;
                    lg.push(format!("task_received: {}", task_text));
                }
                
                // Update activity timestamp
                {
                    let mut activity = activity_slot.write().await;
                    *activity = chrono::Utc::now();
                }

                // Publish TaskUpdate event
                let task_update = crate::bus::PhoenixEvent::TaskUpdate {
                    agent_id: agent_info.agent_id.clone(),
                    task: task_text.clone(),
                    status: "in_progress".to_string(),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                };
                let _ = bus_tx.send(task_update);

                let worker_system_prompt = build_subagent_system_prompt(
                    &inherited_system_prompt,
                    &agent_info,
                );

                let call_res = openrouter_chat(
                    &http_client,
                    &openrouter_url,
                    &openrouter_api_key,
                    &model,
                    &worker_system_prompt,
                    &task_text,
                    0.2,
                )
                .await;

                match call_res {
                    Ok(raw) => {
                        let created_at = chrono::Utc::now().to_rfc3339();
                        let rep = AgentReport {
                            agent_id: agent_info.agent_id.clone(),
                            task: task_text.clone(),
                            report: raw.clone(),
                            created_at: created_at.clone(),
                        };
                        {
                            let mut slot = report_slot.write().await;
                            *slot = Some(rep);
                        }
                        {
                            let mut lg = log_slot.write().await;
                            lg.push(format!("task_completed: bytes={}", raw.len()));
                        }
                        
                        // Log episodic memory: SUCCESS
                        if let Some(ref mem_client) = memory_client {
                            let _ = log_episodic_memory(
                                mem_client.clone(),
                                &agent_info.agent_id,
                                &agent_info.name,
                                &task_text,
                                &raw,
                                "Success",
                            ).await;
                        }
                        
                        // Publish TaskUpdate event with completed status
                        let task_update = crate::bus::PhoenixEvent::TaskUpdate {
                            agent_id: agent_info.agent_id.clone(),
                            task: task_text.clone(),
                            status: "completed".to_string(),
                            timestamp: created_at,
                        };
                        let _ = bus_tx.send(task_update);
                    }
                    Err(e) => {
                        error!(agent_id = %agent_info.agent_id, error = %e, "sub-agent task failed");
                        let created_at = chrono::Utc::now().to_rfc3339();
                        let rep = AgentReport {
                            agent_id: agent_info.agent_id.clone(),
                            task: task_text.clone(),
                            report: format!("ERROR: {e}"),
                            created_at,
                        };
                        {
                            let mut slot = report_slot.write().await;
                            *slot = Some(rep);
                        }
                        {
                            let mut lg = log_slot.write().await;
                            lg.push(format!("task_failed: {}", e));
                        }
                        
                        // Log episodic memory: FAILURE
                        if let Some(ref mem_client) = memory_client {
                            let _ = log_episodic_memory(
                                mem_client.clone(),
                                &agent_info.agent_id,
                                &agent_info.name,
                                &task_text,
                                &format!("ERROR: {e}"),
                                "Failure",
                            ).await;
                        }
                    }
                }
            }

            {
                let mut lg = log_slot.write().await;
                lg.push("agent_shutdown".to_string());
            }
        });

        guard.agents.insert(
            agent_id.clone(),
            AgentHandle {
                info,
                task_tx,
                last_report,
                logs,
                join,
                last_activity,
            },
        );

        Ok(AgentSpawnResult {
            agent_id,
            status: "spawned".to_string(),
        })
    }

    pub async fn post_task(&self, agent_id: &str, data: String) -> Result<(), String> {
        let data = data.trim().to_string();
        if data.is_empty() {
            return Err("task data must not be empty".to_string());
        }

        let guard = self.inner.read().await;
        let h = guard
            .agents
            .get(agent_id)
            .ok_or_else(|| "agent_id not found".to_string())?;

        h.task_tx
            .send(AgentTask { task: data })
            .await
            .map_err(|_| "agent task channel closed".to_string())?;
        Ok(())
    }

    pub async fn get_report(&self, agent_id: &str) -> Result<Option<AgentReport>, String> {
        let last_report = {
            let guard = self.inner.read().await;
            let h = guard
                .agents
                .get(agent_id)
                .ok_or_else(|| "agent_id not found".to_string())?;
            Arc::clone(&h.last_report)
        };
        let out = last_report.read().await.clone();
        Ok(out)
    }

    pub async fn kill_agent(&self, agent_id: &str) -> Result<(), String> {
        let mut guard = self.inner.write().await;
        let h = guard
            .agents
            .remove(agent_id)
            .ok_or_else(|| "agent_id not found".to_string())?;
        warn!(agent_id = %agent_id, "killing agent");
        h.join.abort();
        Ok(())
    }
}

fn build_subagent_system_prompt(inherited_system_prompt: &str, info: &AgentInfo) -> String {
    // IMPORTANT:
    // - We do not attempt to expose chain-of-thought; the UI "thought log" is a worker activity log.
    // - We keep the worker output structured to reduce drift and simplify downstream parsing.
    let mut s = String::new();
    s.push_str(inherited_system_prompt);
    s.push_str("\n\n");
    s.push_str("[SUB-AGENT MODE]\n");
    s.push_str("You are an EPHEMERAL WORKER spawned by the Phoenix Orchestrator.\n");
    s.push_str("You MUST stay within your mission. Do not expand scope.\n");
    s.push_str("You MUST respect permissions: if a task would require unavailable permissions, say so.\n\n");
    s.push_str(&format!("AgentName: {}\n", info.name));
    s.push_str(&format!("AgentId: {}\n", info.agent_id));
    s.push_str(&format!("Mission: {}\n", info.mission));
    s.push_str(&format!("Permissions: {:?}\n\n", info.permissions));
    s.push_str(
        "Output STRICT JSON only with the following structure:\n\
{\n\
  \"status\": \"ok\" | \"blocked\" | \"error\",\n\
  \"report\": \"...\",\n\
  \"evidence\": [\"...\"],\n\
  \"next_steps\": [\"...\"]\n\
}\n\n",
    );
    s
}

async fn openrouter_chat(
    http_client: &reqwest::Client,
    openrouter_url: &str,
    openrouter_api_key: &str,
    model: &str,
    system_prompt: &str,
    user_content: &str,
    temperature: f32,
) -> Result<String, String> {
    let payload = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_content}
        ],
        "response_format": {"type": "json_object"},
        "temperature": temperature
    });

    let response = http_client
        .post(openrouter_url)
        .header("Authorization", format!("Bearer {}", openrouter_api_key))
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

    // Minimal extraction (local copy) to avoid tight coupling to main.rs helpers.
    if let Some(err) = api_response.get("error") {
        return Err(format!("OpenRouter returned an error object: {}", err));
    }
    let choice0 = api_response
        .get("choices")
        .and_then(|choices| choices.as_array())
        .and_then(|arr| arr.first());
    if let Some(content) = choice0
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|v| v.as_str())
    {
        return Ok(content.to_string());
    }
    if let Some(text) = choice0.and_then(|c| c.get("text")).and_then(|t| t.as_str()) {
        return Ok(text.to_string());
    }
    if let Some(delta) = choice0
        .and_then(|c| c.get("delta"))
        .and_then(|d| d.get("content"))
        .and_then(|t| t.as_str())
    {
        return Ok(delta.to_string());
    }

    Err(format!(
        "Failed to extract content from OpenRouter response. Raw JSON: {}",
        api_response
    ))
}

/// Log episodic memory (success or failure) to Qdrant for playbook distillation
async fn log_episodic_memory(
    memory_client: Arc<Mutex<crate::memory_client::memory_service_client::MemoryServiceClient<Channel>>>,
    agent_id: &str,
    agent_name: &str,
    task: &str,
    result: &str,
    outcome: &str, // "Success" or "Failure"
) -> Result<(), String> {
    use crate::memory_client::{CommitMemoryRequest};
    use std::collections::HashMap;
    
    let content = format!(
        "Agent: {}\nTask: {}\nOutcome: {}\nResult: {}",
        agent_name, task, outcome, result
    );
    
    let mut metadata = HashMap::new();
    metadata.insert("agent_id".to_string(), agent_id.to_string());
    metadata.insert("agent_name".to_string(), agent_name.to_string());
    metadata.insert("outcome".to_string(), outcome.to_string());
    metadata.insert("task".to_string(), task.to_string());
    
    let request = tonic::Request::new(CommitMemoryRequest {
        content,
        namespace: "episodic_memory".to_string(),
        twin_id: "orchestrator".to_string(),
        memory_type: "Episodic".to_string(),
        risk_level: if outcome == "Failure" { "Medium".to_string() } else { "Low".to_string() },
        metadata,
    });

    let mut client = memory_client.lock().await;
    match client.commit_memory(request).await {
        Ok(response) => {
            let resp = response.into_inner();
            if resp.success {
                info!(
                    agent_id = %agent_id,
                    outcome = %outcome,
                    memory_id = %resp.memory_id,
                    "Episodic memory logged"
                );
                Ok(())
            } else {
                Err(format!("Memory service returned error: {}", resp.error_message))
            }
        }
        Err(e) => {
            warn!(
                agent_id = %agent_id,
                error = %e,
                "Failed to log episodic memory"
            );
            Err(format!("Failed to commit episodic memory: {}", e))
        }
    }
}
