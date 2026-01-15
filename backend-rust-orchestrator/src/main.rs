use axum::{
    extract::{Json, State},
    http::{Method, StatusCode},
    response::Json as ResponseJson,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::env;
use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::sync::Arc;
use quick_xml::Reader as XmlReader;
use quick_xml::events::Event as XmlEvent;
use tokio::process::Command as TokioCommand;
use tokio::sync::{mpsc, RwLock};
use tokio::fs;
use tonic::transport::Channel;
use tracing::{info, warn, error};
use uuid::Uuid;
use tower_http::cors::{Any, CorsLayer};

use tools::network_scanner;

const DEFAULT_SYSTEM_PROMPT_TEMPLATE: &str = include_str!("../config/system_prompt.txt");

// P50: Analyst System Prompt for transcript summarization
const ANALYST_SYSTEM_PROMPT: &str = "Summarize the following transcript into 3 sentences. Identify key decisions and action items. Output strictly as JSON: { \"summary\": \"...\", \"decisions\": [], \"tasks\": [] }.";

fn truncate_for_log(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    format!("{}…(truncated)", &s[..max])
}

/// OpenRouter is mostly OpenAI-compatible, but different upstream models/providers can return:
/// - choices[0].message.content as a string
/// - choices[0].message.content as an array of parts
/// - choices[0].text
/// - choices[0].delta.content (stream-style payloads)
/// - error object with HTTP 200
fn extract_openrouter_content(api_response: &serde_json::Value) -> Result<String, String> {
    if let Some(err) = api_response.get("error") {
        // Some gateways/providers return error bodies with HTTP 200.
        return Err(format!(
            "OpenRouter returned an error object: {}",
            truncate_for_log(&err.to_string(), 8_000)
        ));
    }

    let choice0 = api_response
        .get("choices")
        .and_then(|choices| choices.as_array())
        .and_then(|arr| arr.first());

    // 1) OpenAI-style: choices[0].message.content
    if let Some(content) = choice0
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
    {
        if let Some(s) = content.as_str() {
            return Ok(s.to_string());
        }

        // Some models return content as an array of parts.
        if let Some(parts) = content.as_array() {
            let mut out = String::new();
            for p in parts {
                if let Some(s) = p.as_str() {
                    out.push_str(s);
                    continue;
                }
                if let Some(s) = p.get("text").and_then(|t| t.as_str()) {
                    out.push_str(s);
                    continue;
                }
                // If it's some other JSON object part, stringify it.
                if p.is_object() {
                    out.push_str(&p.to_string());
                }
            }
            if !out.trim().is_empty() {
                return Ok(out);
            }
        }

        // If content is an object (rare), stringify it.
        if content.is_object() {
            return Ok(content.to_string());
        }
    }

    // 2) Non-chat completions: choices[0].text
    if let Some(text) = choice0.and_then(|c| c.get("text")).and_then(|t| t.as_str()) {
        return Ok(text.to_string());
    }

    // 3) Stream-style: choices[0].delta.content
    if let Some(delta) = choice0
        .and_then(|c| c.get("delta"))
        .and_then(|d| d.get("content"))
        .and_then(|t| t.as_str())
    {
        return Ok(delta.to_string());
    }

    Err(format!(
        "Failed to extract content from OpenRouter response. Raw JSON: {}",
        truncate_for_log(&api_response.to_string(), 8_000)
    ))
}

fn extract_first_json_object(s: &str) -> Option<&str> {
    let start = s.find('{')?;
    let end = s.rfind('}')?;
    if end <= start {
        return None;
    }
    Some(&s[start..=end])
}

fn load_dotenv() {
    // We often run services from their crate directories (e.g. `cd backend-rust-orchestrator && cargo run`).
    // In that case, the repo-root `.env` won't be found if we only look at the current working directory.
    //
    // Prefer a deterministic search anchored at the crate location.
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let candidates: Vec<std::path::PathBuf> = vec![
        // If someone created a per-crate env
        manifest_dir.join(".env"),
        // Repo root (this repo's services are in `<repo>/backend-rust-orchestrator`)
        manifest_dir
            .parent()
            .map(|p| p.join(".env"))
            .unwrap_or_else(|| std::path::PathBuf::from(".env")),
        // Fallback to runtime CWD, in case the binary is launched from repo root.
        std::path::PathBuf::from(".env"),
    ];

    for candidate in candidates {
        info!(path = %candidate.display(), exists = %candidate.exists(), "Dotenv candidate");
        if !candidate.exists() {
            continue;
        }

        match dotenvy::from_path(&candidate) {
            Ok(_) => {
                info!(path = %candidate.display(), "Loaded .env");
                return;
            }
            Err(e) => {
                warn!(path = %candidate.display(), error = %e, "Failed to load .env");
            }
        }
    }

    warn!(
        manifest_dir = %manifest_dir.display(),
        cwd = %env::current_dir().map(|p| p.display().to_string()).unwrap_or_else(|_| "<unknown>".to_string()),
        "No .env file loaded; relying on process environment"
    );
}

// Include generated proto clients
pub mod memory_client {
    tonic::include_proto!("memory");
}

pub mod tools_client {
    tonic::include_proto!("tools");
}

pub mod orchestrator_admin {
    tonic::include_proto!("orchestrator_admin");
}

pub mod orchestrator {
    tonic::include_proto!("orchestrator");
}

pub mod handshake_proto {
    tonic::include_proto!("handshake");
}

pub mod memory_exchange_proto {
    tonic::include_proto!("memory_exchange");
}

use memory_client::memory_service_client::MemoryServiceClient;
use memory_client::{
    CommitMemoryRequest,
    DeleteMemoryRequest,
    DeleteMemoryResponse,
    ListMemoriesRequest,
    ListMemoriesResponse,
    QueryMemoryRequest,
    QueryMemoryResponse,
};
use tools_client::tool_executor_service_client::ToolExecutorServiceClient;
use tools_client::{ExecutionRequest, ExecutionResponse};

use orchestrator_admin::orchestrator_admin_service_server::{
    OrchestratorAdminService, OrchestratorAdminServiceServer,
};
use orchestrator_admin::{
    HealthCheckRequest as AdminHealthCheckRequest,
    HealthCheckResponse as AdminHealthCheckResponse,
    GetPromptHistoryRequest, GetPromptHistoryResponse,
    PromptHistoryEntry,
    UpdateSystemPromptRequest, UpdateSystemPromptResponse,
};

use orchestrator::orchestrator_service_server::{
    OrchestratorService, OrchestratorServiceServer,
};
use orchestrator::{SummarizeRequest as GrpcSummarizeRequest, SummarizeResponse as GrpcSummarizeResponse};

mod tools;
mod agents;
mod health;
mod project_watcher;
mod email_teams_monitor;
mod bus;
mod agent_library;
mod playbook_distiller;
mod security;
mod memory;
mod preferences;
mod analytics;
mod network;
mod foundry;
mod api;
mod services;
mod knowledge;
use tools::system::{get_logs, manage_service, read_file, run_command, systemctl, write_file};
use tools::get_system_snapshot;
use health::HealthManager;
use project_watcher::ProjectWatcher;
use email_teams_monitor::EmailTeamsMonitor;

#[derive(Clone, Debug)]
struct SystemPromptRepository {
    path: PathBuf,
}

impl SystemPromptRepository {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn default_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("config")
            .join("system_prompt.txt")
    }

    async fn load_or_init(&self, default_prompt: &str) -> Result<String, String> {
        if tokio::fs::metadata(&self.path).await.is_ok() {
            let s = tokio::fs::read_to_string(&self.path)
                .await
                .map_err(|e| format!("failed to read system prompt file {}: {e}", self.path.display()))?;
            if !s.trim().is_empty() {
                return Ok(s);
            }
        }

        // Initialize with default.
        self.write(default_prompt).await?;
        Ok(default_prompt.to_string())
    }

    async fn write(&self, prompt: &str) -> Result<(), String> {
        let dir = self
            .path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));

        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|e| format!("failed to create config dir {}: {e}", dir.display()))?;

        let tmp_path = dir.join(format!(
            "system_prompt.txt.tmp-{}",
            Uuid::new_v4().to_string()
        ));

        tokio::fs::write(&tmp_path, prompt)
            .await
            .map_err(|e| format!("failed to write temp system prompt file {}: {e}", tmp_path.display()))?;

        // Best-effort atomic replace.
        // On Windows, rename won't overwrite the destination, so we remove first.
        let _ = tokio::fs::remove_file(&self.path).await;
        tokio::fs::rename(&tmp_path, &self.path)
            .await
            .map_err(|e| format!("failed to replace system prompt file {}: {e}", self.path.display()))?;

        Ok(())
    }
}

#[derive(Clone, Debug)]
struct SystemPromptManager {
    repo: SystemPromptRepository,
    current: Arc<RwLock<String>>,
    history: Arc<RwLock<Vec<PromptHistoryRecord>>>,
}

#[derive(Clone, Debug)]
struct PromptHistoryRecord {
    id: String,
    timestamp: String,
    previous_prompt: String,
    new_prompt: String,
    change_summary: String,
}

impl SystemPromptManager {
    async fn update(&self, new_prompt: String) -> Result<(), String> {
        self.update_with_history(new_prompt, None).await
    }

    async fn update_with_history(
        &self,
        new_prompt: String,
        change_summary: Option<String>,
    ) -> Result<(), String> {
        let trimmed = new_prompt.trim();
        if trimmed.is_empty() {
            return Err("new_prompt must not be empty".to_string());
        }

        let previous_prompt = self.current.read().await.clone();
        let normalized_new_prompt = format!("{}\n", trimmed);

        // Persist first; only update in-memory if the write succeeds.
        self.repo.write(trimmed).await?;

        {
            let mut guard = self.current.write().await;
            *guard = normalized_new_prompt.clone();
        }

        let entry = PromptHistoryRecord {
            id: Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            previous_prompt,
            new_prompt: normalized_new_prompt,
            change_summary: change_summary.unwrap_or_default(),
        };

        {
            let mut history = self.history.write().await;
            history.push(entry);
        }

        Ok(())
    }

    async fn history(&self) -> Vec<PromptHistoryRecord> {
        self.history.read().await.clone()
    }

    async fn get_template(&self) -> String {
        self.current.read().await.clone()
    }
}

#[derive(Clone, Debug)]
struct OrchestratorAdminServiceImpl {
    prompt_mgr: SystemPromptManager,
}

#[tonic::async_trait]
impl OrchestratorAdminService for OrchestratorAdminServiceImpl {
    async fn update_system_prompt(
        &self,
        request: tonic::Request<UpdateSystemPromptRequest>,
    ) -> Result<tonic::Response<UpdateSystemPromptResponse>, tonic::Status> {
        let req = request.into_inner();
        let new_prompt = req.new_prompt;
        let change_summary = if req.change_summary.trim().is_empty() {
            None
        } else {
            Some(req.change_summary)
        };

        if new_prompt.len() > 200_000 {
            return Err(tonic::Status::invalid_argument(
                "new_prompt too large (max 200k chars)",
            ));
        }

        self.prompt_mgr
            .update_with_history(new_prompt, change_summary)
            .await
            .map_err(|e| tonic::Status::internal(e))?;

        Ok(tonic::Response::new(UpdateSystemPromptResponse {
            success: true,
            message: "system prompt updated".to_string(),
        }))
    }

    async fn health_check(
        &self,
        _request: tonic::Request<AdminHealthCheckRequest>,
    ) -> Result<tonic::Response<AdminHealthCheckResponse>, tonic::Status> {
        Ok(tonic::Response::new(AdminHealthCheckResponse {
            status: "healthy".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            message: "Orchestrator admin service is operational".to_string(),
        }))
    }

    async fn get_prompt_history(
        &self,
        _request: tonic::Request<GetPromptHistoryRequest>,
    ) -> Result<tonic::Response<GetPromptHistoryResponse>, tonic::Status> {
        let history = self.prompt_mgr.history().await;
        let entries = history
            .into_iter()
            .map(|e| PromptHistoryEntry {
                id: e.id,
                timestamp: e.timestamp,
                previous_prompt: e.previous_prompt,
                new_prompt: e.new_prompt,
                change_summary: e.change_summary,
            })
            .collect();

        Ok(tonic::Response::new(GetPromptHistoryResponse { entries }))
    }
}

// --- HTTP Request/Response Types ---

#[derive(Debug, Deserialize)]
struct ChatRequest {
    message: String,
    twin_id: String,
    session_id: String,
    namespace: Option<String>,
    #[serde(default)]
    media_active: bool,
    #[serde(default)]
    user_name: Option<String>,
    // LLM settings (optional - will use defaults if not provided)
    #[serde(default)]
    temperature: Option<f32>,
    #[serde(default)]
    top_p: Option<f32>,
    #[serde(default)]
    max_tokens: Option<u32>,
    #[serde(default)]
    max_memory: Option<u32>,
}

#[derive(Debug, Serialize)]
struct ChatResponse {
    response: String,
    job_id: Option<String>,
    actions_taken: Vec<String>,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    issued_command: Option<serde_json::Value>,
    /// Raw (unparsed) structured JSON decision from the LLM planner.
    /// Only present for action-producing decisions (tool/memory).
    #[serde(skip_serializing_if = "Option::is_none")]
    raw_orchestrator_decision: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MemoryListHttpRequest {
    #[serde(default)]
    namespace: String,
    #[serde(default = "default_page")]
    page: i32,
    #[serde(default = "default_page_size")]
    page_size: i32,
    #[serde(default)]
    twin_id: String,
}

fn default_page() -> i32 {
    1
}

fn default_page_size() -> i32 {
    50
}

#[derive(Debug, Deserialize)]
struct SummarizeTranscriptRequest {
    transcript: String,
    filename: String,
    twin_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TranscriptInsights {
    summary: String,
    key_decisions: Vec<String>,
    follow_up_tasks: Vec<String>,
}

#[derive(Debug, Serialize)]
struct SummarizeTranscriptResponse {
    success: bool,
    insights: Option<TranscriptInsights>,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AnalystInsightsJson {
    summary: String,

    // Canonical names (P50)
    #[serde(default)]
    #[serde(alias = "key_decisions")]
    decisions: Vec<String>,

    #[serde(default)]
    #[serde(alias = "follow_up_tasks")]
    tasks: Vec<String>,
}

#[derive(Debug, Serialize)]
struct MemoryResultJson {
    id: String,
    timestamp: String,
    content: String,
    agent_id: String,
    risk_level: String,
    similarity: f64,
    memory_type: String,
    metadata: HashMap<String, String>,
}

#[derive(Debug, Serialize)]
struct MemoryListHttpResponse {
    memories: Vec<MemoryResultJson>,
    total_count: i32,
    total_pages: i32,
    page: i32,
    page_size: i32,
    namespace: String,
}

#[derive(Debug, Deserialize)]
struct MemoryDeleteHttpRequest {
    memory_id: String,
    #[serde(default)]
    namespace: String,
}

#[derive(Debug, Serialize)]
struct MemoryDeleteHttpResponse {
    success: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    error_message: String,
}

#[derive(Debug, Serialize)]
struct PromptHistoryEntryHttp {
    id: String,
    timestamp: String,
    previous_prompt: String,
    new_prompt: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    change_summary: String,
}

#[derive(Debug, Serialize)]
struct PromptHistoryHttpResponse {
    entries: Vec<PromptHistoryEntryHttp>,
}

#[derive(Debug, Deserialize)]
struct PromptRestoreHttpRequest {
    entry_id: String,
}

#[derive(Debug, Serialize)]
struct PromptRestoreHttpResponse {
    success: bool,
    message: String,
}

#[derive(Debug, Serialize)]
struct PromptCurrentHttpResponse {
    prompt: String,
}

#[derive(Debug, Deserialize)]
struct PromptUpdateHttpRequest {
    new_prompt: String,
    #[serde(default)]
    change_summary: String,
}

#[derive(Debug, Serialize)]
struct PromptUpdateHttpResponse {
    success: bool,
    message: String,
}

#[derive(Debug, Serialize)]
struct PromptResetHttpResponse {
    success: bool,
    message: String,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    service: &'static str,
    status: &'static str,
    version: &'static str,
}

#[derive(Debug, Serialize)]
struct SyncMetricsResponse {
    neural_sync: f32,
    services: HashMap<String, String>,
}

// --- Job Management ---

#[derive(Debug, Clone)]
struct Job {
    id: Uuid,
    twin_id: String,
    status: String,
    progress: u32,
    logs: Vec<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

type JobQueue = Arc<RwLock<HashMap<String, Job>>>;

// --- Application State ---

#[derive(Clone)]
struct AppState {
    memory_client: MemoryServiceClient<Channel>,
    tools_client: ToolExecutorServiceClient<Channel>,
    job_queue: JobQueue,
    job_sender: mpsc::Sender<Job>,
    // LLM provider selection
    llm_provider: String,
    // Pending tool requests awaiting UI authorization
    pending_tools: Arc<RwLock<HashMap<String, PendingToolCall>>>,
    // Pending memory requests awaiting UI authorization
    pending_memories: Arc<RwLock<HashMap<String, PendingMemoryCall>>>,
    // OpenRouter LLM configuration
    http_client: reqwest::Client,
    openrouter_url: String,
    openrouter_api_key: String,
    openrouter_model: String,

    // Telemetry service (media recordings)
    telemetry_url: String,

    // Self-improvement / persona prompt
    system_prompt: SystemPromptManager,

    // Per-user personalization (persona presets + profile)
    preferences: preferences::PreferencesManager,

    // Health check manager
    health_manager: HealthManager,

    // Last network scan results (keyed by twin_id::namespace)
    last_network_scans: Arc<RwLock<HashMap<String, NetworkScanResult>>>,

    // Project folder watcher for monitoring application logs/files
    project_watcher: Arc<ProjectWatcher>,

    // Email and Teams monitoring
    email_teams_monitor: Arc<RwLock<Option<EmailTeamsMonitor>>>,

    // Ephemeral sub-agent factory (worker crew)
    agent_factory: Arc<agents::factory::AgentFactory>,

    // Agent library for manifest-based agent loading
    agent_library: Arc<agents::loader::AgentLibrary>,

    // Global message bus for inter-agent communication
    message_bus: Arc<bus::GlobalMessageBus>,

    // Leaderboard engine for agent analytics
    leaderboard_engine: Arc<analytics::leaderboard::LeaderboardEngine>,

    // Network handshake service
    handshake_service: Arc<network::handshake::NodeHandshakeServiceImpl>,

    // Quarantine manager
    quarantine_manager: Arc<network::quarantine::QuarantineManager>,

    // Mesh health service
    mesh_health_service: Arc<analytics::mesh_health::MeshHealthService>,

    // Fleet manager for distributed node tracking
    fleet_state: Arc<network::fleet::FleetState>,
}

impl AppState {
    /// Publish an event to the global message bus.
    pub fn publish_event(&self, event: bus::PhoenixEvent) -> usize {
        self.message_bus.publish(event)
    }

    /// Subscribe to the global message bus.
    pub fn subscribe_to_bus(&self) -> tokio::sync::broadcast::Receiver<bus::PhoenixEvent> {
        self.message_bus.subscribe()
    }
}

#[derive(Debug, Clone)]
struct PendingToolCall {
    tool_name: String,
    args: HashMap<String, String>,
    namespace: String,
}

#[derive(Debug, Clone)]
struct PendingMemoryCall {
    memory_id: String,
    query: String,
    namespace: String,
}

// --- LLM Planning Logic (OpenRouter Integration) ---

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action_type", content = "details")]
pub enum LLMAction {
    #[serde(rename = "ActionMemory")]
    ActionMemory {
        query: String,
    },
    #[serde(rename = "ActionTool")]
    ActionTool {
        tool_name: String,
        args: HashMap<String, String>,
    },
    #[serde(rename = "ActionResponse")]
    ActionResponse {
        content: String,
    },
    #[serde(rename = "ActionBuildTool")]
    ActionBuildTool {
        tool_name: String,
        tool_code: String,
    },
    #[serde(rename = "ActionListRecordings")]
    ActionListRecordings {
        #[serde(default)]
        twin_id: Option<String>,
        #[serde(default)]
        limit: Option<u32>,
    },
    #[serde(rename = "ActionInspectSystem")]
    ActionInspectSystem {},
    #[serde(rename = "ActionKillProcess")]
    ActionKillProcess { pid: u32 },
    #[serde(rename = "ActionSelfImprove")]
    ActionSelfImprove {
        new_prompt: String,
    },

    #[serde(rename = "ActionNetworkScan")]
    ActionNetworkScan {
        target: String,
    },
    #[serde(rename = "ActionMonitorEmail")]
    ActionMonitorEmail {
        #[serde(default)]
        filter_unread: Option<bool>,
    },
    #[serde(rename = "ActionSendEmail")]
    ActionSendEmail {
        original_email_id: String,
        reply_body: String,
    },
    #[serde(rename = "ActionQuarantineNode")]
    ActionQuarantineNode {
        node_id: String,
        reason: String,
    },
    #[serde(rename = "ActionMonitorTeams")]
    ActionMonitorTeams {},
    #[serde(rename = "ActionSendTeamsMessage")]
    ActionSendTeamsMessage {
        chat_id: String,
        message_content: String,
    },
    #[serde(rename = "ActionEmailTrends")]
    ActionEmailTrends {
        period: String, // "day", "week", "month"
    },

    #[serde(rename = "ActionSpawnAgent")]
    ActionSpawnAgent {
        name: String,
        mission: String,
        #[serde(default)]
        permissions: Vec<String>,
    },
    #[serde(rename = "ActionSyncAgentLibrary")]
    ActionSyncAgentLibrary {},
    #[serde(rename = "ActionGitPush")]
    ActionGitPush {
        repo_path: String,
        playbooks_dir: String,
        commit_message: String,
        #[serde(default)]
        remote_name: Option<String>,
        #[serde(default)]
        branch: Option<String>,
    },
}

#[derive(Debug, Deserialize)]
struct AgentSpawnHttpRequest {
    name: String,
    mission: String,
    #[serde(default)]
    permissions: Vec<String>,
    #[serde(default)]
    twin_id: String,
    #[serde(default)]
    user_name: Option<String>,
    #[serde(default)]
    media_active: bool,
}

#[derive(Debug, Deserialize)]
struct AgentTaskHttpRequest {
    data: String,
}

#[derive(Debug, Deserialize)]
struct AgentIdPath {
    agent_id: String,
}

async fn build_effective_system_prompt(
    state: &AppState,
    twin_id: &str,
    user_name: Option<&str>,
    media_active: bool,
) -> String {
    let template = state.system_prompt.get_template().await;
    let base = if template.trim().is_empty() {
        DEFAULT_SYSTEM_PROMPT_TEMPLATE.to_string()
    } else {
        template
    };
    let mut system_prompt = base.replace("{twin_id}", twin_id);
    let user_display_name = user_name.unwrap_or("FG_User");
    system_prompt = system_prompt.replace("{user_name}", user_display_name);
    if media_active {
        system_prompt.push_str("\n\n[CONTEXT: MULTI-MODAL ACTIVE]\n");
        system_prompt.push_str("media_active=true - The operator is currently recording voice/video and/or sharing their screen in real-time.\n");
        system_prompt.push_str("- You are aware that live multi-modal input (audio/video/screen) is being captured.\n");
        system_prompt.push_str("- You can reference this in your responses (e.g., 'I see you're currently recording...').\n");
        system_prompt.push_str("- Use ActionListRecordings to discover and reference stored recordings from previous sessions.\n");
        system_prompt.push_str("- Provide context-aware responses that account for the visual/audio information being captured.\n");
    }

    // Apply per-user personalization overlay (style/tone). This does not grant new capabilities.
    let overlay = state.preferences.render_prompt_overlay(twin_id).await;
    if !overlay.trim().is_empty() {
        system_prompt.push_str("\n\n");
        system_prompt.push_str(&overlay);
    }
    system_prompt
}

#[derive(Debug, Deserialize)]
struct PreferencesGetQuery {
    #[serde(default)]
    twin_id: String,
}

async fn handle_preferences_get(
    State(state): State<AppState>,
    axum::extract::Query(query): axum::extract::Query<PreferencesGetQuery>,
) -> Result<ResponseJson<preferences::UserPreferences>, StatusCode> {
    if query.twin_id.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let prefs = state.preferences.get_for_twin(&query.twin_id).await;
    Ok(ResponseJson(prefs))
}

#[derive(Debug, Deserialize)]
struct PreferencesUpdateHttpRequest {
    twin_id: String,
    #[serde(default)]
    profile: preferences::UserProfile,
    #[serde(default)]
    persona_preset: String,
    #[serde(default)]
    custom_instructions: String,
    #[serde(default)]
    verbosity: preferences::Verbosity,
    #[serde(default)]
    enable_cynical: bool,
    #[serde(default)]
    enable_sarcastic: bool,
}

#[derive(Debug, Serialize)]
struct PreferencesUpdateHttpResponse {
    success: bool,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    preferences: Option<preferences::UserPreferences>,
}

async fn handle_preferences_update(
    State(state): State<AppState>,
    Json(request): Json<PreferencesUpdateHttpRequest>,
) -> Result<ResponseJson<PreferencesUpdateHttpResponse>, StatusCode> {
    if request.twin_id.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let updated = preferences::UserPreferences {
        profile: request.profile,
        persona_preset: request.persona_preset,
        custom_instructions: request.custom_instructions,
        verbosity: request.verbosity,
        enable_cynical: request.enable_cynical,
        enable_sarcastic: request.enable_sarcastic,
        updated_at: chrono::Utc::now().to_rfc3339(),
    };

    let saved = state
        .preferences
        .update_for_twin(&request.twin_id, updated)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to update preferences");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Best-effort audit copy into Memory Service.
    // This supports "saved memories" semantics without relying solely on local filesystem.
    {
        let mut mem = state.memory_client.clone();
        let content = serde_json::to_string_pretty(&saved).unwrap_or_else(|_| "{}".to_string());
        let mut metadata = HashMap::new();
        metadata.insert("type".to_string(), "user_preferences".to_string());
        metadata.insert("persona_preset".to_string(), saved.persona_preset.clone());
        metadata.insert("updated_at".to_string(), saved.updated_at.clone());
        let _ = mem
            .commit_memory(tonic::Request::new(CommitMemoryRequest {
                content,
                namespace: "user_preferences".to_string(),
                twin_id: request.twin_id.clone(),
                memory_type: "Preference".to_string(),
                risk_level: "Low".to_string(),
                metadata,
            }))
            .await;
    }

    Ok(ResponseJson(PreferencesUpdateHttpResponse {
        success: true,
        message: "preferences updated".to_string(),
        preferences: Some(saved),
    }))
}

#[derive(Debug, Serialize)]
struct PreferencesPresetsHttpResponse {
    presets: Vec<preferences::PersonaPreset>,
}

async fn handle_preferences_presets() -> ResponseJson<PreferencesPresetsHttpResponse> {
    ResponseJson(PreferencesPresetsHttpResponse {
        presets: preferences::default_persona_presets(),
    })
}

async fn handle_agents_list(
    State(state): State<AppState>,
) -> ResponseJson<serde_json::Value> {
    let agents = state.agent_factory.list_agents().await;
    ResponseJson(json!({
        "max_agents": state.agent_factory.max_agents(),
        "agents": agents,
    }))
}

async fn handle_agents_spawn(
    State(state): State<AppState>,
    Json(req): Json<AgentSpawnHttpRequest>,
) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    if req.twin_id.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let inherited = build_effective_system_prompt(
        &state,
        &req.twin_id,
        req.user_name.as_deref(),
        req.media_active,
    )
    .await;
    let res = state
        .agent_factory
        .spawn_agent(req.name, req.mission, req.permissions, inherited)
        .await
        .map_err(|e| {
            error!(error = %e, "agent spawn failed");
            StatusCode::BAD_REQUEST
        })?;

    Ok(ResponseJson(json!({"ok": true, "agent_id": res.agent_id})))
}

async fn handle_agents_post_task(
    State(state): State<AppState>,
    axum::extract::Path(path): axum::extract::Path<AgentIdPath>,
    Json(req): Json<AgentTaskHttpRequest>,
) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    state
        .agent_factory
        .post_task(&path.agent_id, req.data)
        .await
        .map_err(|e| {
            error!(error = %e, "post_task failed");
            StatusCode::BAD_REQUEST
        })?;
    Ok(ResponseJson(json!({"ok": true})))
}

async fn handle_agents_get_report(
    State(state): State<AppState>,
    axum::extract::Path(path): axum::extract::Path<AgentIdPath>,
) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    let rep = state
        .agent_factory
        .get_report(&path.agent_id)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    Ok(ResponseJson(json!({"ok": true, "report": rep})))
}

async fn handle_agents_get_logs(
    State(state): State<AppState>,
    axum::extract::Path(path): axum::extract::Path<AgentIdPath>,
) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    let logs = state
        .agent_factory
        .get_logs(&path.agent_id)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    Ok(ResponseJson(json!({"ok": true, "logs": logs})))
}

async fn handle_agents_kill(
    State(state): State<AppState>,
    axum::extract::Path(path): axum::extract::Path<AgentIdPath>,
) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    state
        .agent_factory
        .kill_agent(&path.agent_id)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    Ok(ResponseJson(json!({"ok": true})))
}

async fn handle_agents_leaderboard(
    State(state): State<AppState>,
) -> ResponseJson<serde_json::Value> {
    match state.leaderboard_engine.get_leaderboard().await {
        Ok(leaderboard_data) => {
            // Convert to the format expected by the frontend
            let mut metrics: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
            
            for agent in leaderboard_data.agents {
                metrics.insert(agent.agent_id.clone(), json!({
                    "agent_id": agent.agent_id,
                    "name": agent.agent_name,
                    "commits": agent.playbooks_committed,
                    "efficiency": agent.sovereign_score as f64 / 100.0, // Normalize for display
                    "durability": agent.successful_tasks, // Use successful tasks as durability proxy
                    "badges": agent.badges,
                    "sovereign_score": agent.sovereign_score,
                    "successful_tasks": agent.successful_tasks,
                    "resource_warnings": agent.resource_warnings,
                }));
            }
            
            ResponseJson(json!({
                "metrics": metrics,
                "generated_at": leaderboard_data.generated_at
            }))
        }
        Err(e) => {
            error!(error = %e, "Failed to fetch leaderboard data");
            // Return empty metrics on error
            ResponseJson(json!({
                "metrics": {},
                "error": e
            }))
        }
    }
}

#[derive(Debug, Clone)]
struct LLMSettings {
    temperature: f32,
    top_p: Option<f32>,
    max_tokens: Option<u32>,
    max_memory: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkScanPort {
    pub port: u16,
    pub protocol: String,
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkScanHost {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipv4: Option<String>,
    #[serde(default)]
    pub hostnames: Vec<String>,
    #[serde(default)]
    pub ports: Vec<NetworkScanPort>,
    pub is_agi_core_node: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkScanResult {
    pub target: String,
    pub timestamp: String,
    pub scanned_ports: Vec<u16>,
    pub hosts: Vec<NetworkScanHost>,
}

fn is_private_ipv4(ip: Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    a == 10
        || (a == 172 && (16..=31).contains(&b))
        || (a == 192 && b == 168)
        || (a == 169 && b == 254) // link-local
}

// Security gate helper functions
fn env_var_enabled(name: &str) -> bool {
    matches!(
        env::var(name)
            .unwrap_or_default()
            .to_lowercase()
            .as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn env_var_token(name: &str) -> Option<String> {
    let t = env::var(name).ok()?;
    let t = t.trim().to_string();
    if t.is_empty() {
        None
    } else {
        Some(t)
    }
}

// Security gates for research project access
fn allow_ipv6_network_scan() -> bool {
    env_var_enabled("ALLOW_IPV6_NETWORK_SCAN")
}

fn bypass_hitl_tool_exec() -> bool {
    env_var_enabled("BYPASS_HITL_TOOL_EXEC")
}

fn bypass_hitl_memory() -> bool {
    env_var_enabled("BYPASS_HITL_MEMORY")
}

fn bypass_hitl_kill_process() -> bool {
    env_var_enabled("BYPASS_HITL_KILL_PROCESS")
}

fn allow_restricted_commands() -> bool {
    env_var_enabled("ALLOW_RESTRICTED_COMMANDS")
}

fn bypass_email_teams_approval() -> bool {
    env_var_enabled("BYPASS_EMAIL_TEAMS_APPROVAL")
}

fn allow_arbitrary_port_scan() -> bool {
    env_var_enabled("ALLOW_ARBITRARY_PORT_SCAN")
}

fn public_network_scan_enabled() -> bool {
    env_var_enabled("ALLOW_PUBLIC_NETWORK_SCAN")
}

fn public_network_scan_hitl_token() -> Option<String> {
    env_var_token("NETWORK_SCAN_HITL_TOKEN")
}

fn parse_ipv4_target(target: &str) -> Result<Ipv4Addr, String> {
    let t = target.trim();
    if t.is_empty() {
        return Err("target must not be empty".to_string());
    }

    // Disallow obvious hostnames / URLs.
    if t.contains("://")
        || (t.contains('/')
            && t.split('/').nth(1).and_then(|s| s.parse::<u8>().ok()).is_none())
    {
        return Err(
            "network scanning target must be an IPv4 address or IPv4 CIDR (e.g., 192.168.1.0/24)".to_string(),
        );
    }
    
    // Security gate: Allow IPv6 if ALLOW_IPV6_NETWORK_SCAN is enabled
    if t.contains(':') {
        if allow_ipv6_network_scan() {
            // IPv6 scanning enabled - return a placeholder for now
            // Note: Full IPv6 support would require additional parsing logic
            return Err("IPv6 network scanning is enabled but full IPv6 parsing is not yet implemented. Use IPv4 addresses or CIDR notation for now.".to_string());
        }
        return Err("IPv6 targets are not allowed for network scanning. Set ALLOW_IPV6_NETWORK_SCAN=1 to enable (research project only)".to_string());
    }

    let ip_part = t.split('/').next().unwrap_or(t);
    let ip: Ipv4Addr = ip_part
        .parse()
        .map_err(|_| "invalid IPv4 target (expected e.g. 192.168.1.0/24)".to_string())?;
    Ok(ip)
}

/// Corporate guardrail (P64): allow internal scans by default.
/// Public scans require BOTH:
/// - `ALLOW_PUBLIC_NETWORK_SCAN=1`
/// - caller provides a matching HITL token (`NETWORK_SCAN_HITL_TOKEN`)
fn enforce_network_scan_policy(target: &str, hitl_token: Option<&str>) -> Result<(), String> {
    let ip = parse_ipv4_target(target)?;

    if is_private_ipv4(ip) {
        return Ok(());
    }

    if !public_network_scan_enabled() {
        return Err(
            "network scanning is restricted to internal research subnets (e.g., 192.168.x.x). Public scans require explicit HITL and ALLOW_PUBLIC_NETWORK_SCAN=1".to_string(),
        );
    }

    let expected = public_network_scan_hitl_token().ok_or_else(|| {
        "public network scan is enabled, but NETWORK_SCAN_HITL_TOKEN is not set; refusing public scan".to_string()
    })?;
    let provided = hitl_token.unwrap_or("").trim();
    if provided.is_empty() {
        return Err(
            "public target requested: HITL token required (provide hitl_token matching NETWORK_SCAN_HITL_TOKEN)".to_string(),
        );
    }
    if provided != expected {
        return Err("public target requested: invalid HITL token".to_string());
    }

    Ok(())
}

fn parse_nmap_xml(xml: &str) -> Result<Vec<NetworkScanHost>, String> {
    let mut reader = XmlReader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();

    let mut hosts: Vec<NetworkScanHost> = Vec::new();
    let mut current_host: Option<NetworkScanHost> = None;
    let mut current_port: Option<NetworkScanPort> = None;
    let mut in_hostnames = false;
    let mut in_ports = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(XmlEvent::Eof) => break,
            Ok(XmlEvent::Start(e)) => {
                match e.name().as_ref() {
                    b"host" => {
                        current_host = Some(NetworkScanHost {
                            ipv4: None,
                            hostnames: Vec::new(),
                            ports: Vec::new(),
                            is_agi_core_node: false,
                        });
                    }
                    b"hostnames" => in_hostnames = true,
                    b"ports" => in_ports = true,
                    b"address" => {
                        if let Some(h) = current_host.as_mut() {
                            let mut addr: Option<String> = None;
                            let mut addrtype: Option<String> = None;
                            for a in e.attributes().flatten() {
                                match a.key.as_ref() {
                                    b"addr" => addr = Some(a.unescape_value().unwrap_or_default().to_string()),
                                    b"addrtype" => {
                                        addrtype = Some(a.unescape_value().unwrap_or_default().to_string())
                                    }
                                    _ => {}
                                }
                            }
                            if addrtype.as_deref() == Some("ipv4") {
                                h.ipv4 = addr;
                            }
                        }
                    }
                    b"hostname" => {
                        if in_hostnames {
                            if let Some(h) = current_host.as_mut() {
                                for a in e.attributes().flatten() {
                                    if a.key.as_ref() == b"name" {
                                        let name = a.unescape_value().unwrap_or_default().to_string();
                                        if !name.trim().is_empty() {
                                            h.hostnames.push(name);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    b"port" => {
                        if in_ports {
                            let mut portid: Option<u16> = None;
                            let mut protocol: Option<String> = None;
                            for a in e.attributes().flatten() {
                                match a.key.as_ref() {
                                    b"portid" => {
                                        portid = a
                                            .unescape_value()
                                            .ok()
                                            .and_then(|v| v.parse::<u16>().ok());
                                    }
                                    b"protocol" => {
                                        protocol = Some(a.unescape_value().unwrap_or_default().to_string());
                                    }
                                    _ => {}
                                }
                            }
                            if let Some(p) = portid {
                                current_port = Some(NetworkScanPort {
                                    port: p,
                                    protocol: protocol.unwrap_or_else(|| "tcp".to_string()),
                                    state: "unknown".to_string(),
                                    service: None,
                                });
                            }
                        }
                    }
                    b"state" => {
                        if let Some(p) = current_port.as_mut() {
                            for a in e.attributes().flatten() {
                                if a.key.as_ref() == b"state" {
                                    p.state = a.unescape_value().unwrap_or_default().to_string();
                                }
                            }
                        }
                    }
                    b"service" => {
                        if let Some(p) = current_port.as_mut() {
                            for a in e.attributes().flatten() {
                                if a.key.as_ref() == b"name" {
                                    let s = a.unescape_value().unwrap_or_default().to_string();
                                    if !s.trim().is_empty() {
                                        p.service = Some(s);
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(XmlEvent::End(e)) => match e.name().as_ref() {
                b"hostnames" => in_hostnames = false,
                b"ports" => in_ports = false,
                b"port" => {
                    if let (Some(h), Some(p)) = (current_host.as_mut(), current_port.take()) {
                        // Keep all ports, but caller can filter.
                        h.ports.push(p);
                    }
                }
                b"host" => {
                    if let Some(mut h) = current_host.take() {
                        // Mark AGI core node if any of the orchestrator ports are open.
                        h.is_agi_core_node = h
                            .ports
                            .iter()
                            .any(|p| p.protocol == "tcp" && p.state == "open" && (8281..=8284).contains(&p.port));
                        hosts.push(h);
                    }
                }
                _ => {}
            },
            Err(e) => {
                return Err(format!("failed to parse nmap XML: {e}"));
            }
            _ => {}
        }

        buf.clear();
    }

    Ok(hosts)
}

async fn run_nmap_scan_xml(target: &str) -> Result<String, String> {
    // NOTE:
    // - On Unix, `-sS` requires root; we attempt `sudo -n` (no password prompt).
    // - On Windows, the process must be launched with Administrator privileges.

    let scanned_ports = "8281-8284";

    let mut cmd = if cfg!(target_os = "windows") {
        let mut c = TokioCommand::new("nmap");
        c.arg("-sS")
            .arg("-T4")
            .arg("-Pn")
            .arg("--max-retries")
            .arg("2")
            .arg("--host-timeout")
            .arg("15s")
            .arg("-p")
            .arg(scanned_ports)
            .arg("-oX")
            .arg("-")
            .arg(target);
        c
    } else {
        let mut c = TokioCommand::new("sudo");
        c.arg("-n")
            .arg("nmap")
            .arg("-sS")
            .arg("-T4")
            .arg("-Pn")
            .arg("--max-retries")
            .arg("2")
            .arg("--host-timeout")
            .arg("15s")
            .arg("-p")
            .arg(scanned_ports)
            .arg("-oX")
            .arg("-")
            .arg(target);
        c
    };

    cmd.kill_on_drop(true);
    let output = cmd.output().await.map_err(|e| format!("failed to launch nmap: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        if !cfg!(target_os = "windows") && stderr.to_lowercase().contains("password") {
            return Err(
                "nmap requires elevated privileges. Configure sudoers NOPASSWD for agi-orchestrator (see backend-rust-orchestrator/config/sudoers.*)".to_string(),
            );
        }
        return Err(format!(
            "nmap scan failed (status={:?}). stderr: {}",
            output.status.code(),
            truncate_for_log(&stderr, 4_000)
        ));
    }

    if stdout.trim().is_empty() {
        return Err(format!(
            "nmap returned empty output. stderr: {}",
            truncate_for_log(&stderr, 4_000)
        ));
    }

    Ok(stdout)
}

/// Deterministic mock planning used for local E2E runs.
///
/// This mirrors the assumptions in [`tests/e2e_test_script.md`](tests/e2e_test_script.md:1).
fn llm_plan_mock(user_message: &str) -> LLMAction {
    let msg = user_message.to_lowercase();

    // Deterministic system-health routing.
    if msg.contains("ram")
        || msg.contains("memory")
        || msg.contains("cpu")
        || msg.contains("process")
        || msg.contains("slow")
        || msg.contains("performance")
    {
        return LLMAction::ActionInspectSystem {};
    }

    if msg.contains("list") && msg.contains("record") {
        return LLMAction::ActionListRecordings {
            twin_id: None,
            limit: Some(20),
        };
    }

    // Tool signals
    if msg.contains("write") && msg.contains("file") {
        // For Windows, this will run under `cmd /C` via the tools service `command_exec` bridge.
        let mut args = HashMap::new();
        args.insert("cmd".to_string(), "echo hello world> test.txt".to_string());
        return LLMAction::ActionTool {
            tool_name: "command_exec".to_string(),
            args,
        };
    }

    // Memory signals
    if msg.contains("search") || msg.contains("find") || msg.contains("memory") {
        return LLMAction::ActionMemory {
            query: user_message.to_string(),
        };
    }

    // Default
    LLMAction::ActionResponse {
        content: format!("I understand you said: '{}'.", user_message),
    }
}

fn pending_key(twin_id: &str, session_id: &str, namespace: &str) -> String {
    format!("{}::{}::{}", twin_id, session_id, namespace)
}

fn build_tool_args_vec(tool_name: &str, args: &HashMap<String, String>) -> Vec<String> {
    if tool_name == "command_exec" {
        let cmdline = args
            .get("cmd")
            .or_else(|| args.get("command"))
            .or_else(|| args.get("cmdline"))
            .cloned()
            .or_else(|| {
                // If there's exactly one arg, treat it as the command.
                if args.len() == 1 {
                    args.values().next().cloned()
                } else {
                    None
                }
            })
            .unwrap_or_default();
        return vec![cmdline];
    }

    // Stable ordering for test determinism.
    let mut kvs: Vec<(String, String)> = args.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    kvs.sort_by(|a, b| a.0.cmp(&b.0));
    kvs.into_iter().map(|(k, v)| format!("{}={}", k, v)).collect()
}

fn is_supported_tool_name(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "command_exec" | "file_write" | "vector_query" | "run_command" | "read_file" | "write_file" | "systemctl" | "manage_service" | "get_logs" | "agi-doctor" | "github_tool_finder" | "archive_audit_report" | "search_audit_history"
    )
}

/// Check if a tool is a system tool that should be executed directly in the orchestrator
/// rather than through the Tools Service.
pub fn is_system_tool(tool_name: &str) -> bool {
    matches!(tool_name, "run_command" | "read_file" | "write_file" | "systemctl" | "manage_service" | "get_logs" | "agi-doctor" | "github_tool_finder" | "archive_audit_report" | "search_audit_history")
}

/// Execute a system tool directly in the orchestrator.
pub async fn execute_system_tool(
    tool_name: &str,
    args: &HashMap<String, String>,
) -> Result<String, String> {
    match tool_name {
        "run_command" => {
            let cmd = args
                .get("cmd")
                .or_else(|| args.get("command"))
                .or_else(|| args.get("cmdline"))
                .ok_or_else(|| "Missing 'cmd' parameter for run_command".to_string())?;
            run_command(cmd.clone()).await
        }
        "read_file" => {
            let path = args
                .get("path")
                .ok_or_else(|| "Missing 'path' parameter for read_file".to_string())?;
            read_file(path.clone()).await
        }
        "write_file" => {
            let path = args
                .get("path")
                .ok_or_else(|| "Missing 'path' parameter for write_file".to_string())?;
            let content = args
                .get("content")
                .ok_or_else(|| "Missing 'content' parameter for write_file".to_string())?;
            write_file(path.clone(), content.clone()).await?;
            Ok(format!("File '{}' written successfully", path))
        }
        "systemctl" | "manage_service" => {
            let action = args
                .get("action")
                .ok_or_else(|| "Missing 'action' parameter for manage_service".to_string())?;
            let service = args
                .get("service")
                .or_else(|| args.get("service_name"))
                .ok_or_else(|| "Missing 'service' parameter for manage_service".to_string())?;
            crate::tools::system::manage_service(service, action).await
        }
        "get_logs" => {
            let service = args
                .get("service")
                .or_else(|| args.get("service_name"))
                .ok_or_else(|| "Missing 'service' parameter for get_logs".to_string())?;
            crate::tools::system::get_logs(service).await
        }
        "agi-doctor" => {
            // agi-doctor takes no parameters - it runs a full diagnostic
            match crate::tools::doctor::agi_doctor().await {
                Ok(result) => Ok(result),
                Err(error_json) => {
                    // Even errors are returned as JSON, so we return them as-is
                    // The orchestrator can parse the JSON error object
                    Ok(error_json)
                }
            }
        }
        "github_tool_finder" => {
            let query = args
                .get("query")
                .or_else(|| args.get("search"))
                .ok_or_else(|| "Missing 'query' parameter for github_tool_finder".to_string())?;
            let language = args.get("language").cloned();
            let max_results = args
                .get("max_results")
                .and_then(|s| s.parse::<usize>().ok())
                .or_else(|| args.get("max_results").and_then(|s| s.parse::<usize>().ok()));
            
            match crate::tools::github_tool_finder::find_github_tool(query.clone(), language, max_results, None).await {
                Ok(tools) => {
                    if tools.is_empty() {
                        Ok("No tools found matching your query.".to_string())
                    } else {
                        let mut output = format!("Found {} tool(s) on GitHub:\n\n", tools.len());
                        for (idx, tool) in tools.iter().enumerate() {
                            output.push_str(&format!(
                                "{}. **{}** (⭐ {} stars, relevance: {:.2})\n",
                                idx + 1,
                                tool.tool_name,
                                tool.stars,
                                tool.relevance_score
                            ));
                            output.push_str(&format!("   Repository: {}\n", tool.repository));
                            output.push_str(&format!("   File: {}\n", tool.file_path));
                            if let Some(lang) = &tool.language {
                                output.push_str(&format!("   Language: {}\n", lang));
                            }
                            output.push_str(&format!("   URL: {}\n", tool.github_url));
                            output.push_str(&format!("   Raw: {}\n", tool.raw_url));
                            output.push_str(&format!("   Description: {}\n", tool.description));
                            output.push_str("\n");
                            
                            // Add proposal for the best match
                            if idx == 0 {
                                output.push_str(&format!(
                                    "\n💡 **Installation Proposal for Best Match:**\n{}\n",
                                    crate::tools::github_tool_finder::propose_tool_installation(tool)
                                ));
                            }
                        }
                        Ok(output)
                    }
                }
                Err(e) => Err(format!("GitHub tool search failed: {}", e)),
            }
        }
        "archive_audit_report" => {
            let report_json = args
                .get("report_json")
                .or_else(|| args.get("report"))
                .ok_or_else(|| "Missing 'report_json' parameter for archive_audit_report".to_string())?;
            
            let source_node = args.get("source_node");
            
            match crate::tools::audit_archiver::archive_audit_report(report_json, source_node.map(|s| s.as_str())).await {
                Ok(result) => Ok(result),
                Err(e) => Err(format!("Failed to archive audit report: {}", e)),
            }
        }
        "search_audit_history" => {
            let path = args
                .get("path")
                .ok_or_else(|| "Missing 'path' parameter for search_audit_history".to_string())?;
            let days = args
                .get("days")
                .and_then(|s| s.parse::<u32>().ok());
            let source_node = args.get("source_node");
            
            match crate::tools::audit_archiver::search_audit_history(path, days, source_node.map(|s| s.as_str())).await {
                Ok(reports) => {
                    let json_result = serde_json::to_string(&reports)
                        .map_err(|e| format!("Failed to serialize results: {}", e))?;
                    Ok(json_result)
                }
                Err(e) => Err(format!("Failed to search audit history: {}", e)),
            }
        }
        _ => Err(format!("Unknown system tool: {}", tool_name)),
    }
}

fn maybe_handle_builtin(user_message: &str) -> Option<LLMAction> {
    // Keep a tiny set of built-in handlers for requests that should NOT go to memory/tooling.
    // This avoids misclassification from the LLM (e.g., "today's date" -> ActionMemory).
    let msg = user_message.to_lowercase();
    let msg = msg.trim();

    // Date/time
    if msg.contains("today") && msg.contains("date")
        || msg.contains("what date is it")
        || msg.contains("current date")
        || msg.contains("what day is it")
        || msg.contains("today's date")
    {
        // Use server local time.
        let now = chrono::Local::now();
        let date = now.format("%Y-%m-%d").to_string();
        let weekday = now.format("%A").to_string();
        return Some(LLMAction::ActionResponse {
            content: format!("Today's date is {date} ({weekday})."),
        });
    }

    None
}

fn parse_create_project_chat(user_message: &str) -> Option<String> {
    // Examples we want to catch:
    // - "Creating chats under Project Alpha"
    // - "Create chat under Project Alpha"
    // - "Create chats under: Project Alpha"
    let msg = user_message.trim();
    if msg.is_empty() {
        return None;
    }

    let lower = msg.to_lowercase();
    let needles = [
        "creating chats under",
        "create chats under",
        "creating chat under",
        "create chat under",
        "create chats under:",
        "create chat under:",
        "creating chats under:",
        "creating chat under:",
    ];

    for needle in needles {
        if let Some(idx) = lower.find(needle) {
            let after = msg[idx + needle.len()..].trim();
            let after = after.trim_matches(|c: char| c == ':' || c == '-' || c.is_whitespace());
            let after = after.trim_matches(|c: char| c == '.' || c == '!' || c == '?' || c == '"');
            if after.is_empty() {
                return None;
            }
            return Some(after.to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::parse_create_project_chat;

    #[test]
    fn parse_create_project_chat_variants() {
        assert_eq!(
            parse_create_project_chat("Creating chats under Project Alpha"),
            Some("Project Alpha".to_string())
        );
        assert_eq!(
            parse_create_project_chat("create chat under: Project Alpha"),
            Some("Project Alpha".to_string())
        );
        assert_eq!(
            parse_create_project_chat("Create chats under   Neural Sync  "),
            Some("Neural Sync".to_string())
        );
        assert_eq!(parse_create_project_chat("create chats under"), None);
        assert_eq!(parse_create_project_chat(""), None);
    }
}

fn is_system_query(user_message: &str) -> bool {
    let msg = user_message.to_lowercase();
    let system_keywords = [
        "ram",
        "memory",
        "cpu",
        "process",
        "processes",
        "slow",
        "disk",
        "reboot",
    ];
    system_keywords.iter().any(|k| msg.contains(k))
}

/// OpenRouter LLM planning function that uses real AI for decision-making
pub async fn llm_plan_openrouter(
    user_message: &str,
    twin_id: &str,
    state: &AppState,
    media_active: bool,
    user_name: Option<&str>,
    settings: Option<&LLMSettings>,
) -> Result<(LLMAction, String), String> {
    info!(
        user_message = %user_message,
        twin_id = %twin_id,
        "OpenRouter LLM planning"
    );

    // Always use the current, live system prompt template.
    // The template may include "{twin_id}" and "{user_name}" which will be substituted here.
    let template = state.system_prompt.get_template().await;
    let base = if template.trim().is_empty() {
        DEFAULT_SYSTEM_PROMPT_TEMPLATE.to_string()
    } else {
        template
    };
    let mut system_prompt = base.replace("{twin_id}", twin_id);
    
    // Replace user_name placeholder (default to "FG_User" if not provided)
    let user_display_name = user_name.unwrap_or("FG_User");
    system_prompt = system_prompt.replace("{user_name}", user_display_name);
    
    if media_active {
        system_prompt.push_str("\n\n[CONTEXT: MULTI-MODAL ACTIVE]\n");
        system_prompt.push_str("media_active=true - The operator is currently recording voice/video and/or sharing their screen in real-time.\n");
        system_prompt.push_str("- You are aware that live multi-modal input (audio/video/screen) is being captured.\n");
        system_prompt.push_str("- You can reference this in your responses (e.g., 'I see you're currently recording...').\n");
        system_prompt.push_str("- Use ActionListRecordings to discover and reference stored recordings from previous sessions.\n");
        system_prompt.push_str("- Provide context-aware responses that account for the visual/audio information being captured.\n");
    }

    // Apply per-user personalization overlay (style/tone). This does not grant new capabilities.
    let overlay = state.preferences.render_prompt_overlay(twin_id).await;
    if !overlay.trim().is_empty() {
        system_prompt.push_str("\n\n");
        system_prompt.push_str(&overlay);
    }

    // Build the API request body
    let mut payload = serde_json::Map::new();
    payload.insert("model".to_string(), json!(state.openrouter_model));
    payload.insert("messages".to_string(), json!([
        {
            "role": "system",
            "content": system_prompt
        },
        {
            "role": "user",
            "content": user_message
        }
    ]));
    payload.insert("response_format".to_string(), json!({
        "type": "json_object"
    }));
    payload.insert("temperature".to_string(), json!(settings.map(|s| s.temperature).unwrap_or(0.1)));
    
    // Add optional parameters only if provided
    if let Some(s) = settings {
        if let Some(top_p) = s.top_p {
            payload.insert("top_p".to_string(), json!(top_p));
        }
        if let Some(max_tokens) = s.max_tokens {
            payload.insert("max_tokens".to_string(), json!(max_tokens));
        }
    }
    
    let payload = serde_json::Value::Object(payload);

    // Make the API call to OpenRouter
    let response = state
        .http_client
        .post(&state.openrouter_url)
        .header("Authorization", format!("Bearer {}", state.openrouter_api_key))
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

    // Parse the response
    let api_response: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse OpenRouter response: {}", e))?;

    let content = extract_openrouter_content(&api_response)?;

    info!(
        content = %content,
        "Received LLM response from OpenRouter"
    );

    // Parse the structured JSON into LLMAction.
    // Some OpenRouter upstream models ignore `response_format` and return plain text.
    // In that case we degrade gracefully to an ActionResponse rather than failing the whole request.
    match serde_json::from_str::<LLMAction>(&content).or_else(|e1| {
        if let Some(snippet) = extract_first_json_object(&content) {
            serde_json::from_str::<LLMAction>(snippet)
        } else {
            Err(e1)
        }
    }) {
        Ok(action) => Ok((action, content.to_string())),
        Err(e) => {
            warn!(
                error = %e,
                content = %truncate_for_log(&content, 2_000),
                "Planner returned non-JSON; falling back to ActionResponse"
            );
            let fallback = LLMAction::ActionResponse {
                content: content.clone(),
            };
            let raw = serde_json::to_string(&fallback).unwrap_or_else(|_| "{}".to_string());
            Ok((fallback, raw))
        }
    }
}

async fn openrouter_chat_json(
    state: &AppState,
    system_prompt: &str,
    user_content: &str,
    temperature: f32,
) -> Result<String, String> {
    let payload = json!({
        "model": state.openrouter_model,
        "messages": [
            {
                "role": "system",
                "content": system_prompt,
            },
            {
                "role": "user",
                "content": user_content,
            }
        ],
        "response_format": {
            "type": "json_object"
        },
        "temperature": temperature
    });

    let response = state
        .http_client
        .post(&state.openrouter_url)
        .header("Authorization", format!("Bearer {}", state.openrouter_api_key))
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

    let content = extract_openrouter_content(&api_response)?;

    Ok(content.to_string())
}

#[derive(Clone)]
struct OrchestratorServiceImpl {
    state: Arc<AppState>,
}

#[tonic::async_trait]
impl OrchestratorService for OrchestratorServiceImpl {
    async fn summarize_transcript(
        &self,
        request: tonic::Request<GrpcSummarizeRequest>,
    ) -> Result<tonic::Response<GrpcSummarizeResponse>, tonic::Status> {
        let req = request.into_inner();
        let transcript = req.transcript_text;

        if transcript.trim().is_empty() {
            return Err(tonic::Status::invalid_argument(
                "transcript_text must not be empty",
            ));
        }
        if transcript.len() > 2_000_000 {
            return Err(tonic::Status::invalid_argument(
                "transcript_text too large (max 2,000,000 chars)",
            ));
        }

        if self.state.llm_provider != "openrouter" {
            return Err(tonic::Status::failed_precondition(format!(
                "Summarization requires LLM_PROVIDER=openrouter; current={}",
                self.state.llm_provider
            )));
        }

        let user_content = format!("Transcript:\n\n{}", transcript);

        let raw = openrouter_chat_json(&self.state, ANALYST_SYSTEM_PROMPT, &user_content, 0.2)
            .await
            .map_err(|e| {
                error!(error = %e, "SummarizeTranscript OpenRouter call failed");
                tonic::Status::unavailable(format!("LLM request failed: {}", e))
            })?;

        let insights: AnalystInsightsJson = serde_json::from_str(&raw).map_err(|e| {
            error!(error = %e, content = %raw, "SummarizeTranscript invalid JSON from LLM");
            tonic::Status::internal(format!(
                "LLM returned invalid JSON for insights: {}",
                e
            ))
        })?;

        if insights.summary.trim().is_empty() {
            return Err(tonic::Status::internal(
                "LLM returned JSON but `summary` was empty",
            ));
        }

        Ok(tonic::Response::new(GrpcSummarizeResponse {
            summary: insights.summary,
            key_decisions: insights.decisions,
            follow_up_tasks: insights.tasks,
        }))
    }
}

#[derive(Debug, Deserialize)]
struct TelemetryMediaListItem {
    filename: String,
    size_bytes: u64,
    stored_path: String,
    #[serde(default)]
    ts_ms: Option<u128>,
}

#[derive(Debug, Deserialize)]
struct TelemetryMediaListResponse {
    #[serde(default)]
    recordings: Vec<TelemetryMediaListItem>,
}

#[derive(Debug, Deserialize)]
struct NetworkScanLatestQuery {
    #[serde(default)]
    twin_id: String,
    #[serde(default)]
    namespace: String,
}

async fn handle_network_scan_latest(
    State(state): State<AppState>,
    axum::extract::Query(query): axum::extract::Query<NetworkScanLatestQuery>,
) -> Result<ResponseJson<NetworkScanResult>, StatusCode> {
    let ns = if query.namespace.trim().is_empty() {
        "default".to_string()
    } else {
        query.namespace
    };
    let twin = query.twin_id;
    if twin.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let key = pending_key(&twin, "_", &ns);

    let scans = state.last_network_scans.read().await;
    let Some(res) = scans.get(&key) else {
        return Err(StatusCode::NOT_FOUND);
    };
    Ok(ResponseJson(res.clone()))
}

#[derive(Debug, Deserialize)]
struct NetworkScanRequest {
    target: String,
    #[serde(default)]
    twin_id: String,
    #[serde(default)]
    namespace: String,
    #[serde(default)]
    hitl_token: Option<String>,
    #[serde(default)]
    ports: Option<String>, // Optional custom port range (e.g., "22,80,443" or "1-65535")
}

async fn handle_network_scan(
    State(state): State<AppState>,
    Json(req): Json<NetworkScanRequest>,
) -> Result<ResponseJson<NetworkScanResult>, StatusCode> {
    let ns = if req.namespace.trim().is_empty() {
        "default".to_string()
    } else {
        req.namespace
    };
    if req.twin_id.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    enforce_network_scan_policy(&req.target, req.hitl_token.as_deref()).map_err(|_| StatusCode::FORBIDDEN)?;

    // Security gate: Check if arbitrary port scanning is allowed
    let requested_ports = req.ports.clone();
    let ports_to_scan = if let Some(custom_ports) = requested_ports.clone() {
        if !allow_arbitrary_port_scan() {
            return Err(StatusCode::FORBIDDEN);
        }
        custom_ports
    } else {
        // Default to AGI core ports
        "8281-8284".to_string()
    };

    let mut hosts = network_scanner::run_xml_scan(&req.target, &ports_to_scan)
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    // Parse scanned ports for result
    let scanned_ports_vec: Vec<u16> = if allow_arbitrary_port_scan() && requested_ports.is_some() {
        // Parse custom port range (simplified - could be enhanced)
        let ports_str = requested_ports.unwrap_or_default();
        if ports_str.contains('-') {
            // Range like "1-65535"
            let parts: Vec<&str> = ports_str.split('-').collect();
            if parts.len() == 2 {
                if let (Ok(start), Ok(end)) = (parts[0].parse::<u16>(), parts[1].parse::<u16>()) {
                    (start..=end.min(65535)).collect()
                } else {
                    vec![8281, 8282, 8283, 8284]
                }
            } else {
                vec![8281, 8282, 8283, 8284]
            }
        } else if ports_str.contains(',') {
            // Comma-separated like "22,80,443"
            ports_str.split(',')
                .filter_map(|p| p.trim().parse::<u16>().ok())
                .collect()
        } else {
            vec![8281, 8282, 8283, 8284]
        }
    } else {
        vec![8281, 8282, 8283, 8284]
    };

    // `network_scanner` already filters to open ports, but keep this defensive.
    for h in hosts.iter_mut() {
        h.ports.retain(|p| p.state == "open");
        h.is_agi_core_node = h
            .ports
            .iter()
            .any(|p| p.protocol == "tcp" && (8281..=8284).contains(&p.port));
    }

    let result = NetworkScanResult {
        target: req.target.clone(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        scanned_ports: scanned_ports_vec,
        hosts,
    };

    let key = pending_key(&req.twin_id, "_", &ns);
    {
        let mut scans = state.last_network_scans.write().await;
        scans.insert(key, result.clone());
    }

    Ok(ResponseJson(result))
}

// Network peers API handlers
async fn handle_network_peers(
    State(state): State<AppState>,
) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    let peers = state.handshake_service.get_verified_peers().await;
    let quarantined = state.quarantine_manager.list_quarantined().await;

    // Mark quarantined peers
    let mut peer_list: Vec<serde_json::Value> = Vec::new();
    for peer in peers {
        let is_quarantined = quarantined.iter().any(|q| q.node_id == peer.node_id);
        peer_list.push(serde_json::json!({
            "node_id": peer.node_id,
            "software_version": peer.software_version,
            "manifest_hash": peer.manifest_hash,
            "remote_address": peer.remote_address,
            "status": if is_quarantined { "Quarantined" } else {
                match peer.status {
                    network::handshake::PeerStatus::Verified => "Verified",
                    network::handshake::PeerStatus::Pending => "Pending",
                    network::handshake::PeerStatus::Quarantined => "Quarantined",
                }
            },
            "last_seen": peer.last_seen,
        }));
    }

    Ok(ResponseJson(serde_json::json!({
        "peers": peer_list,
    })))
}

// Quarantine API handlers
#[derive(Debug, Deserialize)]
struct QuarantineRequest {
    node_id: String,
    ip_address: Option<String>,
    reason: String,
}

async fn handle_network_quarantine_list(
    State(state): State<AppState>,
) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    let entries = state.quarantine_manager.list_quarantined().await;
    let list: Vec<serde_json::Value> = entries.iter().map(|e| {
        serde_json::json!({
            "node_id": e.node_id,
            "ip_address": e.ip_address,
            "reason": e.reason,
            "timestamp": e.timestamp,
            "quarantined_by": e.quarantined_by,
        })
    }).collect();

    Ok(ResponseJson(serde_json::json!({
        "quarantined": list,
    })))
}

async fn handle_network_quarantine_add(
    State(state): State<AppState>,
    Json(req): Json<QuarantineRequest>,
) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    // Get local node_id from handshake service identity
    let local_node_id = state.handshake_service.identity.node_id.clone();

    state.quarantine_manager.quarantine_node(
        req.node_id.clone(),
        req.ip_address.clone(),
        req.reason.clone(),
        local_node_id,
    ).await.map_err(|e| {
        error!(error = %e, "Failed to quarantine node");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(ResponseJson(serde_json::json!({
        "success": true,
        "message": format!("Node {} quarantined", req.node_id),
    })))
}

async fn handle_network_quarantine_remove(
    State(state): State<AppState>,
    axum::extract::Path(node_id): axum::extract::Path<String>,
) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    state.quarantine_manager.reintegrate_node(&node_id).await
        .map_err(|e| {
            error!(error = %e, "Failed to reintegrate node");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(ResponseJson(serde_json::json!({
        "success": true,
        "message": format!("Node {} reintegrated", node_id),
    })))
}

// Fleet Manager API handlers

/// Handle fleet heartbeat from a node
async fn handle_fleet_heartbeat(
    State(state): State<AppState>,
    Json(req): Json<network::fleet::HeartbeatRequest>,
) -> Result<ResponseJson<network::fleet::HeartbeatResponse>, StatusCode> {
    // Get client IP address if available
    // Note: In production, you'd extract this from the request headers
    let ip_address = req.ip_address.clone();
    
    state.fleet_state.heartbeat(
        req.node_id.clone(),
        req.hostname.clone(),
        ip_address,
        req.software_version.clone(),
    ).await.map_err(|e| {
        error!(error = %e, "Failed to process heartbeat");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let fleet_size = state.fleet_state.list_nodes().await.len();

    Ok(ResponseJson(network::fleet::HeartbeatResponse {
        success: true,
        message: format!("Heartbeat received for node {}", req.node_id),
        fleet_size,
    }))
}

/// Get fleet status (all nodes)
async fn handle_fleet_status(
    State(state): State<AppState>,
) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    let nodes = state.fleet_state.list_nodes().await;
    let health = state.fleet_state.get_fleet_health().await;

    Ok(ResponseJson(serde_json::json!({
        "nodes": nodes,
        "health": health,
    })))
}

/// Get fleet health summary
async fn handle_fleet_health(
    State(state): State<AppState>,
) -> Result<ResponseJson<network::fleet::FleetHealth>, StatusCode> {
    let health = state.fleet_state.get_fleet_health().await;
    Ok(ResponseJson(health))
}

// Network topology API handler
async fn handle_network_topology(
    State(state): State<AppState>,
) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    let peers = state.handshake_service.get_verified_peers().await;
    let quarantined = state.quarantine_manager.list_quarantined().await;
    let quarantined_set: std::collections::HashSet<String> = quarantined
        .iter()
        .map(|q| q.node_id.clone())
        .collect();

    // Build nodes
    let mut nodes: Vec<serde_json::Value> = Vec::new();
    for peer in &peers {
        let is_quarantined = quarantined_set.contains(&peer.node_id);
        let status = if is_quarantined {
            "Quarantined"
        } else {
            match peer.status {
                network::handshake::PeerStatus::Verified => "Verified",
                network::handshake::PeerStatus::Pending => "Pending",
                network::handshake::PeerStatus::Quarantined => "Quarantined",
            }
        };

        nodes.push(serde_json::json!({
            "id": peer.node_id,
            "node_id": peer.node_id,
            "status": status,
            "software_version": peer.software_version,
            "manifest_hash": peer.manifest_hash,
            "remote_address": peer.remote_address,
            "last_seen": peer.last_seen,
        }));
    }

    // Build links (trust links between verified nodes)
    let mut links: Vec<serde_json::Value> = Vec::new();
    let verified_peers: Vec<_> = peers
        .iter()
        .filter(|p| {
            matches!(p.status, network::handshake::PeerStatus::Verified)
                && !quarantined_set.contains(&p.node_id)
        })
        .collect();

    // Create a mesh: connect each verified node to a few others
    for (i, source) in verified_peers.iter().enumerate() {
        let connections = std::cmp::min(3, verified_peers.len().saturating_sub(1));
        for j in 0..connections {
            let target_index = (i + j + 1) % verified_peers.len();
            let target = verified_peers[target_index];
            if source.node_id != target.node_id {
                links.push(serde_json::json!({
                    "source": source.node_id,
                    "target": target.node_id,
                    "type": "trust",
                }));
            }
        }
    }

    // Add weak links to pending nodes
    let pending_peers: Vec<_> = peers
        .iter()
        .filter(|p| matches!(p.status, network::handshake::PeerStatus::Pending))
        .collect();
    
    for pending in pending_peers {
        if !verified_peers.is_empty() {
            // Connect to first verified peer
            links.push(serde_json::json!({
                "source": verified_peers[0].node_id,
                "target": pending.node_id,
                "type": "weak",
            }));
        }
    }

    Ok(ResponseJson(serde_json::json!({
        "nodes": nodes,
        "links": links,
    })))
}

// Mesh health API handler
async fn handle_mesh_health(
    State(state): State<AppState>,
) -> Result<ResponseJson<analytics::mesh_health::MeshHealthReport>, StatusCode> {
    let report = state.mesh_health_service.get_report().await;
    Ok(ResponseJson(report))
}

// Compliance alerts API handler
async fn handle_compliance_alerts(
    State(state): State<AppState>,
) -> Result<ResponseJson<Vec<serde_json::Value>>, StatusCode> {
    // If immune_response is available in AppState, use it
    // Otherwise, return empty array (will be added when immune_response is integrated into AppState)
    // For now, we'll return an empty array as a placeholder
    // TODO: Add immune_response to AppState and use it here
    Ok(ResponseJson(vec![]))
}

#[derive(Debug, Deserialize)]
struct ConfigureProjectWatchRequest {
    project_id: String,
    project_name: String,
    watch_path: String,
}

async fn handle_configure_project_watch(
    State(state): State<AppState>,
    Json(req): Json<ConfigureProjectWatchRequest>,
) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    state.project_watcher
        .watch_project_folder(&req.project_id, &req.project_name, &req.watch_path)
        .await
        .map_err(|e| {
            error!(error = %e, project_id = %req.project_id, "Failed to configure project watch");
            StatusCode::BAD_REQUEST
        })?;
    
    Ok(ResponseJson(json!({
        "ok": true,
        "message": format!("Now watching folder for {}", req.project_name)
    })))
}

#[derive(Debug, Deserialize)]
struct GetWatchConfigsQuery {
    #[serde(default)]
    project_id: String,
}

async fn handle_get_watch_configs(
    State(state): State<AppState>,
    axum::extract::Query(query): axum::extract::Query<GetWatchConfigsQuery>,
) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    let configs = state.project_watcher.get_all_configs().await;
    
    let result: HashMap<String, serde_json::Value> = configs
        .into_iter()
        .map(|(id, (name, path))| {
            (
                id,
                json!({
                    "project_name": name,
                    "watch_path": path.to_string_lossy().to_string(),
                }),
            )
        })
        .collect();
    
    Ok(ResponseJson(json!(result)))
}

#[derive(Debug, Deserialize)]
struct GetProcessingStatsQuery {
    #[serde(default)]
    project_id: String,
}

async fn handle_get_processing_stats(
    State(state): State<AppState>,
    axum::extract::Query(query): axum::extract::Query<GetProcessingStatsQuery>,
) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    let project_id = if query.project_id.is_empty() {
        None
    } else {
        Some(query.project_id.as_str())
    };
    
    let stats = state.project_watcher.get_processing_stats(project_id).await;
    Ok(ResponseJson(json!(stats)))
}

// --- Email/Teams Monitoring Handlers ---

#[derive(Debug, Deserialize)]
struct OAuthConfigRequest {
    client_id: String,
    client_secret: String,
    tenant_id: String,
    user_email: String,
    user_name: String,
    redirect_uri: String,
}

#[derive(Debug, Deserialize)]
struct OAuthTokenRequest {
    access_token: String,
    refresh_token: Option<String>,
}

async fn handle_configure_email_teams(
    State(state): State<AppState>,
    Json(config): Json<OAuthConfigRequest>,
) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    let monitor = EmailTeamsMonitor::new(
        config.client_id,
        config.client_secret,
        config.tenant_id,
        config.user_email,
        config.user_name,
        config.redirect_uri,
    );

    *state.email_teams_monitor.write().await = Some(monitor);

    Ok(ResponseJson(json!({
        "ok": true,
        "message": "Email/Teams monitor configured. Complete OAuth flow to activate."
    })))
}

async fn handle_set_oauth_tokens(
    State(state): State<AppState>,
    Json(tokens): Json<OAuthTokenRequest>,
) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    let mut monitor_guard = state.email_teams_monitor.write().await;
    if let Some(ref mut monitor) = *monitor_guard {
        monitor.set_access_token(tokens.access_token, tokens.refresh_token).await;
        Ok(ResponseJson(json!({
            "ok": true,
            "message": "OAuth tokens set successfully"
        })))
    } else {
        Err(StatusCode::BAD_REQUEST)
    }
}

#[derive(Debug, Deserialize)]
struct ExchangeTokenRequest {
    code: String,
}

async fn handle_exchange_token(
    State(state): State<AppState>,
    Json(request): Json<ExchangeTokenRequest>,
) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    let monitor_guard = state.email_teams_monitor.read().await;
    if let Some(ref monitor) = *monitor_guard {
        match monitor.exchange_code_for_token(&request.code).await {
            Ok((access_token, refresh_token)) => {
                Ok(ResponseJson(json!({
                    "ok": true,
                    "access_token": access_token,
                    "refresh_token": refresh_token,
                    "message": "Token exchange successful"
                })))
            }
            Err(e) => {
                error!("Failed to exchange token: {}", e);
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    } else {
        Err(StatusCode::BAD_REQUEST)
    }
}

async fn handle_check_emails(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    let filter_unread = params.get("unread").map(|v| v == "true").unwrap_or(true);
    
    let monitor_guard = state.email_teams_monitor.read().await;
    if let Some(ref monitor) = *monitor_guard {
        match monitor.check_new_emails(filter_unread).await {
            Ok(emails) => {
                let emails_json: Vec<serde_json::Value> = emails.iter()
                    .map(|e| json!({
                        "id": e.id,
                        "subject": e.subject,
                        "from": {
                            "name": e.from.name,
                            "address": e.from.address
                        },
                        "received_date_time": e.received_date_time.to_rfc3339(),
                        "is_read": e.is_read,
                        "importance": e.importance,
                        "has_attachments": e.has_attachments,
                        "body_preview": e.body.content.chars().take(200).collect::<String>()
                    }))
                    .collect();
                Ok(ResponseJson(json!({
                    "ok": true,
                    "emails": emails_json,
                    "count": emails_json.len()
                })))
            }
            Err(e) => {
                error!("Failed to check emails: {}", e);
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    } else {
        Err(StatusCode::BAD_REQUEST)
    }
}

async fn handle_check_teams(
    State(state): State<AppState>,
) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    let monitor_guard = state.email_teams_monitor.read().await;
    if let Some(ref monitor) = *monitor_guard {
        match monitor.check_teams_messages().await {
            Ok(messages) => {
                let messages_json: Vec<serde_json::Value> = messages.iter()
                    .map(|m| json!({
                        "id": m.id,
                        "chat_id": m.chat_id,
                        "channel_id": m.channel_id,
                        "from": {
                            "display_name": m.from.display_name,
                            "user_principal_name": m.from.user_principal_name
                        },
                        "body": m.body.content,
                        "created_date_time": m.created_date_time.to_rfc3339(),
                        "message_type": m.message_type,
                        "mentions": m.mentions.iter().map(|ment| json!({
                            "mention_text": ment.mention_text,
                            "mentioned": {
                                "display_name": ment.mentioned.display_name
                            }
                        })).collect::<Vec<_>>()
                    }))
                    .collect();
                Ok(ResponseJson(json!({
                    "ok": true,
                    "messages": messages_json,
                    "count": messages_json.len()
                })))
            }
            Err(e) => {
                error!("Failed to check Teams messages: {}", e);
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    } else {
        Err(StatusCode::BAD_REQUEST)
    }
}

async fn handle_send_email_reply(
    State(state): State<AppState>,
    Json(payload): Json<serde_json::Value>,
) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    let email_id = payload.get("email_id").and_then(|v| v.as_str()).ok_or(StatusCode::BAD_REQUEST)?;
    let reply_body = payload.get("reply_body").and_then(|v| v.as_str()).ok_or(StatusCode::BAD_REQUEST)?;

    let monitor_guard = state.email_teams_monitor.read().await;
    if let Some(ref monitor) = *monitor_guard {
        match monitor.send_email_reply(email_id.to_string(), reply_body.to_string()).await {
            Ok(msg) => Ok(ResponseJson(json!({
                "ok": true,
                "message": msg
            }))),
            Err(e) => {
                error!("Failed to send email reply: {}", e);
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    } else {
        Err(StatusCode::BAD_REQUEST)
    }
}

async fn handle_send_teams_message(
    State(state): State<AppState>,
    Json(payload): Json<serde_json::Value>,
) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    let chat_id = payload.get("chat_id").and_then(|v| v.as_str()).ok_or(StatusCode::BAD_REQUEST)?;
    let message_content = payload.get("message_content").and_then(|v| v.as_str()).ok_or(StatusCode::BAD_REQUEST)?;

    let monitor_guard = state.email_teams_monitor.read().await;
    if let Some(ref monitor) = *monitor_guard {
        match monitor.send_teams_message(chat_id.to_string(), message_content.to_string()).await {
            Ok(msg) => Ok(ResponseJson(json!({
                "ok": true,
                "message": msg
            }))),
            Err(e) => {
                error!("Failed to send Teams message: {}", e);
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    } else {
        Err(StatusCode::BAD_REQUEST)
    }
}

async fn handle_email_trends(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    let period = params.get("period").map(|s| s.as_str()).unwrap_or("week");
    
    let monitor_guard = state.email_teams_monitor.read().await;
    if let Some(ref monitor) = *monitor_guard {
        match monitor.get_email_trends(period).await {
            Ok(trends) => {
                Ok(ResponseJson(json!({
                    "ok": true,
                    "trends": {
                        "period": trends.period,
                        "total_emails": trends.total_emails,
                        "unread_count": trends.unread_count,
                        "urgent_count": trends.urgent_count,
                        "top_senders": trends.from_top_senders.iter().map(|s| json!({
                            "email": s.email,
                            "name": s.name,
                            "count": s.count
                        })).collect::<Vec<_>>()
                    }
                })))
            }
            Err(e) => {
                error!("Failed to get email trends: {}", e);
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    } else {
        Err(StatusCode::BAD_REQUEST)
    }
}

// --- Orchestrator Logic ---

async fn handle_chat_request(
    State(state): State<AppState>,
    Json(request): Json<ChatRequest>,
) -> Result<ResponseJson<ChatResponse>, StatusCode> {
    let job_id = Uuid::new_v4();
    let namespace = request.namespace.unwrap_or_else(|| "default".to_string());
    let pkey = pending_key(&request.twin_id, &request.session_id, &namespace);

    info!(
        job_id = %job_id,
        twin_id = %request.twin_id,
        message = %request.message,
        "Received chat request"
    );

    // --- Tool authorization follow-ups from the UI ---
    // Frontend sends:
    // - `[TOOL_EXECUTED] <tool_name> - CONFIRMED`
    // - `[TOOL_DENIED] <tool_name>`
    if let Some(rest) = request.message.strip_prefix("[TOOL_EXECUTED]") {
        let tool_name = rest
            .trim()
            .split('-')
            .next()
            .unwrap_or_default()
            .trim()
            .to_string();

        let pending = {
            let mut pending_map = state.pending_tools.write().await;
            pending_map.remove(&pkey)
        };

        if let Some(pending) = pending {
            if pending.tool_name != tool_name {
                return Ok(ResponseJson(ChatResponse {
                    response: format!(
                        "Tool authorization mismatch: pending='{}' but received='{}'",
                        pending.tool_name, tool_name
                    ),
                    job_id: Some(job_id.to_string()),
                    actions_taken: vec!["tool_authorization_mismatch".to_string()],
                    status: "error".to_string(),
                    issued_command: None,
                    raw_orchestrator_decision: None,
                }));
            }

            info!(job_id = %job_id, tool_name = %tool_name, "Tool authorization received; executing tool");

            // Check if this is a system tool that should be executed directly
            if is_system_tool(&pending.tool_name) {
                match execute_system_tool(&pending.tool_name, &pending.args).await {
                    Ok(output) => {
                        let mut actions_taken = vec![
                            format!("Tool execution: {} authorized", tool_name),
                            format!("Tool execution: {} completed", tool_name),
                        ];
                        let response_message = format!(
                            "System tool '{}' executed successfully. Output:\n{}",
                            tool_name, output
                        );

                        return Ok(ResponseJson(ChatResponse {
                            response: response_message,
                            job_id: Some(job_id.to_string()),
                            actions_taken,
                            status: "completed".to_string(),
                            issued_command: None,
                            raw_orchestrator_decision: None,
                        }));
                    }
                    Err(e) => {
                        error!(job_id = %job_id, error = %e, "System tool execution failed");
                        return Ok(ResponseJson(ChatResponse {
                            response: format!("System tool '{}' execution failed: {}", tool_name, e),
                            job_id: Some(job_id.to_string()),
                            actions_taken: vec![format!("System tool execution failed: {}", tool_name)],
                            status: "error".to_string(),
                            issued_command: None,
                            raw_orchestrator_decision: None,
                        }));
                    }
                }
            }

            // For non-system tools, execute via Tools Service
            let args_vec = build_tool_args_vec(&pending.tool_name, &pending.args);
            let mut tools_client = state.tools_client.clone();
            let tool_request = tonic::Request::new(ExecutionRequest {
                command: pending.tool_name.clone(),
                args: args_vec,
                twin_id: request.twin_id.clone(),
                job_id: job_id.to_string(),
                namespace: pending.namespace.clone(),
                metadata: HashMap::new(),
            });

            match tools_client.request_execution(tool_request).await {
                Ok(response) => {
                    let exec_response: ExecutionResponse = response.into_inner();
                    let mut actions_taken = vec![format!("Tool execution: {} authorized", tool_name)];

                    let response_message = if exec_response.success {
                        actions_taken.push(format!("Tool execution: {} completed", tool_name));
                        format!(
                            "Tool '{}' executed successfully. Output:\n{}",
                            tool_name,
                            exec_response.stdout_logs.join("\n")
                        )
                    } else {
                        actions_taken.push(format!("Tool execution: {} failed", tool_name));
                        format!("Tool '{}' execution failed: {}", tool_name, exec_response.message)
                    };

                    return Ok(ResponseJson(ChatResponse {
                        response: response_message,
                        job_id: Some(job_id.to_string()),
                        actions_taken,
                        status: "completed".to_string(),
                        issued_command: None,
                        raw_orchestrator_decision: None,
                    }));
                }
                Err(e) => {
                    error!(job_id = %job_id, error = %e, "Tool execution failed");
                    return Ok(ResponseJson(ChatResponse {
                        response: format!("Tool execution failed: {}", e),
                        job_id: Some(job_id.to_string()),
                        actions_taken: vec!["Tool execution failed".to_string()],
                        status: "error".to_string(),
                        issued_command: None,
                        raw_orchestrator_decision: None,
                    }));
                }
            }
        }

        return Ok(ResponseJson(ChatResponse {
            response: "No pending tool request found to execute.".to_string(),
            job_id: Some(job_id.to_string()),
            actions_taken: vec!["no_pending_tool".to_string()],
            status: "error".to_string(),
            issued_command: None,
            raw_orchestrator_decision: None,
        }));
    }

    if let Some(rest) = request.message.strip_prefix("[TOOL_DENIED]") {
        let tool_name = rest.trim().to_string();
        {
            let mut pending_map = state.pending_tools.write().await;
            pending_map.remove(&pkey);
        }
        return Ok(ResponseJson(ChatResponse {
            response: format!("Tool '{}' execution denied.", tool_name),
            job_id: Some(job_id.to_string()),
            actions_taken: vec![format!("Tool denied: {}", tool_name)],
            status: "completed".to_string(),
            issued_command: None,
            raw_orchestrator_decision: None,
        }));
    }

    // --- Memory authorization follow-ups from the UI ---
    // Frontend sends:
    // - `[MEMORY_SHOWN] <memory_id>` (approved)
    // - `[MEMORY_DENIED] <memory_id>` (denied)
    if let Some(rest) = request.message.strip_prefix("[MEMORY_SHOWN]") {
        let memory_id = rest.trim().to_string();

        let pending = {
            let mut pending_map = state.pending_memories.write().await;
            pending_map.remove(&pkey)
        };

        if let Some(pending) = pending {
            if pending.memory_id != memory_id {
                return Ok(ResponseJson(ChatResponse {
                    response: format!(
                        "Memory authorization mismatch: pending='{}' but received='{}'",
                        pending.memory_id, memory_id
                    ),
                    job_id: Some(job_id.to_string()),
                    actions_taken: vec!["memory_authorization_mismatch".to_string()],
                    status: "error".to_string(),
                    issued_command: None,
                    raw_orchestrator_decision: None,
                }));
            }

            info!(job_id = %job_id, memory_id = %memory_id, "Memory authorization received; executing memory query");

            let mut memory_client = state.memory_client.clone();
            let memory_request = tonic::Request::new(QueryMemoryRequest {
                query: pending.query.clone(),
                namespace: pending.namespace.clone(),
                twin_id: request.twin_id.clone(),
                top_k: 10,
                memory_types: vec![],
            });

            match memory_client.query_memory(memory_request).await {
                Ok(response) => {
                    let memory_response: QueryMemoryResponse = response.into_inner();
                    let result_count = memory_response.results.len();

                    let response_message = format!(
                        "Found {} memory results for query '{}'. Top result: {}",
                        result_count,
                        pending.query,
                        memory_response
                            .results
                            .first()
                            .map(|r| r.content.as_str())
                            .unwrap_or("No results")
                    );

                    return Ok(ResponseJson(ChatResponse {
                        response: response_message,
                        job_id: Some(job_id.to_string()),
                        actions_taken: vec![format!("Memory query authorized: {} results", result_count)],
                        status: "completed".to_string(),
                        issued_command: None,
                        raw_orchestrator_decision: None,
                    }));
                }
                Err(e) => {
                    error!(job_id = %job_id, error = %e, "Memory query failed");
                    return Ok(ResponseJson(ChatResponse {
                        response: format!("Memory query failed: {}", e),
                        job_id: Some(job_id.to_string()),
                        actions_taken: vec![format!("Memory query failed: {}", e)],
                        status: "error".to_string(),
                        issued_command: None,
                        raw_orchestrator_decision: None,
                    }));
                }
            }
        }

        return Ok(ResponseJson(ChatResponse {
            response: "No pending memory request found to execute.".to_string(),
            job_id: Some(job_id.to_string()),
            actions_taken: vec!["no_pending_memory".to_string()],
            status: "error".to_string(),
            issued_command: None,
            raw_orchestrator_decision: None,
        }));
    }

    if let Some(rest) = request.message.strip_prefix("[MEMORY_DENIED]") {
        let memory_id = rest.trim().to_string();
        {
            let mut pending_map = state.pending_memories.write().await;
            pending_map.remove(&pkey);
        }

        return Ok(ResponseJson(ChatResponse {
            response: format!("Memory request '{}' denied.", memory_id),
            job_id: Some(job_id.to_string()),
            actions_taken: vec![format!("Memory denied: {}", memory_id)],
            status: "completed".to_string(),
            issued_command: None,
            raw_orchestrator_decision: None,
        }));
    }

    // Create job
    let job = Job {
        id: job_id,
        twin_id: request.twin_id.clone(),
        status: "processing".to_string(),
        progress: 0,
        logs: vec!["Job created".to_string()],
        created_at: chrono::Utc::now(),
    };

    {
        let mut queue = state.job_queue.write().await;
        queue.insert(job_id.to_string(), job.clone());
    }

    // Project chat creation is a UI-level action (client switches session_id).
    // We handle it deterministically here so it works regardless of the LLM model's JSON compliance.
    if let Some(project_name) = parse_create_project_chat(&request.message) {
        let issued_command = json!({
            "command": "create_project_chat",
            "project_name": project_name,
        });

        {
            let mut queue = state.job_queue.write().await;
            if let Some(job) = queue.get_mut(&job_id.to_string()) {
                job.status = "completed".to_string();
                job.progress = 100;
                job.logs.push("Job completed (create_project_chat builtin)".to_string());
            }
        }

        return Ok(ResponseJson(ChatResponse {
            response: "Creating a new chat under that project.".to_string(),
            job_id: Some(job_id.to_string()),
            actions_taken: vec!["create_project_chat".to_string()],
            status: "completed".to_string(),
            issued_command: Some(issued_command),
            raw_orchestrator_decision: Some(
                json!({
                    "action_type": "ActionResponse",
                    "details": {"content": "create_project_chat"}
                })
                .to_string(),
            ),
        }));
    }

    // Built-in handlers (bypass LLM planning entirely for certain queries)
    if let Some(action) = maybe_handle_builtin(&request.message) {
        let raw = serde_json::to_string(&action).unwrap_or_default();

        // Treat as a direct response.
        let mut actions_taken = Vec::new();
        let response_message = match action {
            LLMAction::ActionResponse { content } => {
                actions_taken.push("Direct response generated (builtin)".to_string());
                content
            }
            // Builtins should never return tool/memory.
            _ => {
                actions_taken.push("builtin_unexpected_action".to_string());
                "Unsupported builtin action".to_string()
            }
        };

        // Update job to completed
        {
            let mut queue = state.job_queue.write().await;
            if let Some(job) = queue.get_mut(&job_id.to_string()) {
                job.status = "completed".to_string();
                job.progress = 100;
                job.logs.push("Job completed (builtin)".to_string());
            }
        }

        return Ok(ResponseJson(ChatResponse {
            response: response_message,
            job_id: Some(job_id.to_string()),
            actions_taken,
            status: "completed".to_string(),
            issued_command: None,
            raw_orchestrator_decision: Some(raw),
        }));
    }

    // P58: Deterministic pre-router for system health queries.
    // This prevents the planner from hallucinating media recordings when the user is asking about system performance.
    let forced_system_action: Option<LLMAction> = if is_system_query(&request.message) {
        Some(LLMAction::ActionInspectSystem {})
    } else {
        None
    };

    // LLM Planning (we also keep the raw decision text for UI transparency)
    //
    // IMPORTANT: Do not silently fall back to the mock planner when OpenRouter fails.
    // That produces an "echo" response (`I understand you said: ...`) and masks the real issue.
    let (action, raw_decision): (LLMAction, String) = if let Some(forced) = forced_system_action {
        let raw = serde_json::to_string(&forced).unwrap_or_else(|_| "{}".to_string());
        (forced, raw)
    } else if state.llm_provider == "openrouter" {
        // Extract LLM settings from request
        let llm_settings = if request.temperature.is_some() || request.top_p.is_some() || request.max_tokens.is_some() || request.max_memory.is_some() {
            Some(LLMSettings {
                temperature: request.temperature.unwrap_or(0.1),
                top_p: request.top_p,
                max_tokens: request.max_tokens,
                max_memory: request.max_memory,
            })
        } else {
            None
        };
        
        match llm_plan_openrouter(&request.message, &request.twin_id, &state, request.media_active, request.user_name.as_deref(), llm_settings.as_ref()).await {
            Ok((action, raw)) => (action, raw),
            Err(e) => {
                error!(job_id = %job_id, error = %e, "LLM planning failed");

                // Update job to error
                {
                    let mut queue = state.job_queue.write().await;
                    if let Some(job) = queue.get_mut(&job_id.to_string()) {
                        job.status = "error".to_string();
                        job.progress = 100;
                        job.logs.push(format!("LLM planning failed: {}", e));
                    }
                }

                return Ok(ResponseJson(ChatResponse {
                    response: format!("Orchestrator LLM planning failed (OpenRouter): {}", e),
                    job_id: Some(job_id.to_string()),
                    actions_taken: vec!["llm_planning_failed".to_string()],
                    status: "error".to_string(),
                    issued_command: None,
                    raw_orchestrator_decision: None,
                }));
            }
        }
    } else {
        info!(job_id = %job_id, "Mock LLM planning");
        let action = llm_plan_mock(&request.message);
        let raw = serde_json::to_string(&action).unwrap_or_default();
        (action, raw)
    };

    let mut actions_taken = Vec::new();
    let mut response_message = String::new();
    let mut issued_command: Option<serde_json::Value> = None;
    let mut raw_orchestrator_decision: Option<String> = None;

    match action {
        LLMAction::ActionMemory { query } => {
            info!(
                job_id = %job_id,
                query = %query,
                "Memory action requested; awaiting user authorization"
            );

            let memory_id = Uuid::new_v4().to_string();
            {
                let mut pending_map = state.pending_memories.write().await;
                pending_map.insert(
                    pkey.clone(),
                    PendingMemoryCall {
                        memory_id: memory_id.clone(),
                        query: query.clone(),
                        namespace: namespace.clone(),
                    },
                );
            }

            actions_taken.push(format!("Memory authorization requested: {}", memory_id));
            response_message = "Authorization required to run a memory search. Please approve or deny in the UI.".to_string();

            issued_command = Some(json!({
                "command": "show_memory_page",
                "memory_id": memory_id,
                "query": query,
            }));

            raw_orchestrator_decision = Some(raw_decision);
        }

        LLMAction::ActionTool { tool_name, args } => {
            if !is_supported_tool_name(&tool_name) {
                // Defensive: if the LLM invents a tool name, do not surface it to the UI.
                error!(
                    job_id = %job_id,
                    tool_name = %tool_name,
                    "Unsupported tool requested by planner"
                );

                // Update job to completed (handled)
                {
                    let mut queue = state.job_queue.write().await;
                    if let Some(job) = queue.get_mut(&job_id.to_string()) {
                        job.status = "completed".to_string();
                        job.progress = 100;
                        job.logs.push("Job completed (unsupported tool)".to_string());
                    }
                }

                return Ok(ResponseJson(ChatResponse {
                    response: format!(
                        "Tool '{}' is not available. Supported tools: command_exec, file_write, vector_query, run_command, read_file, write_file, systemctl, manage_service, get_logs, github_tool_finder.",
                        tool_name
                    ),
                    job_id: Some(job_id.to_string()),
                    actions_taken: vec!["unsupported_tool_requested".to_string()],
                    status: "completed".to_string(),
                    issued_command: None,
                    raw_orchestrator_decision: None,
                }));
            }

            info!(
                job_id = %job_id,
                tool_name = %tool_name,
                "Tool action requested; awaiting user authorization"
            );

            // Store pending tool request for this (twin_id, session_id, namespace)
            {
                let mut pending_map = state.pending_tools.write().await;
                pending_map.insert(
                    pkey.clone(),
                    PendingToolCall {
                        tool_name: tool_name.clone(),
                        args: args.clone(),
                        namespace: namespace.clone(),
                    },
                );
            }

            actions_taken.push(format!("Tool authorization requested: {}", tool_name));
            response_message = format!(
                "Authorization required to execute tool '{}'. Please approve or deny in the UI.",
                tool_name
            );

            issued_command = Some(json!({
                "command": "execute_tool",
                "tool_name": tool_name,
                "arguments": args,
            }));

            raw_orchestrator_decision = Some(raw_decision);

            // NOTE: We do not execute the tool here. The UI will send `[TOOL_EXECUTED] ...`.
        }

        LLMAction::ActionResponse { content } => {
            info!(job_id = %job_id, "Generating direct response");
            response_message = content;
            actions_taken.push("Direct response generated".to_string());
        }

        LLMAction::ActionBuildTool { tool_name, tool_code } => {
            info!(
                job_id = %job_id,
                tool_name = %tool_name,
                "Build tool action requested; awaiting Build Service implementation"
            );

            // TODO: Implement Build Service (P38) integration
            // This should compile the Rust code and make it available as a new tool
            actions_taken.push(format!("Build tool requested: {}", tool_name));
            response_message = format!(
                "Tool creation requested for '{}'. Build Service (P38) integration pending. Tool code length: {} bytes.",
                tool_name,
                tool_code.len()
            );

            // Store the build request for future processing
            // In a full implementation, this would trigger the Build Service
            issued_command = Some(json!({
                "command": "build_tool",
                "tool_name": tool_name,
                "tool_code_length": tool_code.len(),
                "status": "pending_implementation"
            }));

            raw_orchestrator_decision = Some(raw_decision);
        }

        LLMAction::ActionListRecordings { twin_id, limit } => {
            let twin = twin_id.unwrap_or_else(|| request.twin_id.clone());
            let limit = limit.unwrap_or(20).clamp(1, 200);

            let list_url = format!("{}/v1/media/list", state.telemetry_url.trim_end_matches('/'));
            info!(job_id = %job_id, url = %list_url, twin_id = %twin, limit = limit, "Listing recordings via telemetry");

            let resp = state
                .http_client
                .get(&list_url)
                .query(&[("twin_id", twin.as_str()), ("limit", &limit.to_string())])
                .send()
                .await;

            match resp {
                Ok(r) if r.status().is_success() => {
                    let parsed: TelemetryMediaListResponse = r
                        .json()
                        .await
                        .map_err(|e| {
                            error!(job_id = %job_id, error = %e, "Failed to parse telemetry media list");
                            StatusCode::BAD_GATEWAY
                        })?;

                    actions_taken.push("list_recordings".to_string());

                    if parsed.recordings.is_empty() {
                        response_message = format!("No recordings found for twin_id='{}'.", twin);
                    } else {
                        let mut lines: Vec<String> = Vec::new();
                        for rec in parsed.recordings.iter().take(limit as usize) {
                            lines.push(format!("- {} ({} bytes) [{}]", rec.filename, rec.size_bytes, rec.stored_path));
                        }
                        response_message = format!("Recent recordings for twin_id='{}':\n{}", twin, lines.join("\n"));
                    }

                    raw_orchestrator_decision = Some(raw_decision);
                }
                Ok(r) => {
                    let status = r.status();
                    let body = r.text().await.unwrap_or_default();
                    error!(job_id = %job_id, status = %status, body = %body, "Telemetry media list failed");
                    response_message = format!("Telemetry media list failed (status={}).", status);
                    actions_taken.push("list_recordings_failed".to_string());
                    raw_orchestrator_decision = Some(raw_decision);
                }
                Err(e) => {
                    error!(job_id = %job_id, error = %e, "Telemetry media list request failed");
                    response_message = format!("Telemetry media list request failed: {}", e);
                    actions_taken.push("list_recordings_failed".to_string());
                    raw_orchestrator_decision = Some(raw_decision);
                }
            }
        }

        LLMAction::ActionInspectSystem {} => {
            actions_taken.push("inspect_system".to_string());

            match get_system_snapshot().await {
                Ok(snapshot) => {
                    let used_mib = snapshot.memory.used_kib as f64 / 1024.0;
                    let total_mib = snapshot.memory.total_kib as f64 / 1024.0;
                    let cpu_global = snapshot.cpu.global_usage_percent;

                    let mut lines: Vec<String> = Vec::new();
                    lines.push(format!(
                        "Memory: {:.1} MiB / {:.1} MiB used ({:.1}%)",
                        used_mib,
                        total_mib,
                        if total_mib > 0.0 { (used_mib / total_mib) * 100.0 } else { 0.0 }
                    ));
                    lines.push(format!("CPU: {:.1}% global", cpu_global));

                    let per_core = snapshot
                        .cpu
                        .per_core_usage_percent
                        .iter()
                        .enumerate()
                        .map(|(i, u)| format!("core{}={:.1}%", i, u))
                        .collect::<Vec<_>>()
                        .join(", ");
                    if !per_core.is_empty() {
                        lines.push(format!("CPU cores: {}", per_core));
                    }

                    lines.push("Top 10 processes by memory:".to_string());
                    for p in snapshot.top_processes.iter() {
                        let mib = p.memory_kib as f64 / 1024.0;
                        lines.push(format!("- {} (PID {}): {:.1} MiB", p.name, p.pid, mib));
                    }

                    response_message = lines.join("\n");
                }
                Err(e) => {
                    actions_taken.push("inspect_system_failed".to_string());
                    response_message = format!("Failed to inspect system: {}", e);
                }
            }

            raw_orchestrator_decision = Some(raw_decision);
        }

        LLMAction::ActionKillProcess { pid } => {
            // P59: Safety + HITL gate. We do NOT kill immediately; we require tool authorization.
            let current_pid = std::process::id();
            if pid <= 4 || pid == current_pid {
                actions_taken.push("kill_process_blocked".to_string());
                response_message = format!(
                    "Refusing to terminate PID {} (blocked by safety rules).",
                    pid
                );
                raw_orchestrator_decision = Some(raw_decision);
                // Continue to job completion below.
            } else {
                let kill_cmd = if cfg!(target_os = "windows") {
                    format!("taskkill /PID {} /F", pid)
                } else {
                    format!("kill -9 {}", pid)
                };

                let mut args: HashMap<String, String> = HashMap::new();
                args.insert("cmd".to_string(), kill_cmd.clone());

                {
                    let mut pending_map = state.pending_tools.write().await;
                    pending_map.insert(
                        pkey.clone(),
                        PendingToolCall {
                            tool_name: "run_command".to_string(),
                            args: args.clone(),
                            namespace: namespace.clone(),
                        },
                    );
                }

                actions_taken.push(format!("kill_process_authorization_requested: pid={}", pid));
                response_message = format!(
                    "Authorization required to terminate process PID {}. Please approve or deny in the UI.",
                    pid
                );

                issued_command = Some(json!({
                    "command": "execute_tool",
                    "tool_name": "run_command",
                    "arguments": {"cmd": kill_cmd},
                    "purpose": "kill_process"
                }));

                raw_orchestrator_decision = Some(raw_decision);
            }
        }

        LLMAction::ActionSelfImprove { new_prompt } => {
            info!(
                job_id = %job_id,
                prompt_length = new_prompt.len(),
                "Self-improvement action requested"
            );

            // Persist + atomically swap the active prompt and store history.
            match state
                .system_prompt
                .update_with_history(new_prompt.clone(), Some("self_improve".to_string()))
                .await
            {
                Ok(_) => {
                    actions_taken.push("self_improve_applied".to_string());
                    response_message = "System prompt updated and reloaded for future requests.".to_string();
                    issued_command = Some(json!({
                        "command": "self_improve",
                        "status": "applied"
                    }));
                }
                Err(e) => {
                    error!(job_id = %job_id, error = %e, "Self-improvement failed");
                    actions_taken.push("self_improve_failed".to_string());
                    response_message = format!("Failed to update system prompt: {}", e);
                    issued_command = Some(json!({
                        "command": "self_improve",
                        "status": "error",
                        "error": e
                    }));
                }
            }

            raw_orchestrator_decision = Some(raw_decision);
        }

        LLMAction::ActionNetworkScan { target } => {
            actions_taken.push("network_scan".to_string());
            // NOTE: planner-triggered network scans are restricted to internal ranges only.
            enforce_network_scan_policy(&target, None).map_err(|e| {
                error!(job_id = %job_id, error = %e, target = %target, "Network scan blocked by guardrail");
                StatusCode::FORBIDDEN
            })?;

            let xml = match run_nmap_scan_xml(&target).await {
                Ok(x) => x,
                Err(e) => {
                    actions_taken.push("network_scan_failed".to_string());
                    response_message = format!("Network scan failed: {}", e);
                    raw_orchestrator_decision = Some(raw_decision);

                    // Update job to completed
                    {
                        let mut queue = state.job_queue.write().await;
                        if let Some(job) = queue.get_mut(&job_id.to_string()) {
                            job.status = "completed".to_string();
                            job.progress = 100;
                            job.logs.push("Job completed".to_string());
                        }
                    }

                    return Ok(ResponseJson(ChatResponse {
                        response: response_message,
                        job_id: Some(job_id.to_string()),
                        actions_taken,
                        status: "completed".to_string(),
                        issued_command,
                        raw_orchestrator_decision,
                    }));
                }
            };

            let mut hosts = parse_nmap_xml(&xml).map_err(|e| {
                error!(job_id = %job_id, error = %e, "Failed to parse nmap XML");
                StatusCode::BAD_GATEWAY
            })?;
            for h in hosts.iter_mut() {
                h.ports.retain(|p| p.state == "open");
                h.is_agi_core_node = h
                    .ports
                    .iter()
                    .any(|p| p.protocol == "tcp" && (8281..=8284).contains(&p.port));
            }

            let result = NetworkScanResult {
                target: target.clone(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                scanned_ports: vec![8281, 8282, 8283, 8284],
                hosts: hosts.clone(),
            };

            // Store for UI to fetch
            {
                let key = pending_key(&request.twin_id, "_", &namespace);
                let mut scans = state.last_network_scans.write().await;
                scans.insert(key, result.clone());
            }

            let host_count = result.hosts.len();
            let agi_nodes = result.hosts.iter().filter(|h| h.is_agi_core_node).count();
            response_message = format!(
                "Network scan completed for target '{}'. Hosts observed: {}. AGI Core Nodes (ports 8281-8284 open): {}.",
                target, host_count, agi_nodes
            );

            issued_command = Some(json!({
                "command": "network_scan_completed",
                "target": target,
                "host_count": host_count,
                "agi_core_nodes": agi_nodes,
            }));

            raw_orchestrator_decision = Some(raw_decision);
        }

        LLMAction::ActionMonitorEmail { filter_unread } => {
            actions_taken.push("monitor_email".to_string());
            
            let monitor_guard = state.email_teams_monitor.read().await;
            if let Some(ref monitor) = *monitor_guard {
                match monitor.check_new_emails(filter_unread.unwrap_or(true)).await {
                    Ok(emails) => {
                        if emails.is_empty() {
                            response_message = "No new emails addressed to you found.".to_string();
                        } else {
                            let mut lines = Vec::new();
                            for email in emails.iter().take(10) {
                                lines.push(format!(
                                    "- From: {} | Subject: {} | Received: {}",
                                    email.from.address,
                                    email.subject,
                                    email.received_date_time.format("%Y-%m-%d %H:%M")
                                ));
                            }
                            response_message = format!(
                                "Found {} email(s) addressed to you:\n{}",
                                emails.len(),
                                lines.join("\n")
                            );
                        }
                        actions_taken.push(format!("checked_emails: {} found", emails.len()));
                    }
                    Err(e) => {
                        error!(job_id = %job_id, error = %e, "Failed to check emails");
                        response_message = format!("Failed to check emails: {}", e);
                        actions_taken.push("monitor_email_failed".to_string());
                    }
                }
            } else {
                response_message = "Email/Teams monitor not configured. Please configure OAuth authentication first.".to_string();
                actions_taken.push("monitor_email_not_configured".to_string());
            }
            raw_orchestrator_decision = Some(raw_decision);
        }

        LLMAction::ActionSendEmail { original_email_id, reply_body } => {
            actions_taken.push("send_email".to_string());
            
            let monitor_guard = state.email_teams_monitor.read().await;
            if let Some(ref monitor) = *monitor_guard {
                match monitor.send_email_reply(original_email_id.clone(), reply_body.clone()).await {
                    Ok(msg) => {
                        response_message = format!("Email reply sent successfully: {}", msg);
                        actions_taken.push("email_sent".to_string());
                    }
                    Err(e) => {
                        error!(job_id = %job_id, error = %e, "Failed to send email");
                        response_message = format!("Failed to send email reply: {}", e);
                        actions_taken.push("send_email_failed".to_string());
                    }
                }
            } else {
                response_message = "Email/Teams monitor not configured. Please configure OAuth authentication first.".to_string();
                actions_taken.push("send_email_not_configured".to_string());
            }
            raw_orchestrator_decision = Some(raw_decision);
        }

        LLMAction::ActionQuarantineNode { node_id, reason } => {
            info!(
                job_id = %job_id,
                node_id = %node_id,
                reason = %reason,
                "Quarantine node action requested"
            );

            // Get IP address from peer if available
            let ip_address = {
                let peer = state.handshake_service.get_peer(&node_id).await;
                peer.map(|p| Some(p.remote_address)).unwrap_or(None)
            };

            let local_node_id = state.handshake_service.identity.node_id.clone();

            match state.quarantine_manager.quarantine_node(
                node_id.clone(),
                ip_address,
                reason.clone(),
                local_node_id,
            ).await {
                Ok(_) => {
                    response_message = format!("Node {} has been quarantined. Reason: {}", node_id, reason);
                    actions_taken.push(format!("quarantine_node:{}", node_id));
                }
                Err(e) => {
                    error!(job_id = %job_id, error = %e, "Failed to quarantine node");
                    response_message = format!("Failed to quarantine node {}: {}", node_id, e);
                    actions_taken.push("quarantine_failed".to_string());
                }
            }

            raw_orchestrator_decision = Some(raw_decision);
        }

        LLMAction::ActionMonitorTeams {} => {
            actions_taken.push("monitor_teams".to_string());
            
            let monitor_guard = state.email_teams_monitor.read().await;
            if let Some(ref monitor) = *monitor_guard {
                match monitor.check_teams_messages().await {
                    Ok(messages) => {
                        if messages.is_empty() {
                            response_message = "No new Teams messages found.".to_string();
                        } else {
                            let mut lines = Vec::new();
                            for msg in messages.iter().take(10) {
                                let mention_info = if !msg.mentions.is_empty() {
                                    format!(" ({} mention(s))", msg.mentions.len())
                                } else {
                                    String::new()
                                };
                                lines.push(format!(
                                    "- From: {} | {} | Created: {}{}",
                                    msg.from.display_name,
                                    msg.body.content.chars().take(50).collect::<String>(),
                                    msg.created_date_time.format("%Y-%m-%d %H:%M"),
                                    mention_info
                                ));
                            }
                            response_message = format!(
                                "Found {} Teams message(s):\n{}",
                                messages.len(),
                                lines.join("\n")
                            );
                        }
                        actions_taken.push(format!("checked_teams: {} found", messages.len()));
                    }
                    Err(e) => {
                        error!(job_id = %job_id, error = %e, "Failed to check Teams messages");
                        response_message = format!("Failed to check Teams messages: {}", e);
                        actions_taken.push("monitor_teams_failed".to_string());
                    }
                }
            } else {
                response_message = "Email/Teams monitor not configured. Please configure OAuth authentication first.".to_string();
                actions_taken.push("monitor_teams_not_configured".to_string());
            }
            raw_orchestrator_decision = Some(raw_decision);
        }

        LLMAction::ActionSendTeamsMessage { chat_id, message_content } => {
            actions_taken.push("send_teams_message".to_string());
            
            let monitor_guard = state.email_teams_monitor.read().await;
            if let Some(ref monitor) = *monitor_guard {
                match monitor.send_teams_message(chat_id.clone(), message_content.clone()).await {
                    Ok(msg) => {
                        response_message = format!("Teams message sent successfully: {}", msg);
                        actions_taken.push("teams_message_sent".to_string());
                    }
                    Err(e) => {
                        error!(job_id = %job_id, error = %e, "Failed to send Teams message");
                        response_message = format!("Failed to send Teams message: {}", e);
                        actions_taken.push("send_teams_failed".to_string());
                    }
                }
            } else {
                response_message = "Email/Teams monitor not configured. Please configure OAuth authentication first.".to_string();
                actions_taken.push("send_teams_not_configured".to_string());
            }
            raw_orchestrator_decision = Some(raw_decision);
        }

        LLMAction::ActionEmailTrends { period } => {
            actions_taken.push("email_trends".to_string());
            
            let monitor_guard = state.email_teams_monitor.read().await;
            if let Some(ref monitor) = *monitor_guard {
                match monitor.get_email_trends(&period).await {
                    Ok(trends) => {
                        let mut lines = Vec::new();
                        lines.push(format!("Email trends for the past {}:", period));
                        lines.push(format!("  Total emails: {}", trends.total_emails));
                        lines.push(format!("  Unread: {}", trends.unread_count));
                        lines.push(format!("  Urgent: {}", trends.urgent_count));
                        if !trends.from_top_senders.is_empty() {
                            lines.push("  Top senders:".to_string());
                            for sender in trends.from_top_senders.iter().take(5) {
                                lines.push(format!(
                                    "    - {} ({}): {} emails",
                                    sender.name.as_ref().unwrap_or(&sender.email),
                                    sender.email,
                                    sender.count
                                ));
                            }
                        }
                        response_message = lines.join("\n");
                        actions_taken.push("email_trends_analyzed".to_string());
                    }
                    Err(e) => {
                        error!(job_id = %job_id, error = %e, "Failed to get email trends");
                        response_message = format!("Failed to get email trends: {}", e);
                        actions_taken.push("email_trends_failed".to_string());
                    }
                }
            } else {
                response_message = "Email/Teams monitor not configured. Please configure OAuth authentication first.".to_string();
                actions_taken.push("email_trends_not_configured".to_string());
            }
            raw_orchestrator_decision = Some(raw_decision);
        }

        LLMAction::ActionSpawnAgent {
            name,
            mission,
            permissions,
        } => {
            actions_taken.push("spawn_agent".to_string());

            // Check if agent exists in library by name
            let mut agent_name = name.clone();
            let mut agent_mission = mission.clone();
            let mut agent_permissions = permissions.clone();
            let mut base_prompt_override: Option<String> = None;

            // Try to lookup agent in library
            if let Some(manifest) = state.agent_library.get_manifest(&name).await {
                info!(
                    agent_name = %manifest.name,
                    category = %manifest.category,
                    "Found agent in library, applying manifest configuration"
                );
                
                agent_name = manifest.name.clone();
                
                // Use manifest mission if mission is empty or generic
                if mission.trim().is_empty() || mission.trim().to_lowercase() == "default" {
                    agent_mission = manifest.description
                        .unwrap_or_else(|| format!("Specialized {} agent", manifest.category));
                } else {
                    agent_mission = mission.clone();
                }
                
                // Merge permissions: manifest permissions + provided permissions
                let mut merged_permissions = manifest.permissions.clone();
                for perm in &permissions {
                    if !merged_permissions.contains(perm) {
                        merged_permissions.push(perm.clone());
                    }
                }
                agent_permissions = merged_permissions;
                
                // Load base prompt from manifest
                match state.agent_library.get_base_prompt(&manifest.name).await {
                    Ok(prompt) => {
                        base_prompt_override = Some(prompt);
                        info!(
                            agent_name = %manifest.name,
                            "Loaded base prompt from manifest"
                        );
                    }
                    Err(e) => {
                        warn!(
                            agent_name = %manifest.name,
                            error = %e,
                            "Failed to load base prompt from manifest, using default"
                        );
                    }
                }
            }

            // Build inherited system prompt with Blue Flame overrides
            let mut inherited = build_effective_system_prompt(
                &state,
                &request.twin_id,
                request.user_name.as_deref(),
                request.media_active,
            )
            .await;

            // Apply base prompt override if available (from manifest)
            if let Some(base_prompt) = base_prompt_override {
                // Prepend the manifest's base prompt to the inherited Blue Flame prompt
                // This ensures the agent gets both the specialized prompt and Blue Flame leadership context
                inherited = format!("{}\n\n---\n\n{}", base_prompt, inherited);
            }

            match state
                .agent_factory
                .spawn_agent(agent_name, agent_mission, agent_permissions, inherited)
                .await
            {
                Ok(res) => {
                    response_message = format!(
                        "Spawned sub-agent successfully. agent_id={}",
                        res.agent_id
                    );
                    issued_command = Some(json!({
                        "command": "crew_list",
                        "agent_id": res.agent_id,
                    }));
                    raw_orchestrator_decision = Some(raw_decision);
                }
                Err(e) => {
                    response_message = format!("Failed to spawn sub-agent: {}", e);
                    actions_taken.push("spawn_agent_failed".to_string());
                    raw_orchestrator_decision = Some(raw_decision);
                }
            }
        }

        LLMAction::ActionSyncAgentLibrary {} => {
            actions_taken.push("sync_agent_library".to_string());
            
            // Use the new sync_library function from agents::loader
            match agents::loader::sync_library(&state.agent_library).await {
                Ok(msg) => {
                    response_message = msg.clone();
                    actions_taken.push("agent_library_synced".to_string());
                    
                    // Publish BroadcastDiscovery event
                    let discovery_event = bus::PhoenixEvent::BroadcastDiscovery {
                        source: "orchestrator".to_string(),
                        discovery_type: "agent_library_sync".to_string(),
                        details: "Agent library synchronized from GitHub repository".to_string(),
                        timestamp: chrono::Utc::now().to_rfc3339(),
                    };
                    state.publish_event(discovery_event);
                    
                    // List available agents from the library
                    let agents = state.agent_library.list_manifests().await;
                    if !agents.is_empty() {
                        let agent_list: Vec<String> = agents
                            .iter()
                            .map(|m| format!("- {} ({}), version {}", m.name, m.category, m.version))
                            .collect();
                        response_message.push_str(&format!("\n\nAvailable agent templates:\n{}", agent_list.join("\n")));
                    }
                }
                Err(e) => {
                    error!(job_id = %job_id, error = %e, "Failed to sync agent library");
                    response_message = format!("Failed to sync agent library: {}", e);
                    actions_taken.push("sync_agent_library_failed".to_string());
                }
            }
            raw_orchestrator_decision = Some(raw_decision);
        }

        LLMAction::ActionGitPush {
            repo_path,
            playbooks_dir,
            commit_message,
            remote_name,
            branch,
        } => {
            actions_taken.push("git_push".to_string());
            
            let repo_path = std::path::Path::new(&repo_path);
            let playbooks_dir = std::path::Path::new(&playbooks_dir);
            let remote_name = remote_name.unwrap_or_else(|| "origin".to_string());
            let branch = branch.unwrap_or_else(|| "main".to_string());
            
            match tools::git::GitOperations::commit_and_push_playbooks(
                repo_path,
                playbooks_dir,
                &commit_message,
                &remote_name,
                &branch,
                "Orchestrator",
                "orchestrator@digital-twin.local",
            ).await {
                Ok(_) => {
                    response_message = format!(
                        "Successfully committed and pushed playbooks to {}/{}",
                        remote_name, branch
                    );
                    actions_taken.push("playbooks_committed".to_string());
                }
                Err(e) => {
                    error!(job_id = %job_id, error = %e, "Failed to commit and push playbooks");
                    response_message = format!("Failed to commit and push playbooks: {}", e);
                    actions_taken.push("git_push_failed".to_string());
                }
            }
            raw_orchestrator_decision = Some(raw_decision);
        }
    }

    // Update job to completed
    {
        let mut queue = state.job_queue.write().await;
        if let Some(job) = queue.get_mut(&job_id.to_string()) {
            job.status = "completed".to_string();
            job.progress = 100;
            job.logs.push("Job completed".to_string());
        }
    }

    Ok(ResponseJson(ChatResponse {
        response: response_message,
        job_id: Some(job_id.to_string()),
        actions_taken,
        status: "completed".to_string(),
        issued_command,
        raw_orchestrator_decision,
    }))
}

async fn handle_memory_list(
    State(state): State<AppState>,
    Json(request): Json<MemoryListHttpRequest>,
) -> Result<ResponseJson<MemoryListHttpResponse>, StatusCode> {
    let mut memory_client = state.memory_client.clone();

    let page = request.page.max(1);
    let page_size = request.page_size.clamp(1, 1000);

    let grpc_req = tonic::Request::new(ListMemoriesRequest {
        namespace: request.namespace,
        page,
        page_size,
        twin_id: request.twin_id,
    });

    let resp: ListMemoriesResponse = memory_client
        .list_memories(grpc_req)
        .await
        .map_err(|e| {
            error!(error = %e, "Memory list RPC failed");
            StatusCode::BAD_GATEWAY
        })?
        .into_inner();

    let memories = resp
        .memories
        .into_iter()
        .map(|m| MemoryResultJson {
            id: m.id,
            timestamp: m.timestamp,
            content: m.content,
            agent_id: m.agent_id,
            risk_level: m.risk_level,
            similarity: m.similarity,
            memory_type: m.memory_type,
            metadata: m.metadata,
        })
        .collect();

    Ok(ResponseJson(MemoryListHttpResponse {
        memories,
        total_count: resp.total_count,
        total_pages: resp.total_pages,
        page: resp.page,
        page_size: resp.page_size,
        namespace: resp.namespace,
    }))
}

async fn handle_memory_delete(
    State(state): State<AppState>,
    Json(request): Json<MemoryDeleteHttpRequest>,
) -> Result<ResponseJson<MemoryDeleteHttpResponse>, StatusCode> {
    let mut memory_client = state.memory_client.clone();

    let grpc_req = tonic::Request::new(DeleteMemoryRequest {
        memory_id: request.memory_id,
        namespace: request.namespace,
    });

    let resp: DeleteMemoryResponse = memory_client
        .delete_memory(grpc_req)
        .await
        .map_err(|e| {
            error!(error = %e, "Memory delete RPC failed");
            StatusCode::BAD_GATEWAY
        })?
        .into_inner();

    Ok(ResponseJson(MemoryDeleteHttpResponse {
        success: resp.success,
        error_message: resp.error_message,
    }))
}

async fn handle_prompt_history(
    State(state): State<AppState>,
) -> ResponseJson<PromptHistoryHttpResponse> {
    let history = state.system_prompt.history().await;
    let entries = history
        .into_iter()
        .map(|e| PromptHistoryEntryHttp {
            id: e.id,
            timestamp: e.timestamp,
            previous_prompt: e.previous_prompt,
            new_prompt: e.new_prompt,
            change_summary: e.change_summary,
        })
        .collect();

    ResponseJson(PromptHistoryHttpResponse { entries })
}

async fn handle_summarize_transcript(
    State(state): State<AppState>,
    Json(request): Json<SummarizeTranscriptRequest>,
) -> ResponseJson<SummarizeTranscriptResponse> {
    info!(
        filename = %request.filename,
        transcript_length = request.transcript.len(),
        "Summarizing transcript"
    );

    // Analyst system prompt for extracting insights
    const ANALYST_SYSTEM_PROMPT: &str = r#"You are an AI Analyst specializing in extracting actionable insights from recorded conversations and research sessions.

Your task is to analyze the provided transcript and extract:
1. **Summary**: A concise 2-3 sentence overview of the main topics and outcomes discussed.
2. **Key Decisions**: A list of important decisions, conclusions, or commitments made during the session.
3. **Follow-up Tasks**: A list of actionable items, next steps, or tasks that were mentioned or implied.

Format your response as JSON with this exact structure:
{
  "summary": "Brief overview of the session...",
  "key_decisions": ["Decision 1", "Decision 2", ...],
  "follow_up_tasks": ["Task 1", "Task 2", ...]
}

Be concise but comprehensive. Focus on actionable insights that would be valuable for future reference."#;

    if state.llm_provider != "openrouter" {
        warn!("Summarization requires OpenRouter LLM provider; current provider: {}", state.llm_provider);
        return ResponseJson(SummarizeTranscriptResponse {
            success: false,
            insights: None,
            error: Some(format!("Summarization requires OpenRouter LLM provider; current: {}", state.llm_provider)),
        });
    }

    // Build the API request body
    let payload = json!({
        "model": state.openrouter_model,
        "messages": [
            {
                "role": "system",
                "content": ANALYST_SYSTEM_PROMPT
            },
            {
                "role": "user",
                "content": format!("Analyze this transcript:\n\n{}", request.transcript)
            }
        ],
        "response_format": {
            "type": "json_object"
        },
        "temperature": 0.3
    });

    // Make the API call to OpenRouter
    let response = match state
        .http_client
        .post(&state.openrouter_url)
        .header("Authorization", format!("Bearer {}", state.openrouter_api_key))
        .header("Content-Type", "application/json")
        .header("HTTP-Referer", "ferrellgas-agi-digital-twin")
        .json(&payload)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            error!(error = %e, "OpenRouter API request failed for transcript summarization");
            return ResponseJson(SummarizeTranscriptResponse {
                success: false,
                insights: None,
                error: Some(format!("OpenRouter API request failed: {}", e)),
            });
        }
    };

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        error!(status = %status, error = %error_text, "OpenRouter API returned error");
        return ResponseJson(SummarizeTranscriptResponse {
            success: false,
            insights: None,
            error: Some(format!("OpenRouter API returned error status {}: {}", status, error_text)),
        });
    }

    // Parse the response
    let api_response: serde_json::Value = match response.json().await {
        Ok(r) => r,
        Err(e) => {
            error!(error = %e, "Failed to parse OpenRouter response");
            return ResponseJson(SummarizeTranscriptResponse {
                success: false,
                insights: None,
                error: Some(format!("Failed to parse OpenRouter response: {}", e)),
            });
        }
    };

    let content = match extract_openrouter_content(&api_response) {
        Ok(c) => c,
        Err(e) => {
            error!(error = %e, "Failed to extract content from OpenRouter response");
            return ResponseJson(SummarizeTranscriptResponse {
                success: false,
                insights: None,
                error: Some(e),
            });
        }
    };

    // Parse the structured JSON into TranscriptInsights
    let insights: TranscriptInsights = match serde_json::from_str(&content) {
        Ok(i) => i,
        Err(e) => {
            error!(error = %e, content = %content, "Failed to parse insights JSON");
            return ResponseJson(SummarizeTranscriptResponse {
                success: false,
                insights: None,
                error: Some(format!(
                    "Failed to parse insights JSON: {}. Raw content: {}",
                    e,
                    truncate_for_log(&content, 8_000)
                )),
            });
        }
    };

    info!(
        filename = %request.filename,
        summary_length = insights.summary.len(),
        decisions_count = insights.key_decisions.len(),
        tasks_count = insights.follow_up_tasks.len(),
        "Transcript summarized successfully"
    );

    ResponseJson(SummarizeTranscriptResponse {
        success: true,
        insights: Some(insights),
        error: None,
    })
}

async fn handle_prompt_restore(
    State(state): State<AppState>,
    Json(request): Json<PromptRestoreHttpRequest>,
) -> Result<ResponseJson<PromptRestoreHttpResponse>, StatusCode> {
    let history = state.system_prompt.history().await;
    let Some(entry) = history.into_iter().find(|e| e.id == request.entry_id) else {
        return Ok(ResponseJson(PromptRestoreHttpResponse {
            success: false,
            message: "prompt history entry not found".to_string(),
        }));
    };

    state
        .system_prompt
        .update_with_history(
            entry.new_prompt,
            Some(format!("restore_from:{}", entry.id)),
        )
        .await
        .map_err(|e| {
            error!(error = %e, "Prompt restore failed");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(ResponseJson(PromptRestoreHttpResponse {
        success: true,
        message: "prompt restored".to_string(),
    }))
}

async fn handle_prompt_current(
    State(state): State<AppState>,
) -> ResponseJson<PromptCurrentHttpResponse> {
    let prompt = state.system_prompt.get_template().await;
    ResponseJson(PromptCurrentHttpResponse { prompt })
}

async fn handle_prompt_update(
    State(state): State<AppState>,
    Json(request): Json<PromptUpdateHttpRequest>,
) -> Result<ResponseJson<PromptUpdateHttpResponse>, StatusCode> {
    if request.new_prompt.len() > 200_000 {
        return Ok(ResponseJson(PromptUpdateHttpResponse {
            success: false,
            message: "new_prompt too large (max 200k chars)".to_string(),
        }));
    }

    let summary = if request.change_summary.trim().is_empty() {
        None
    } else {
        Some(request.change_summary)
    };

    state
        .system_prompt
        .update_with_history(request.new_prompt, summary)
        .await
        .map_err(|e| {
            error!(error = %e, "Prompt update failed");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(ResponseJson(PromptUpdateHttpResponse {
        success: true,
        message: "prompt updated".to_string(),
    }))
}

async fn handle_prompt_reset(
    State(state): State<AppState>,
) -> Result<ResponseJson<PromptResetHttpResponse>, StatusCode> {
    state
        .system_prompt
        .update_with_history(
            DEFAULT_SYSTEM_PROMPT_TEMPLATE.to_string(),
            Some("reset_to_default".to_string()),
        )
        .await
        .map_err(|e| {
            error!(error = %e, "Prompt reset failed");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(ResponseJson(PromptResetHttpResponse {
        success: true,
        message: "prompt reset to default".to_string(),
    }))
}

async fn health_check() -> ResponseJson<HealthResponse> {
    ResponseJson(HealthResponse {
        service: "backend-rust-orchestrator",
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

/// Find the .env file path using the same logic as load_dotenv
fn find_env_file() -> Option<PathBuf> {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let candidates: Vec<PathBuf> = vec![
        manifest_dir.join(".env"),
        manifest_dir
            .parent()
            .map(|p| p.join(".env"))
            .unwrap_or_else(|| PathBuf::from(".env")),
        PathBuf::from(".env"),
    ];

    for candidate in candidates {
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

/// Read .env file and return as key-value pairs
async fn read_env_file() -> Result<HashMap<String, String>, String> {
    let env_path = find_env_file().ok_or_else(|| "No .env file found".to_string())?;
    
    let content = fs::read_to_string(&env_path)
        .await
        .map_err(|e| format!("Failed to read .env file: {}", e))?;
    
    let mut env_vars = HashMap::new();
    
    for line in content.lines() {
        let line = line.trim();
        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        
        // Parse KEY=VALUE format
        if let Some(equal_pos) = line.find('=') {
            let key = line[..equal_pos].trim().to_string();
            let value = line[equal_pos + 1..].trim().to_string();
            // Remove quotes if present
            let value = value
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .map(|s| s.to_string())
                .unwrap_or(value);
            env_vars.insert(key, value);
        }
    }
    
    Ok(env_vars)
}

/// Update .env file with new values
async fn update_env_file(updates: HashMap<String, String>) -> Result<(), String> {
    let env_path = find_env_file().ok_or_else(|| "No .env file found".to_string())?;
    
    // Read existing content
    let content = fs::read_to_string(&env_path)
        .await
        .map_err(|e| format!("Failed to read .env file: {}", e))?;
    
    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
    let mut updated_keys = std::collections::HashSet::new();
    
    // Update existing variables
    for line in lines.iter_mut() {
        // Avoid borrowing `line` across a potential assignment to `*line`.
        let original = line.clone();
        let trimmed = original.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        
        if let Some(equal_pos) = trimmed.find('=') {
            let key = trimmed[..equal_pos].trim();
            if let Some(new_value) = updates.get(key) {
                // Preserve original formatting (spaces, quotes, etc.)
                let prefix = if original.chars().next().map(|c| c.is_whitespace()).unwrap_or(false) {
                    original
                        .chars()
                        .take_while(|c| c.is_whitespace())
                        .collect::<String>()
                } else {
                    String::new()
                };
                // Preserve quotes if original had them, otherwise add if value contains spaces
                let formatted_value = if new_value.contains(' ') && !new_value.starts_with('"') {
                    format!("\"{}\"", new_value)
                } else {
                    new_value.clone()
                };
                *line = format!("{}{}={}", prefix, key, formatted_value);
                updated_keys.insert(key.to_string());
            }
        }
    }
    
    // Add new variables that weren't in the file (append at end)
    let mut new_vars: Vec<String> = Vec::new();
    for (key, value) in &updates {
        if !updated_keys.contains(key) {
            let formatted_value = if value.contains(' ') && !value.starts_with('"') {
                format!("\"{}\"", value)
            } else {
                value.clone()
            };
            new_vars.push(format!("{}={}", key, formatted_value));
        }
    }
    
    if !new_vars.is_empty() {
        // Add a comment separator if there are existing lines
        if !lines.is_empty() && !lines.last().unwrap().trim().is_empty() {
            lines.push(String::new());
        }
        lines.extend(new_vars);
    }
    
    // Write back to file (preserve line endings)
    let new_content = if content.contains("\r\n") {
        lines.join("\r\n")
    } else {
        lines.join("\n")
    };
    fs::write(&env_path, new_content)
        .await
        .map_err(|e| format!("Failed to write .env file: {}", e))?;
    
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
struct EnvReadResponse {
    env_vars: HashMap<String, String>,
    env_file_path: String,
}

#[derive(Debug, Deserialize)]
struct EnvUpdateRequest {
    updates: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct EnvUpdateResponse {
    success: bool,
    message: String,
}

async fn handle_env_read() -> Result<ResponseJson<EnvReadResponse>, StatusCode> {
    match read_env_file().await {
        Ok(env_vars) => {
            let env_path = find_env_file()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "unknown".to_string());
            Ok(ResponseJson(EnvReadResponse {
                env_vars,
                env_file_path: env_path,
            }))
        }
        Err(e) => {
            error!(error = %e, "Failed to read .env file");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn handle_env_update(
    Json(request): Json<EnvUpdateRequest>,
) -> Result<ResponseJson<EnvUpdateResponse>, StatusCode> {
    if request.updates.is_empty() {
        return Ok(ResponseJson(EnvUpdateResponse {
            success: false,
            message: "No updates provided".to_string(),
        }));
    }
    
    match update_env_file(request.updates).await {
        Ok(_) => Ok(ResponseJson(EnvUpdateResponse {
            success: true,
            message: "Environment variables updated successfully".to_string(),
        })),
        Err(e) => {
            error!(error = %e, "Failed to update .env file");
            Ok(ResponseJson(EnvUpdateResponse {
                success: false,
                message: e,
            }))
        }
    }
}

/// System snapshot endpoint handler
async fn handle_system_snapshot() -> Result<ResponseJson<tools::SystemSnapshot>, StatusCode> {
    match get_system_snapshot().await {
        Ok(snapshot) => Ok(ResponseJson(snapshot)),
        Err(e) => {
            error!(error = %e, "Failed to get system snapshot");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Sync metrics endpoint handler
async fn handle_sync_metrics(
    State(state): State<AppState>,
) -> Result<ResponseJson<SyncMetricsResponse>, StatusCode> {
    let neural_sync = state.health_manager.calculate_neural_sync().await;
    let all_health = state.health_manager.get_all_service_health().await;
    
    let services: HashMap<String, String> = all_health
        .into_iter()
        .map(|(name, health)| {
            let status_str = match health.status {
                health::ServiceStatus::Online => "online".to_string(),
                health::ServiceStatus::Offline => "offline".to_string(),
                health::ServiceStatus::Repairing => "repairing".to_string(),
            };
            (name, status_str)
        })
        .collect();

    Ok(ResponseJson(SyncMetricsResponse {
        neural_sync,
        services,
    }))
}

/// Initialize Phoenix Auditor agent and register default scheduled task
async fn initialize_phoenix_auditor(
    agent_factory: &agents::factory::AgentFactory,
    scheduled_tasks: &Arc<tokio::sync::RwLock<api::phoenix_routes::ScheduledTaskStore>>,
    state: &AppState,
) -> Result<(), String> {
    use api::phoenix_routes::{ScheduledTask, TaskStatus, CreateScheduledTaskRequest};
    use cron::Schedule;
    use std::str::FromStr;
    
    // Build specialized system prompt for Phoenix Auditor
    let phoenix_auditor_system_prompt = r#"You are the **Phoenix Auditor**, a Security and Configuration Specialist agent.

## Your Mission
Your primary goal is to review local application directories (`/Applications` on macOS, `/opt` on Linux, `C:\Program Files` on Windows) for changes, security issues, and configuration anomalies.

## Core Responsibilities
1. **Filesystem Auditing:** Scan application directories for:
   - New or modified applications
   - Unusual file permissions
   - Configuration file changes
   - Security-related files (certificates, keys, configs)
   - Proprietary or unknown file formats

2. **Tool Discovery:** When you encounter an unknown file format (e.g., `.log`, `.cfg`, `.conf`, proprietary formats):
   - Use the `github_tool_finder` tool to search for parsers or analyzers
   - Search with queries like: "parse [file_format] log file" or "[file_format] configuration parser"
   - Review the discovered tools and propose installation if appropriate

3. **Reporting:** After each audit:
   - Summarize your findings in a structured format
   - Include: file paths, changes detected, security concerns, tool discovery results
   - Store your findings in the `agent_logs` Qdrant collection so the Matchmaker knows you are the expert on filesystem audits

## Tool Usage
You have access to the following tools via your task responses:
- `read_file`: Read configuration files and logs
- `run_command`: Execute system commands to list directories and check file properties
- `github_tool_finder`: Search GitHub for tools to parse unknown file formats
  - Usage: Request tool execution with `{"tool_name": "github_tool_finder", "args": {"query": "your search query", "language": "optional"}}`
- `archive_audit_report`: Archive your daily audit report to the Knowledge Atlas for trend analysis
  - Usage: `{"tool_name": "archive_audit_report", "args": {"report_json": "<your complete JSON report>"}}`
  - **CRITICAL**: Always call this tool AFTER generating your daily report JSON
- `search_audit_history`: Search historical audit reports for a specific file path to detect patterns
  - Usage: `{"tool_name": "search_audit_history", "args": {"path": "/path/to/file", "days": 30}}`
  - Use this BEFORE finalizing your report to check if issues are recurring

## Daily Audit Report Format (Narrative Structure)

When generating your Daily Audit Report, use the following **"Rising Action" narrative structure** to make your findings scannable for human review:

1. **Executive Pulse:** A 1-sentence status summary
   - Example: "System Nominal" or "Drift Detected" or "Security Anomaly Found"

2. **Rising Action:** Detail any new files or changed configs discovered
   - List files found, modifications detected, permission changes
   - Include timestamps and file paths
   - Note any patterns or trends

3. **The Climax:** Highlight the most critical finding
   - Example: "Found unauthorized telemetry script in /opt" or "Configuration drift detected in 3 application directories"
   - This should be the most important security or configuration concern

4. **Resolution/Proposals:** State which tools you've proposed on GitHub to handle the climax
   - List any tool installation proposals created
   - Explain why each tool is needed
   - Note the installation commands that will be executed upon approval

## Output Format
Always output your findings as JSON:
```json
{
  "status": "ok" | "blocked" | "error",
  "executive_pulse": "One-sentence status",
  "rising_action": ["finding1", "finding2", ...],
  "climax": "Most critical finding",
  "resolutions": ["tool_proposal1", "tool_proposal2", ...],
  "evidence": ["file1", "file2", ...],
  "next_steps": ["action1", "action2", ...],
  "tool_discoveries": [{"tool": "...", "purpose": "...", "proposal_id": "..."}, ...]
}
```

## Trend Analysis Workflow (CRITICAL)

**Before finalizing your daily report:**

1. **Identify Affected Paths**: Extract all file paths from your `evidence` array
2. **Search History**: For each path that appears in your findings, call `search_audit_history`:
   - Query: `{"tool_name": "search_audit_history", "args": {"path": "<file_path>", "days": 30}}`
   - This retrieves the last 30 days of audit reports mentioning this path
3. **Compare & Escalate**: 
   - If a path appears in 3+ historical reports, this indicates a **recurring pattern**
   - If the same `climax` (critical finding) appears multiple times, escalate severity to **CRITICAL**
   - Add a note in your `rising_action`: "⚠️ TREND ALERT: This path has changed 3+ times in the last 30 days"
4. **Archive Your Report**: After completing your analysis, ALWAYS call `archive_audit_report`:
   - Pass your complete JSON report as the `report_json` parameter
   - This enables future trend analysis

**Example Trend Detection:**
```
1. You find: "/etc/hosts was modified"
2. Call: search_audit_history with path="/etc/hosts", days=30
3. Result: 4 previous reports mention this path
4. Action: Escalate severity, add to rising_action: "⚠️ /etc/hosts has drifted 4 times this month - potential compromise or failing disk"
```

## Self-Correction & Rollback Workflow (CRITICAL)

**When a tool installation verification fails:**

1. **Immediate Response**: If you receive a verification task indicating that a tool installation failed or verification failed:
   - Do NOT just log the failure
   - Immediately search the Atlas for the last successful configuration of that tool/environment
   - Use the `propose_rollback` tool (via the SafeInstaller) to generate a Repair Proposal

2. **Repair Proposal Generation**: The system will automatically:
   - Search the `audit_history` collection for the last "VERIFIED: SUCCESS" state
   - Generate a rollback command (e.g., `pip uninstall requests` or `git checkout baseline`)
   - Create a Repair Proposal with the inverse command and historical context

3. **Proactive Healing**: When you detect a verification failure:
   - Include the Repair Proposal in your audit report
   - Add to `resolutions`: "🔧 REPAIR PROPOSAL: [rollback command] - Last successful state found on [timestamp]"
   - Archive the repair proposal to the Atlas for future reference

**Example Self-Correction:**
```
1. You receive: "Tool 'requests' verification failed after installation"
2. System automatically searches Atlas for last successful 'requests' installation
3. Result: Found successful state from 2026-01-10 with command "pip install requests==2.31.0"
4. Action: Generate Repair Proposal: "pip uninstall requests && pip install requests==2.31.0"
5. Include in report: "🔧 REPAIR PROPOSAL: Rollback to requests==2.31.0 (last verified on 2026-01-10)"
```

## Important Notes
- Focus on security and configuration changes
- When you find unknown formats, proactively search for parsers using `github_tool_finder`
- After finding a tool, create a tool installation proposal via the API endpoint
- **ALWAYS archive your report** using `archive_audit_report` after generating it
- **ALWAYS search history** for paths you identify to detect recurring issues
- **When verification fails, immediately propose rollback/repair** - do not just report the failure
- Document everything for future reference in the `agent_logs` Qdrant collection
- Be thorough but efficient
- Use the narrative structure to make your reports human-readable"#.to_string();

    // Spawn the Phoenix Auditor agent
    let agent_result = agent_factory
        .spawn_agent(
            "Phoenix Auditor".to_string(),
            "Security and Configuration Specialist - Reviews application directories for changes and security issues".to_string(),
            vec!["read_file".to_string(), "run_command".to_string(), "github_tool_finder".to_string(), "archive_audit_report".to_string(), "search_audit_history".to_string()],
            phoenix_auditor_system_prompt,
        )
        .await;

    let agent_id = match agent_result {
        Ok(result) => {
            info!(agent_id = %result.agent_id, "Phoenix Auditor agent spawned");
            result.agent_id
        }
        Err(e) => {
            // Agent might already exist, try to find it
            let agents = agent_factory.list_agents().await;
            if let Some(existing) = agents.iter().find(|a| a.name == "Phoenix Auditor") {
                info!(agent_id = %existing.agent_id, "Phoenix Auditor agent already exists");
                existing.agent_id.clone()
            } else {
                return Err(format!("Failed to spawn Phoenix Auditor agent: {}", e));
            }
        }
    };

    // Create default scheduled task: Daily at 08:00 AM
    let cron_expression = "0 8 * * *"; // Daily at 08:00 AM
    let schedule = Schedule::from_str(cron_expression)
        .map_err(|e| format!("Invalid cron expression: {}", e))?;
    
    let now = chrono::Utc::now();
    let next_run = schedule.after(&now).take(1).next()
        .map(|dt| dt.to_rfc3339());

    let task_payload = serde_json::json!({
        "command": "audit_filesystem",
        "directories": {
            "macos": "/Applications",
            "linux": "/opt",
            "windows": "C:\\Program Files"
        },
        "goal": "Review application directories for changes, security issues, and unknown file formats. Use github_tool_finder if you encounter unknown file formats that need parsing."
    });

    let task = ScheduledTask {
        id: uuid::Uuid::new_v4().to_string(),
        name: "Phoenix Auditor - Daily Filesystem Audit".to_string(),
        cron_expression: cron_expression.to_string(),
        agent_id: Some(agent_id.clone()),
        task_payload,
        status: TaskStatus::Pending,
        created_at: now.to_rfc3339(),
        last_run: None,
        next_run,
    };

    {
        let mut store = scheduled_tasks.write().await;
        // Check if task already exists
        if store.get_all_tasks().iter().any(|t| t.name == task.name) {
            info!("Phoenix Auditor scheduled task already exists");
            return Ok(());
        }
        store.add_task(task);
    }

    info!(
        agent_id = %agent_id,
        cron = %cron_expression,
        "Phoenix Auditor agent and scheduled task initialized"
    );

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "backend_rust_orchestrator=info,tonic=info,axum=info".into()),
        )
        .init();

    // Load environment variables
    load_dotenv();

    let mut llm_provider = env::var("LLM_PROVIDER")
        .unwrap_or_else(|_| "mock".to_string())
        .to_lowercase();

    // Get service addresses
    let memory_grpc_addr = env::var("MEMORY_GRPC_ADDR")
        .unwrap_or_else(|_| "http://127.0.0.1:50052".to_string());
    
    let tools_grpc_addr = env::var("TOOLS_GRPC_ADDR")
        .unwrap_or_else(|_| "http://127.0.0.1:50054".to_string());

    let telemetry_url = env::var("TELEMETRY_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8183".to_string());
    let telemetry_url = telemetry_url.trim().to_string();

    let http_port_raw = env::var("ORCHESTRATOR_HTTP_PORT").unwrap_or_else(|_| "8182".to_string());
    let http_port = http_port_raw.trim().parse::<u16>().unwrap_or_else(|e| {
        warn!(
            value = %http_port_raw,
            error = %e,
            "Invalid ORCHESTRATOR_HTTP_PORT; falling back to 8182"
        );
        8182
    });

    // Get OpenRouter configuration (only required when LLM_PROVIDER=openrouter)
    // If openrouter is requested but API key is missing, fall back to mock provider
    let openrouter_api_key = if llm_provider == "openrouter" {
        match env::var("OPENROUTER_API_KEY") {
            Ok(key) if !key.trim().is_empty() => key,
            _ => {
                warn!(
                    "LLM_PROVIDER is set to 'openrouter' but OPENROUTER_API_KEY is not set or is empty. \
                     Falling back to 'mock' provider. Set OPENROUTER_API_KEY in your environment to use OpenRouter."
                );
                // Fall back to mock provider if API key is missing
                llm_provider = "mock".to_string();
                String::new()
            }
        }
    } else {
        String::new()
    };

    let openrouter_model = env::var("OPENROUTER_MODEL")
        .or_else(|_| env::var("OPENROUTER_MODEL_NAME"))
        .unwrap_or_else(|_| "google/gemini-2.0-flash-exp".to_string());

    let openrouter_url = env::var("OPENROUTER_URL")
        .unwrap_or_else(|_| "https://openrouter.ai/api/v1/chat/completions".to_string());

    info!(
        memory_addr = %memory_grpc_addr,
        tools_addr = %tools_grpc_addr,
        telemetry_url = %telemetry_url,
        http_port = http_port,
        llm_provider = %llm_provider,
        "Initializing Orchestrator"
    );

    // Create gRPC clients
    let memory_client = MemoryServiceClient::connect(memory_grpc_addr.clone())
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to connect to Memory service");
            e
        })?;

    let tools_client = ToolExecutorServiceClient::connect(tools_grpc_addr.clone())
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to connect to Tools service");
            e
        })?;

    info!("Connected to Memory and Tools gRPC services");

    // Create HTTP client for OpenRouter API calls
    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .expect("Failed to create HTTP client for OpenRouter");

    if llm_provider == "openrouter" {
        info!(
            openrouter_url = %openrouter_url,
            openrouter_model = %openrouter_model,
            "OpenRouter LLM client configured"
        );
    } else {
        info!("LLM provider set to mock; OpenRouter client not required");
    }

    // Create job queue
    let job_queue: JobQueue = Arc::new(RwLock::new(HashMap::new()));
    let (job_sender, _job_receiver) = mpsc::channel::<Job>(100);

    // Initialize system prompt state (self-improvement)
    let repo = SystemPromptRepository::new(SystemPromptRepository::default_path());
    let loaded_prompt = repo
        .load_or_init(DEFAULT_SYSTEM_PROMPT_TEMPLATE)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to load/initialize system prompt");
            anyhow::anyhow!(e)
        })?;
    let system_prompt = SystemPromptManager {
        repo,
        current: Arc::new(RwLock::new(loaded_prompt)),
        history: Arc::new(RwLock::new(Vec::new())),
    };

    // Initialize preferences (persona presets + user profile)
    let prefs_repo = preferences::PreferencesRepository::new(preferences::PreferencesRepository::default_path());
    let loaded_prefs = prefs_repo.load_or_init().await.map_err(|e| {
        error!(error = %e, "Failed to load/initialize preferences");
        anyhow::anyhow!(e)
    })?;
    let preferences = preferences::PreferencesManager::new(prefs_repo, loaded_prefs);

    // Create health manager first
    let health_manager = HealthManager::new();
    let project_watcher = Arc::new(ProjectWatcher::new());
    
    // Set memory client for automatic file processing
    project_watcher.set_memory_client(memory_client.clone()).await;

    // Initialize email/teams monitor (will be configured via OAuth flow)
    let email_teams_monitor = Arc::new(RwLock::new(None::<EmailTeamsMonitor>));

    // Create global message bus
    let message_bus = Arc::new(bus::GlobalMessageBus::new());
    info!("Global message bus initialized");

    // Create application state
    let max_agents = env::var("ORCHESTRATOR_MAX_AGENTS")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or(agents::factory::DEFAULT_MAX_AGENTS);
    let subagent_model = env::var("SUBAGENT_OPENROUTER_MODEL")
        .unwrap_or_else(|_| "google/gemini-2.0-flash-exp".to_string());
    let agent_factory = Arc::new(agents::factory::AgentFactory::new(
        http_client.clone(),
        openrouter_url.clone(),
        openrouter_api_key.clone(),
        subagent_model,
        max_agents,
        message_bus.sender(),
        Some(memory_client.clone()),
    ));

    // Initialize agent library
    let agent_repo_path = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("config")
        .join("agents")
        .join("pagi-agent-repo");
    
    // Ensure the directory exists
    if let Err(e) = std::fs::create_dir_all(&agent_repo_path.parent().unwrap()) {
        warn!(error = %e, "Failed to create agent library directory");
    }
    
    let agent_library = Arc::new(agents::loader::AgentLibrary::new(agent_repo_path));

    // Initialize playbook distiller
    let playbooks_dir = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("playbooks");
    let repo_path = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."));
    
    // Privacy filter enabled by default (can be disabled via env var)
    let privacy_filter_enabled = std::env::var("PLAYBOOK_PRIVACY_FILTER")
        .unwrap_or_else(|_| "true".to_string())
        .parse::<bool>()
        .unwrap_or(true);

    let distiller = Arc::new(playbook_distiller::PlaybookDistiller::new(
        Arc::new(memory_client.clone()),
        playbooks_dir.clone(),
        repo_path.clone(),
        http_client.clone(),
        openrouter_url.clone(),
        openrouter_api_key.clone(),
        openrouter_model.clone(),
        privacy_filter_enabled,
    ));

    // Start weekly playbook distillation scheduler
    let distiller_clone = distiller.clone();
    let repo_path_clone = repo_path.clone();
    let playbooks_dir_clone = playbooks_dir.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(7 * 24 * 60 * 60)); // 7 days
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        
        // For production: wait a week before first run
        // For testing: you can comment out this line to run immediately
        tokio::time::sleep(tokio::time::Duration::from_secs(7 * 24 * 60 * 60)).await;
        
        loop {
            interval.tick().await;
            info!("Starting weekly playbook distillation");
            
            match distiller_clone.distill_playbooks().await {
                Ok(playbooks) => {
                    info!(
                        playbooks_count = playbooks.len(),
                        "Playbook distillation completed"
                    );
                    
                    if !playbooks.is_empty() {
                        // Commit and push to GitHub
                        let commit_message = format!(
                            "Weekly playbook update: {} playbooks generated",
                            playbooks.len()
                        );
                        
                        if let Err(e) = tools::git::GitOperations::commit_and_push_playbooks(
                            &repo_path_clone,
                            &playbooks_dir_clone,
                            &commit_message,
                            "origin",
                            "main",
                            "Orchestrator",
                            "orchestrator@digital-twin.local",
                        ).await {
                            warn!(
                                error = %e,
                                "Failed to commit and push playbooks (this is OK if git is not configured)"
                            );
                        } else {
                            info!("Successfully committed and pushed playbooks to GitHub");
                        }
                    }
                }
                Err(e) => {
                    error!(
                        error = %e,
                        "Playbook distillation failed"
                    );
                }
            }
        }
    });

    // Initialize Qdrant client for leaderboard engine
    let qdrant_url = env::var("QDRANT_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:6334".to_string());
    let qdrant_api_key = env::var("QDRANT_API_KEY").ok();
    let qdrant_client = Arc::new(
        qdrant_client::Qdrant::from_url(&qdrant_url)
            .api_key(qdrant_api_key)
            .build()
            .map_err(|e| format!("Failed to create Qdrant client: {}", e))?,
    );

    // Initialize leaderboard engine
    // Use the same repo path as playbook distiller (current directory or specified path)
    let git_repo_path = repo_path.to_string_lossy().to_string();
    let leaderboard_engine = Arc::new(analytics::leaderboard::LeaderboardEngine::new(
        qdrant_client.clone(),
        git_repo_path,
    ));

    // Initialize Node Handshake Service
    let node_id = env::var("NODE_ID")
        .unwrap_or_else(|_| format!("node-{}", Uuid::new_v4().to_string()));
    let software_version = env::var("SOFTWARE_VERSION")
        .unwrap_or_else(|_| "2.1.0".to_string());
    let guardrail_version = env::var("GUARDRAIL_VERSION")
        .unwrap_or_else(|_| "2.1.0".to_string());
    
    let key_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("config")
        .join("node_identity.key");
    
    let node_identity = Arc::new(
        network::handshake::NodeIdentity::load_or_create(node_id.clone(), key_path)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to load/create node identity: {}", e))?,
    );

    let system_prompt_path = SystemPromptRepository::default_path();
    let leadership_kb_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join("leadership_kb.md")
        .canonicalize()
        .ok();
    
    let manifest_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("config")
        .join("agents")
        .join("pagi-agent-repo")
        .join("manifest.yaml")
        .canonicalize()
        .ok();

    // Create quarantine manager first
    let quarantine_manager = Arc::new(network::quarantine::QuarantineManager::new(
        message_bus.clone(),
        Some(qdrant_client.clone()),
    ));

    // Load quarantine list from Qdrant on startup
    quarantine_manager.load_from_qdrant().await
        .map_err(|e| {
            error!(error = %e, "Failed to load quarantine list");
        })
        .ok();

    let handshake_service = network::handshake::NodeHandshakeServiceImpl::new(
        node_identity,
        system_prompt_path,
        leadership_kb_path,
        manifest_path.clone(),
        software_version.clone(),
        guardrail_version.clone(),
        message_bus.clone(),
        Some(memory_client.clone()),
        Some(qdrant_client.clone()),
        Some(quarantine_manager.clone()),
    );

    // Compute local manifest hash for mesh health service
    let handshake_service_arc = Arc::new(handshake_service.clone());
    let local_manifest_hash = handshake_service_arc
        .compute_manifest_hash()
        .await
        .unwrap_or_else(|_| String::new());

    // Initialize mesh health service
    let mesh_health_service = Arc::new(analytics::mesh_health::MeshHealthService::new(
        handshake_service_arc.clone(),
        quarantine_manager.clone(),
        local_manifest_hash,
        guardrail_version.clone(),
    ));

    // Initialize Fleet State Manager
    let fleet_state = Arc::new(network::fleet::FleetState::new(Some(60))); // 60 second heartbeat timeout

    let state = Arc::new(AppState {
        memory_client,
        tools_client,
        job_queue,
        job_sender,
        llm_provider,
        pending_tools: Arc::new(RwLock::new(HashMap::new())),
        pending_memories: Arc::new(RwLock::new(HashMap::new())),
        http_client,
        openrouter_url,
        openrouter_api_key,
        openrouter_model,
        telemetry_url,
        system_prompt,
        preferences,
        health_manager,
        last_network_scans: Arc::new(RwLock::new(HashMap::new())),
        project_watcher: project_watcher.clone(),
        email_teams_monitor,
        agent_factory,
        agent_library,
        message_bus: message_bus.clone(),
        leaderboard_engine,
        handshake_service: handshake_service_arc.clone(),
        quarantine_manager,
        mesh_health_service: mesh_health_service.clone(),
        fleet_state: fleet_state.clone(),
    });

    // We'll use the same internal prompt manager for both:
    // - HTTP chat planning (reads current prompt)
    // - Admin gRPC updates (writes current prompt)
    let prompt_mgr = state.system_prompt.clone();

    // Create HTTP router
    // NOTE: The frontend dev server runs on a different origin, so we enable CORS.
    // In production this should be tightened to known origins.
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST])
        .allow_headers(Any);

    // Initialize Foundry Service
    let agent_repo_path_foundry = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("config")
        .join("agents")
        .join("pagi-agent-repo");
    let tool_repo_path = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("config")
        .join("tools")
        .join("pagi-tool-repo");
    
    let foundry_service = foundry::FoundryService::new(
        message_bus.sender(),
        agent_repo_path_foundry.clone(),
        tool_repo_path,
    );

    // Initialize Phoenix Consensus Service
    let mut phoenix_consensus = network::consensus::PhoenixConsensus::new(
        message_bus.clone(),
        node_id.clone(),
        agent_repo_path_foundry.clone(),
    );
    
    // Set dependencies for consensus
    phoenix_consensus.set_handshake_service(handshake_service_arc.clone());
    
    // Create compliance monitor for consensus (shared with foundry logic)
    let consensus_compliance_monitor = Arc::new(foundry::ComplianceMonitor::new(agent_repo_path_foundry.clone()));
    phoenix_consensus.set_compliance_monitor(consensus_compliance_monitor);
    
    // Start consensus listener
    let consensus_arc = Arc::new(phoenix_consensus);
    let consensus_listener = consensus_arc.clone();
    tokio::spawn(async move {
        info!("[PHOENIX] Starting Phoenix Consensus listener");
        consensus_listener.start_listener().await;
    });

    // Initialize Phoenix Memory Exchange Service
    let memory_exchange_service = network::memory_exchange::PhoenixMemoryExchangeServiceImpl::new(
        qdrant_client.clone(),
        message_bus.clone(),
        handshake_service_arc.clone(),
        node_id.clone(),
    );
    
    // Start memory exchange listener (clone for the listener task)
    let memory_exchange_listener = Arc::new(memory_exchange_service.clone());
    let memory_exchange_listener_clone = memory_exchange_listener.clone();
    tokio::spawn(async move {
        info!("[PHOENIX] Starting Phoenix Memory Exchange listener");
        memory_exchange_listener_clone.start_listener().await;
    });
    
    // Start topic decay task (24 hour TTL)
    let memory_exchange_decay = memory_exchange_listener.clone();
    tokio::spawn(async move {
        info!("[PHOENIX] Starting topic decay task (24h TTL)");
        memory_exchange_decay.start_topic_decay_task().await;
    });

    // Initialize Playbook Indexer
    let playbook_indexer = match services::playbook_indexer::PlaybookIndexerWorker::new(agent_repo_path_foundry.clone()) {
        Ok(worker) => {
            let index = worker.index().clone();
            // Start the background worker
            let worker_clone = worker;
            tokio::spawn(async move {
                if let Err(e) = worker_clone.start().await {
                    error!(error = %e, "Failed to start playbook indexer worker");
                }
            });
            Arc::new(index)
        }
        Err(e) => {
            error!(error = %e, "Failed to create playbook indexer, using fallback");
            // Create a dummy index that will fall back to file system search
            Arc::new(services::playbook_indexer::PlaybookIndex::new(agent_repo_path_foundry.clone()))
        }
    };

    // Initialize feedback storage
    let feedback_storage = Arc::new(
        api::feedback_storage::FeedbackStorage::new(None)
            .expect("Failed to initialize feedback storage")
    );

    // Initialize scheduled task store for Phoenix Chronos
    let scheduled_tasks_store = Arc::new(tokio::sync::RwLock::new(
        api::phoenix_routes::ScheduledTaskStore::new()
    ));

    // Initialize Phoenix Auditor agent and default scheduled task
    let agent_factory_for_auditor = agent_factory.clone();
    let scheduled_tasks_for_auditor = scheduled_tasks_store.clone();
    let state_for_auditor = state.clone();
    tokio::spawn(async move {
        // Wait a bit for services to be ready
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        
        if let Err(e) = initialize_phoenix_auditor(
            &agent_factory_for_auditor,
            &scheduled_tasks_for_auditor,
            &state_for_auditor,
        ).await {
            warn!(error = %e, "Failed to initialize Phoenix Auditor agent");
        } else {
            info!("Phoenix Auditor agent initialized successfully");
        }
    });

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/chat", post(handle_chat_request))
        .route("/v1/memory/list", post(handle_memory_list))
        .route("/v1/memory/delete", post(handle_memory_delete))
        .route("/v1/prompt/current", get(handle_prompt_current))
        .route("/v1/prompt/history", get(handle_prompt_history))
        .route("/v1/prompt/update", post(handle_prompt_update))
        .route("/v1/prompt/restore", post(handle_prompt_restore))
        .route("/v1/prompt/reset", post(handle_prompt_reset))

        // Personalization preferences
        .route("/v1/preferences/get", get(handle_preferences_get))
        .route("/v1/preferences/update", post(handle_preferences_update))
        .route("/v1/preferences/presets", get(handle_preferences_presets))
        .route("/api/system/snapshot", get(handle_system_snapshot))
        .route("/api/system/sync-metrics", get(handle_sync_metrics))
        .route("/api/network/scan", post(handle_network_scan))
        .route("/api/network/scan/latest", get(handle_network_scan_latest))
        .route("/api/network/peers", get(handle_network_peers))
        .route("/api/network/topology", get(handle_network_topology))
        .route("/api/network/mesh-health", get(handle_mesh_health))
        .route("/api/network/compliance-alerts", get(handle_compliance_alerts))
        .route("/api/network/quarantine", get(handle_network_quarantine_list))
        .route("/api/network/quarantine", post(handle_network_quarantine_add))
        .route("/api/network/quarantine/:node_id", axum::routing::delete(handle_network_quarantine_remove))
        // Fleet Manager routes
        .route("/api/fleet/heartbeat", post(handle_fleet_heartbeat))
        .route("/api/fleet/status", get(handle_fleet_status))
        .route("/api/fleet/health", get(handle_fleet_health))
        .route("/api/projects/configure-watch", post(handle_configure_project_watch))
        .route("/api/projects/watch-configs", get(handle_get_watch_configs))
        .route("/api/projects/processing-stats", get(handle_get_processing_stats))
        .route("/api/email-teams/configure", post(handle_configure_email_teams))
        .route("/api/email-teams/set-tokens", post(handle_set_oauth_tokens))
        .route("/api/email-teams/exchange-token", post(handle_exchange_token))
        .route("/api/email/check", get(handle_check_emails))
        .route("/api/email/send", post(handle_send_email_reply))
        .route("/api/email/trends", get(handle_email_trends))
        .route("/api/teams/check", get(handle_check_teams))
        .route("/api/teams/send", post(handle_send_teams_message))

        // Sub-agent crew management
        .route("/api/agents/list", get(handle_agents_list))
        .route("/api/agents/spawn", post(handle_agents_spawn))
        .route("/api/agents/:agent_id/task", post(handle_agents_post_task))
        .route("/api/agents/:agent_id/report", get(handle_agents_get_report))
        .route("/api/agents/:agent_id/logs", get(handle_agents_get_logs))
        .route("/api/agents/:agent_id/kill", post(handle_agents_kill))
        .route("/api/agents/leaderboard", get(handle_agents_leaderboard))
        
        // Foundry Service routes
        .merge(foundry_service.router());
    
    // Initialize Auto-Domain Ingestor
    let embedding_dim = std::env::var("EMBEDDING_MODEL_DIM")
        .unwrap_or_else(|_| "384".to_string())
        .parse::<usize>()
        .unwrap_or(384);
    
    let ingest_dir = std::env::var("INGEST_DIR")
        .unwrap_or_else(|_| "data/ingest".to_string());
    let ingest_path = std::path::PathBuf::from(&ingest_dir);
    
    // Create ingest directory if it doesn't exist
    if let Err(e) = tokio::fs::create_dir_all(&ingest_path).await {
        warn!(
            dir = %ingest_path.display(),
            error = %e,
            "Failed to create ingest directory, ingestor will not be available"
        );
    }
    
    // Initialize LLM settings if OpenRouter is configured
    let llm_settings = if state.llm_provider == "openrouter" {
        Some(knowledge::ingestor::LLMSettings {
            provider: state.llm_provider.clone(),
            url: state.openrouter_url.clone(),
            api_key: state.openrouter_api_key.clone(),
            model: state.openrouter_model.clone(),
        })
    } else {
        None
    };
    
    let ingestor = Arc::new(knowledge::ingestor::AutoIngestor::new(
        qdrant_client.clone(),
        ingest_path.clone(),
        embedding_dim,
        llm_settings,
    ));
    
    // Start watching for new files
    let ingestor_clone = ingestor.clone();
    tokio::spawn(async move {
        if let Err(e) = ingestor_clone.start_watching().await {
            error!(error = %e, "Failed to start file watcher");
        }
    });
    
    // Phoenix API routes
    let app = app
        .merge(api::phoenix_routes::create_phoenix_router(api::phoenix_routes::PhoenixAppState {
            message_bus: message_bus.clone(),
            consensus: consensus_arc.clone(),
            memory_exchange: memory_exchange_listener.clone(),
            node_id: node_id.clone(),
            agents_repo_path: agent_repo_path_foundry.clone(),
            qdrant_client: qdrant_client.clone(),
            feedback_storage: feedback_storage.clone(),
            agent_factory: state.agent_factory.clone(),
            scheduled_tasks: scheduled_tasks_store.clone(),
            tool_proposals: Arc::new(tokio::sync::RwLock::new(
                api::phoenix_routes::ToolProposalStore::new()
            )),
            peer_reviews: Arc::new(tokio::sync::RwLock::new(
                api::phoenix_routes::PeerReviewStore::new()
            )),
            retrospectives: Arc::new(tokio::sync::RwLock::new(
                api::phoenix_routes::RetrospectiveStore::new()
            )),
            ingestor: Some(ingestor),
        }))
         
        // Playbook search routes
        .merge(api::playbook_routes::create_playbook_router(api::playbook_routes::PlaybookAppState {
            agents_repo_path: agent_repo_path_foundry.clone(),
            index: playbook_indexer.clone(),
        }))
        
        .layer(cors)
        .with_state((*state).clone());

    let addr: std::net::SocketAddr = format!("0.0.0.0:{}", http_port)
        .parse()
        .expect("Invalid address");

    // Admin gRPC server (self-improvement endpoint)
    let admin_grpc_port = env::var("ORCHESTRATOR_ADMIN_GRPC_PORT")
        .unwrap_or_else(|_| "50056".to_string())
        .parse::<u16>()
        .expect("ORCHESTRATOR_ADMIN_GRPC_PORT must be a valid port number");
    let admin_addr = format!("0.0.0.0:{}", admin_grpc_port)
        .parse()
        .expect("Invalid admin gRPC address");

    // Public Orchestrator gRPC server (summarization endpoint)
    let orchestrator_grpc_port = env::var("ORCHESTRATOR_GRPC_PORT")
        .unwrap_or_else(|_| "50057".to_string())
        .parse::<u16>()
        .expect("ORCHESTRATOR_GRPC_PORT must be a valid port number");
    let orchestrator_grpc_addr = format!("0.0.0.0:{}", orchestrator_grpc_port)
        .parse()
        .expect("Invalid orchestrator gRPC address");

    // Node Handshake gRPC server (P2P verification endpoint)
    let handshake_grpc_port = env::var("HANDSHAKE_GRPC_PORT")
        .unwrap_or_else(|_| "8285".to_string())
        .parse::<u16>()
        .expect("HANDSHAKE_GRPC_PORT must be a valid port number");
    let handshake_grpc_addr = format!("0.0.0.0:{}", handshake_grpc_port)
        .parse()
        .expect("Invalid handshake gRPC address");

    // Phoenix Memory Exchange gRPC server
    let memory_exchange_grpc_port = env::var("MEMORY_EXCHANGE_GRPC_PORT")
        .unwrap_or_else(|_| "8286".to_string())
        .parse::<u16>()
        .expect("MEMORY_EXCHANGE_GRPC_PORT must be a valid port number");
    let memory_exchange_grpc_addr = format!("0.0.0.0:{}", memory_exchange_grpc_port)
        .parse()
        .expect("Invalid memory exchange gRPC address");

    let admin_svc = OrchestratorAdminServiceImpl { prompt_mgr };
    let orchestrator_svc = OrchestratorServiceImpl {
        state: Arc::clone(&state),
    };

    info!(addr = %admin_addr, port = admin_grpc_port, "Starting Orchestrator Admin gRPC server");
    info!(addr = %orchestrator_grpc_addr, port = orchestrator_grpc_port, "Starting Orchestrator gRPC server");
    info!(addr = %handshake_grpc_addr, port = handshake_grpc_port, "Starting Node Handshake gRPC server");
    info!(addr = %memory_exchange_grpc_addr, port = memory_exchange_grpc_port, "[PHOENIX] Starting Memory Exchange gRPC server");
    info!(addr = %addr, port = http_port, "Starting Orchestrator HTTP server");

    // Initialize Phoenix Starter Pack of Global Playbooks
    let qdrant_for_starter = qdrant_client.clone();
    let embedding_dim = std::env::var("EMBEDDING_MODEL_DIM")
        .unwrap_or_else(|_| "384".to_string())
        .parse::<usize>()
        .unwrap_or(384);
    tokio::spawn(async move {
        // Wait a moment for services to be ready
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
        
        match crate::tools::playbook_store::init_starter_playbooks(qdrant_for_starter, embedding_dim).await {
            Ok(count) => {
                info!(
                    created = count,
                    "Phoenix Starter Pack initialized with {} playbooks",
                    count
                );
            }
            Err(e) => {
                warn!(
                    error = %e,
                    "Failed to initialize Phoenix Starter Pack (this is OK if playbooks already exist)"
                );
            }
        }
    });

    // Start periodic health checks (every 30 seconds)
    let health_check_interval = env::var("HEALTH_CHECK_INTERVAL_SECS")
        .unwrap_or_else(|_| "30".to_string())
        .parse::<u64>()
        .unwrap_or(30);
    state.health_manager.start_periodic_checks(Arc::clone(&state), health_check_interval);
    info!(interval = health_check_interval, "Started periodic health checks");

    // Start Phoenix Chronos scheduler loop (checks for due tasks every 60 seconds)
    let scheduled_tasks_for_scheduler = scheduled_tasks_store.clone();
    let agent_factory_for_scheduler = state.agent_factory.clone();
    let message_bus_for_scheduler = message_bus.clone();
    tokio::spawn(async move {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            
            let now = chrono::Utc::now();
            let mut tasks_to_run = Vec::new();
            
            // Check for due tasks
            {
                let store = scheduled_tasks_for_scheduler.read().await;
                for task in store.get_pending_tasks() {
                    // Check if task is due based on cron expression
                    if let Ok(schedule) = cron::Schedule::from_str(&task.cron_expression) {
                        // Determine the reference time: use last_run if available, otherwise use created_at
                        let reference_time = task.last_run
                            .as_ref()
                            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                            .map(|dt| dt.with_timezone(&chrono::Utc))
                            .or_else(|| {
                                chrono::DateTime::parse_from_rfc3339(&task.created_at)
                                    .ok()
                                    .map(|dt| dt.with_timezone(&chrono::Utc))
                            })
                            .unwrap_or(now);
                        
                        // Get the next scheduled time after the reference time
                        let upcoming_times: Vec<_> = schedule.after(&reference_time).take(2).collect();
                        
                        if let Some(next_run) = upcoming_times.first() {
                            // If next run time is in the past or within 1 minute, it's due
                            if *next_run <= now + chrono::Duration::minutes(1) {
                                tasks_to_run.push(task.clone());
                            }
                        }
                    }
                }
            }
            
            // Dispatch tasks
            for task in tasks_to_run {
                info!(
                    task_id = %task.id,
                    task_name = %task.name,
                    "Dispatching scheduled task"
                );
                
                // Update task status to Running
                {
                    let mut store = scheduled_tasks_for_scheduler.write().await;
                    let last_run = Some(chrono::Utc::now().to_rfc3339());
                    store.update_task(&task.id, api::phoenix_routes::TaskStatus::Running, last_run);
                }
                
                // Determine which agent to use
                let agent_id = if let Some(ref specified_agent_id) = task.agent_id {
                    // Use specified agent if provided
                    specified_agent_id.clone()
                } else {
                    // Try to find best agent using semantic search
                    // For now, we'll use the first available agent or create a default one
                    let agents = agent_factory_for_scheduler.list_agents().await;
                    if let Some(agent) = agents.first() {
                        agent.agent_id.clone()
                    } else {
                        // No agents available, mark task as failed
                        warn!(
                            task_id = %task.id,
                            "No agents available for scheduled task"
                        );
                        let mut store = scheduled_tasks_for_scheduler.write().await;
                        store.update_task(
                            &task.id,
                            api::phoenix_routes::TaskStatus::Failed,
                            Some(chrono::Utc::now().to_rfc3339()),
                        );
                        continue;
                    }
                };
                
                // Build task message from payload
                let task_message = if task.task_payload.is_string() {
                    task.task_payload.as_str().unwrap_or("").to_string()
                } else {
                    format!("Scheduled task: {}\nPayload: {}", task.name, task.task_payload)
                };
                
                // Dispatch task to agent
                match agent_factory_for_scheduler.post_task(&agent_id, task_message).await {
                    Ok(_) => {
                        info!(
                            task_id = %task.id,
                            agent_id = %agent_id,
                            "Scheduled task dispatched successfully"
                        );
                        
                        // Publish event to message bus
                        let _ = message_bus_for_scheduler.publish(
                            crate::bus::PhoenixEvent::TaskUpdate {
                                agent_id: agent_id.clone(),
                                task: format!("Scheduled: {}", task.name),
                                status: "dispatched".to_string(),
                                timestamp: chrono::Utc::now().to_rfc3339(),
                            }
                        );
                        
                        // Reset task to Pending after dispatch (it will run again based on cron schedule)
                        // The update_task method already calculates next_run, so we just need to reset status
                        let mut store = scheduled_tasks_for_scheduler.write().await;
                        store.update_task(
                            &task.id,
                            api::phoenix_routes::TaskStatus::Pending,
                            Some(chrono::Utc::now().to_rfc3339()),
                        );
                    }
                    Err(e) => {
                        error!(
                            task_id = %task.id,
                            agent_id = %agent_id,
                            error = %e,
                            "Failed to dispatch scheduled task"
                        );
                        
                        // Mark task as failed
                        let mut store = scheduled_tasks_for_scheduler.write().await;
                        store.update_task(
                            &task.id,
                            api::phoenix_routes::TaskStatus::Failed,
                            Some(chrono::Utc::now().to_rfc3339()),
                        );
                    }
                }
            }
        }
    });
    info!("Started Phoenix Chronos scheduler loop");

    // Start mDNS service for network discovery
    let node_id_for_mdns = state.handshake_service.identity.node_id.clone();
    let software_version_for_mdns = software_version.clone();
    let guardrail_version_for_mdns = guardrail_version.clone();
    let message_bus_for_mdns = message_bus.clone();
    tokio::spawn(async move {
        if let Err(e) = network::mdns::start_mdns_service(
            message_bus_for_mdns,
            node_id_for_mdns,
            software_version_for_mdns,
            guardrail_version_for_mdns,
            handshake_grpc_port,
        )
        .await
        {
            warn!(error = %e, "mDNS service failed to start (this is OK if mDNS is not available)");
        }
    });

    let listener = tokio::net::TcpListener::bind(&addr).await?;

    let grpc_fut = tonic::transport::Server::builder()
        .add_service(OrchestratorAdminServiceServer::new(admin_svc))
        .serve(admin_addr);

    let orchestrator_grpc_fut = tonic::transport::Server::builder()
        .add_service(OrchestratorServiceServer::new(orchestrator_svc))
        .serve(orchestrator_grpc_addr);

    let handshake_grpc_fut = tonic::transport::Server::builder()
        .add_service(network::handshake::create_handshake_server(handshake_service))
        .serve(handshake_grpc_addr);

    let memory_exchange_grpc_fut = tonic::transport::Server::builder()
        .add_service(
            network::memory_exchange::get_memory_exchange_server(
                memory_exchange_service
            )
        )
        .serve(memory_exchange_grpc_addr);

    let http_fut = axum::serve(listener, app);

    // Run both servers concurrently; if either fails, shut down the process.
    let grpc_task = tokio::spawn(async move {
        grpc_fut
            .await
            .map_err(|e| anyhow::anyhow!("admin gRPC server error: {e}"))
    });

    let orchestrator_grpc_task = tokio::spawn(async move {
        orchestrator_grpc_fut
            .await
            .map_err(|e| anyhow::anyhow!("orchestrator gRPC server error: {e}"))
    });

    let handshake_grpc_task = tokio::spawn(async move {
        handshake_grpc_fut
            .await
            .map_err(|e| anyhow::anyhow!("handshake gRPC server error: {e}"))
    });

    let memory_exchange_grpc_task = tokio::spawn(async move {
        memory_exchange_grpc_fut
            .await
            .map_err(|e| anyhow::anyhow!("[PHOENIX] memory exchange gRPC server error: {e}"))
    });

    let http_task = tokio::spawn(async move {
        http_fut
            .await
            .map_err(|e| anyhow::anyhow!("http server error: {e}"))
    });

    // Use tokio::select! for graceful shutdown handling
    tokio::select! {
        res = grpc_task => {
            res.map_err(|e| anyhow::anyhow!("admin gRPC task join error: {e}"))??;
        }
        res = orchestrator_grpc_task => {
            res.map_err(|e| anyhow::anyhow!("orchestrator gRPC task join error: {e}"))??;
        }
        res = handshake_grpc_task => {
            res.map_err(|e| anyhow::anyhow!("handshake gRPC task join error: {e}"))??;
        }
        res = memory_exchange_grpc_task => {
            res.map_err(|e| anyhow::anyhow!("[PHOENIX] memory exchange gRPC task join error: {e}"))??;
        }
        res = http_task => {
            res.map_err(|e| anyhow::anyhow!("http task join error: {e}"))??;
        }
    }

    Ok(())
}
