use crate::tools::system::{get_logs, manage_service, run_command};
use crate::{execute_system_tool, is_system_tool, AppState, LLMAction};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// Service health status
#[derive(Debug, Clone, PartialEq)]
pub enum ServiceStatus {
    Online,
    Offline,
    Repairing,
}

/// Service health information
#[derive(Debug, Clone)]
pub struct ServiceHealth {
    pub status: ServiceStatus,
    pub last_check: chrono::DateTime<chrono::Utc>,
    pub last_error: Option<String>,
    pub repair_attempts: u32,
}

/// Health check manager
#[derive(Clone)]
pub struct HealthManager {
    service_health: Arc<RwLock<HashMap<String, ServiceHealth>>>,
    repair_in_progress: Arc<RwLock<HashMap<String, bool>>>,
}

impl HealthManager {
    pub fn new() -> Self {
        Self {
            service_health: Arc::new(RwLock::new(HashMap::new())),
            repair_in_progress: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Check if telemetry service is online
    pub async fn check_telemetry(&self, state: &Arc<AppState>) -> Result<ServiceStatus, String> {
        let telemetry_url = &state.telemetry_url;
        let health_url = format!("{}/v1/telemetry/stream", telemetry_url.trim_end_matches('/'));

        info!(url = %health_url, "Checking telemetry service health");

        let response = state
            .http_client
            .get(&health_url)
            .timeout(Duration::from_secs(5))
            .send()
            .await;

        match response {
            Ok(r) if r.status().is_success() => {
                info!("Telemetry service is ONLINE");
                self.update_service_status("telemetry", ServiceStatus::Online, None, state)
                    .await;
                Ok(ServiceStatus::Online)
            }
            Ok(r) => {
                let error_msg = format!("Telemetry service returned status: {}", r.status());
                warn!(status = %r.status(), "Telemetry service health check failed");
                self.update_service_status("telemetry", ServiceStatus::Offline, Some(error_msg.clone()), state)
                    .await;
                Err(error_msg)
            }
            Err(e) => {
                let error_msg = format!("Telemetry service unreachable: {}", e);
                warn!(error = %e, "Telemetry service health check failed");
                self.update_service_status("telemetry", ServiceStatus::Offline, Some(error_msg.clone()), state)
                    .await;
                Err(error_msg)
            }
        }
    }

    /// Update service status in memory
    async fn update_service_status(
        &self,
        service_name: &str,
        status: ServiceStatus,
        error: Option<String>,
        state: &Arc<AppState>,
    ) {
        let mut health_map = self.service_health.write().await;
        let health = health_map.entry(service_name.to_string()).or_insert(ServiceHealth {
            status: ServiceStatus::Online,
            last_check: chrono::Utc::now(),
            last_error: None,
            repair_attempts: 0,
        });

        let was_offline = health.status == ServiceStatus::Offline;
        health.status = status.clone();
        health.last_check = chrono::Utc::now();
        health.last_error = error;

        // If service just went offline, trigger repair
        if status == ServiceStatus::Offline && !was_offline {
            info!(service = %service_name, "Service went offline, triggering repair task");
            self.trigger_repair(service_name.to_string(), state).await;
        }
    }

    /// Get current service status
    pub async fn get_service_status(&self, service_name: &str) -> Option<ServiceStatus> {
        let health_map = self.service_health.read().await;
        health_map.get(service_name).map(|h| h.status.clone())
    }

    /// Get a snapshot of all tracked service health.
    pub async fn get_all_service_health(&self) -> HashMap<String, ServiceHealth> {
        self.service_health.read().await.clone()
    }

    /// Compute an aggregate "neural sync" score from tracked service health.
    ///
    /// Current heuristic: percentage of services that are Online.
    pub async fn calculate_neural_sync(&self) -> f32 {
        let health_map = self.service_health.read().await;
        if health_map.is_empty() {
            return 100.0;
        }
        let total = health_map.len() as f32;
        let online = health_map
            .values()
            .filter(|h| h.status == ServiceStatus::Online)
            .count() as f32;
        (online / total) * 100.0
    }

    /// Trigger repair task for a service
    async fn trigger_repair(&self, service_name: String, state: &Arc<AppState>) {
        // Check if repair is already in progress
        {
            let mut in_progress = self.repair_in_progress.write().await;
            if in_progress.get(&service_name).copied().unwrap_or(false) {
                warn!(service = %service_name, "Repair already in progress, skipping");
                return;
            }
            in_progress.insert(service_name.clone(), true);
        }

        // Update status to Repairing
        {
            let mut health_map = self.service_health.write().await;
            if let Some(health) = health_map.get_mut(&service_name) {
                health.status = ServiceStatus::Repairing;
                health.repair_attempts += 1;
            }
        }

        let state_clone = Arc::clone(state);
        let health_mgr = self.clone();

        // Spawn repair task
        tokio::spawn(async move {
            info!(service = %service_name, "Starting repair task");
            let repair_result = health_mgr
                .execute_repair_workflow(&service_name, &state_clone)
                .await;

            match repair_result {
                Ok(_) => {
                    info!(service = %service_name, "Repair task completed successfully");
                }
                Err(e) => {
                    error!(service = %service_name, error = %e, "Repair task failed");
                }
            }

            // Mark repair as complete
            {
                let mut in_progress = health_mgr.repair_in_progress.write().await;
                in_progress.remove(&service_name);
            }

            // Re-check health after repair (we'll need to pass http_client and telemetry_url)
            // For now, just log the result
            info!(service = %service_name, "Repair task completed");
        });
    }

    /// Execute the repair workflow using direct system tools (simplified for repair tasks)
    async fn execute_repair_workflow_direct(
        &self,
        service_name: &str,
        http_client: &reqwest::Client,
        telemetry_url: &str,
    ) -> Result<(), String> {
        // Direct repair workflow without full LLM planning
        // This avoids the Send trait issues with AppState
        info!(service = %service_name, "Executing direct repair workflow");
        
        // Step 1: Check logs
        let log_cmd = format!("journalctl -u {} -n 50", service_name);
        match run_command(log_cmd).await {
            Ok(logs) => {
                info!(service = %service_name, logs_len = logs.len(), "Retrieved service logs");
            }
            Err(e) => {
                warn!(service = %service_name, error = %e, "Failed to retrieve logs");
            }
        }
        
        // Step 2: Attempt restart
        self.attempt_service_restart(service_name).await?;
        
        Ok(())
    }

    /// Execute the repair workflow using LLM and System Tools (full version)
    async fn execute_repair_workflow(
        &self,
        service_name: &str,
        state: &Arc<AppState>,
    ) -> Result<(), String> {
        // Build the repair prompt
        let repair_prompt = format!(
            r#"CRITICAL: {} Service is OFFLINE. 

Use your System Tools to diagnose and repair:
1. Check logs for crashes (OOM, Port conflicts) using run_command with journalctl
2. Verify the storage directory exists using run_command
3. Restart the service using systemctl

Report your actions and findings. Execute the necessary repair steps immediately."#,
            service_name
        );

        info!(service = %service_name, prompt = %repair_prompt, "Sending repair prompt to LLM");

        // Use LLM to plan the repair
        // For repair tasks, we'll use a direct approach with system tools
        // rather than going through the full LLM planning (which requires UI approval)
        let action = if state.llm_provider == "openrouter" {
            // Try to use LLM, but if it fails, fall back to direct repair
            match crate::llm_plan_openrouter(&repair_prompt, "system", state, false, None).await {
                Ok((action, _)) => action,
                Err(e) => {
                    warn!(error = %e, "LLM planning failed, using direct repair");
                    // Fall back to direct repair
                    LLMAction::ActionTool {
                        tool_name: "run_command".to_string(),
                        args: {
                            let mut args = HashMap::new();
                            args.insert("cmd".to_string(), format!("journalctl -u {} -n 50", service_name));
                            args
                        },
                    }
                }
            }
        } else {
            // For mock provider, create a simple repair action
            LLMAction::ActionTool {
                tool_name: "run_command".to_string(),
                args: {
                    let mut args = HashMap::new();
                    args.insert("cmd".to_string(), format!("journalctl -u {} -n 50", service_name));
                    args
                },
            }
        };

        // Execute the repair action (auto-approved for repair tasks)
        match action {
            LLMAction::ActionTool { tool_name, args } => {
                info!(
                    service = %service_name,
                    tool = %tool_name,
                    "Executing repair tool (auto-approved)"
                );

                // Check if it's a system tool and execute directly
                if is_system_tool(&tool_name) {
                    match execute_system_tool(&tool_name, &args).await {
                        Ok(output) => {
                            info!(service = %service_name, output = %output, "Repair tool executed successfully");
                            
                            // If the tool was a diagnostic, we might need to follow up with a restart
                            if tool_name == "run_command" {
                                // Try to restart the service
                                let restart_result = self.attempt_service_restart(service_name).await;
                                if let Err(e) = restart_result {
                                    warn!(service = %service_name, error = %e, "Service restart failed");
                                }
                            }
                        }
                        Err(e) => {
                            return Err(format!("Repair tool execution failed: {}", e));
                        }
                    }
                } else {
                    // For non-system tools, we'd need to go through the Tools Service
                    // But for repair, we should focus on system tools
                    warn!(tool = %tool_name, "Non-system tool in repair workflow, skipping");
                }
            }
            LLMAction::ActionResponse { content } => {
                info!(service = %service_name, response = %content, "LLM repair response");
                // If LLM just responded, try to restart anyway
                let _ = self.attempt_service_restart(service_name).await;
            }
            _ => {
                warn!(service = %service_name, "Unexpected action type in repair workflow");
            }
        }

        Ok(())
    }

    /// Attempt to restart a service using the cross-platform service manager
    async fn attempt_service_restart(&self, service_name: &str) -> Result<(), String> {
        info!(service = %service_name, "Attempting to restart service");

        // Map service name to systemd service name
        let systemd_service = match service_name {
            "telemetry" => "telemetry",
            "sys_control" => "sys_control",
            _ => service_name,
        };

        // Restart the service (cross-platform abstraction)
        manage_service(systemd_service, "restart").await?;

        // Wait a bit for service to start
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Check status
        let status_output = manage_service(systemd_service, "status").await?;
        info!(service = %service_name, status = %status_output, "Service status after restart");

        Ok(())
    }

    /// Update repair status after repair attempt
    async fn update_repair_status(
        &self,
        service_name: &str,
        repair_result: &Result<ServiceStatus, String>,
    ) {
        let status_text = match repair_result {
            Ok(ServiceStatus::Online) => {
                info!(service = %service_name, "Service restored to ONLINE after repair");
                "ONLINE - Service successfully restored"
            }
            Ok(ServiceStatus::Offline) => {
                warn!(service = %service_name, "Service still OFFLINE after repair attempt");
                "OFFLINE - Repair attempted but service still down"
            }
            Ok(ServiceStatus::Repairing) => {
                "REPAIRING - Repair in progress"
            }
            Err(e) => {
                error!(service = %service_name, error = %e, "Health check failed after repair");
                &format!("ERROR - Health check failed: {}", e)
            }
        };

        // Update service health record
        {
            let mut health_map = self.service_health.write().await;
            if let Some(health) = health_map.get_mut(service_name) {
                if let Ok(status) = repair_result {
                    health.status = status.clone();
                }
                health.last_check = chrono::Utc::now();
            }
        }

        // Log repair status (in a full implementation, this would be committed to Memory Service)
        info!(
            service = %service_name,
            status = %status_text,
            "Repair workflow completed"
        );
    }

    /// Start periodic health checks
    pub fn start_periodic_checks(&self, state: Arc<AppState>, interval_secs: u64) {
        let health_mgr = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
            loop {
                interval.tick().await;
                let _ = health_mgr.check_telemetry(&state).await;
            }
        });
    }
}
