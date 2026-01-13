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
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tonic::transport::Channel;
use tracing::{info, warn, error};
use uuid::Uuid;
use tower_http::cors::{Any, CorsLayer};

const DEFAULT_SYSTEM_PROMPT_TEMPLATE: &str = include_str!("../config/system_prompt.txt");

// P50: Analyst System Prompt for transcript summarization
const ANALYST_SYSTEM_PROMPT: &str = "Summarize the following transcript into 3 sentences. Identify key decisions and action items. Output strictly as JSON: { \"summary\": \"...\", \"decisions\": [], \"tasks\": [] }.";

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

use memory_client::memory_service_client::MemoryServiceClient;
use memory_client::{
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
mod health;
use tools::system::{get_logs, manage_service, read_file, run_command, systemctl, write_file};
use tools::get_system_snapshot;
use health::HealthManager;

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
struct HealthResponse {
    service: &'static str,
    status: &'static str,
    version: &'static str,
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

    // Health check manager
    health_manager: HealthManager,
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
        "command_exec" | "file_write" | "vector_query" | "run_command" | "read_file" | "write_file" | "systemctl" | "manage_service" | "get_logs" | "agi-doctor"
    )
}

/// Check if a tool is a system tool that should be executed directly in the orchestrator
/// rather than through the Tools Service.
pub fn is_system_tool(tool_name: &str) -> bool {
    matches!(tool_name, "run_command" | "read_file" | "write_file" | "systemctl" | "manage_service" | "get_logs" | "agi-doctor")
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
) -> Result<(LLMAction, String), String> {
    info!(
        user_message = %user_message,
        twin_id = %twin_id,
        "OpenRouter LLM planning"
    );

    // Always use the current, live system prompt template.
    // The template may include "{twin_id}" which will be substituted here.
    let template = state.system_prompt.get_template().await;
    let base = if template.trim().is_empty() {
        DEFAULT_SYSTEM_PROMPT_TEMPLATE.to_string()
    } else {
        template
    };
    let mut system_prompt = base.replace("{twin_id}", twin_id);
    if media_active {
        system_prompt.push_str("\n\n[CONTEXT: MULTI-MODAL ACTIVE]\n");
        system_prompt.push_str("media_active=true - The operator is currently recording voice/video and/or sharing their screen in real-time.\n");
        system_prompt.push_str("- You are aware that live multi-modal input (audio/video/screen) is being captured.\n");
        system_prompt.push_str("- You can reference this in your responses (e.g., 'I see you're currently recording...').\n");
        system_prompt.push_str("- Use ActionListRecordings to discover and reference stored recordings from previous sessions.\n");
        system_prompt.push_str("- Provide context-aware responses that account for the visual/audio information being captured.\n");
    }

    // Build the API request body
    let payload = json!({
        "model": state.openrouter_model,
        "messages": [
            {
                "role": "system",
                "content": system_prompt
            },
            {
                "role": "user",
                "content": user_message
            }
        ],
        "response_format": {
            "type": "json_object"
        },
        "temperature": 0.1
    });

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

    // Extract the content from the response
    let content = api_response
        .get("choices")
        .and_then(|choices| choices.as_array())
        .and_then(|arr| arr.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|msg| msg.get("content"))
        .and_then(|c| c.as_str())
        .ok_or_else(|| "Failed to extract content from OpenRouter response".to_string())?;

    info!(
        content = %content,
        "Received LLM response from OpenRouter"
    );

    // Parse the structured JSON into LLMAction
    let llm_action: LLMAction = serde_json::from_str(content)
        .map_err(|e| format!("Failed to parse LLM JSON response: {}. Raw content: {}", e, content))?;

    Ok((llm_action, content.to_string()))
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

    let content = api_response
        .get("choices")
        .and_then(|choices| choices.as_array())
        .and_then(|arr| arr.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|msg| msg.get("content"))
        .and_then(|c| c.as_str())
        .ok_or_else(|| "Failed to extract content from OpenRouter response".to_string())?;

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
        match llm_plan_openrouter(&request.message, &request.twin_id, &state, request.media_active).await {
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
                        "Tool '{}' is not available. Supported tools: command_exec, file_write, vector_query, run_command, read_file, write_file, systemctl, manage_service, get_logs.",
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
    State(state): State<Arc<AppState>>,
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

    // Extract the content from the response
    let content = match api_response
        .get("choices")
        .and_then(|choices| choices.as_array())
        .and_then(|arr| arr.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|msg| msg.get("content"))
        .and_then(|c| c.as_str())
    {
        Some(c) => c,
        None => {
            error!("Failed to extract content from OpenRouter response");
            return ResponseJson(SummarizeTranscriptResponse {
                success: false,
                insights: None,
                error: Some("Failed to extract content from OpenRouter response".to_string()),
            });
        }
    };

    // Parse the structured JSON into TranscriptInsights
    let insights: TranscriptInsights = match serde_json::from_str(content) {
        Ok(i) => i,
        Err(e) => {
            error!(error = %e, content = %content, "Failed to parse insights JSON");
            return ResponseJson(SummarizeTranscriptResponse {
                success: false,
                insights: None,
                error: Some(format!("Failed to parse insights JSON: {}. Raw content: {}", e, content)),
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

async fn health_check() -> ResponseJson<HealthResponse> {
    ResponseJson(HealthResponse {
        service: "backend-rust-orchestrator",
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
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

    // Create health manager first
    let health_manager = HealthManager::new();

    // Create application state
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
        health_manager,
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

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/chat", post(handle_chat_request))
        .route("/v1/memory/list", post(handle_memory_list))
        .route("/v1/memory/delete", post(handle_memory_delete))
        .route("/v1/prompt/history", get(handle_prompt_history))
        .route("/v1/prompt/restore", post(handle_prompt_restore))
        .route("/api/system/snapshot", get(handle_system_snapshot))
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

    let admin_svc = OrchestratorAdminServiceImpl { prompt_mgr };
    let orchestrator_svc = OrchestratorServiceImpl {
        state: Arc::clone(&state),
    };

    info!(addr = %admin_addr, port = admin_grpc_port, "Starting Orchestrator Admin gRPC server");
    info!(addr = %orchestrator_grpc_addr, port = orchestrator_grpc_port, "Starting Orchestrator gRPC server");
    info!(addr = %addr, port = http_port, "Starting Orchestrator HTTP server");

    // Start periodic health checks (every 30 seconds)
    let health_check_interval = env::var("HEALTH_CHECK_INTERVAL_SECS")
        .unwrap_or_else(|_| "30".to_string())
        .parse::<u64>()
        .unwrap_or(30);
    state.health_manager.start_periodic_checks(Arc::clone(&state), health_check_interval);
    info!(interval = health_check_interval, "Started periodic health checks");

    let listener = tokio::net::TcpListener::bind(&addr).await?;

    let grpc_fut = tonic::transport::Server::builder()
        .add_service(OrchestratorAdminServiceServer::new(admin_svc))
        .serve(admin_addr);

    let orchestrator_grpc_fut = tonic::transport::Server::builder()
        .add_service(OrchestratorServiceServer::new(orchestrator_svc))
        .serve(orchestrator_grpc_addr);

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

    let http_task = tokio::spawn(async move {
        http_fut
            .await
            .map_err(|e| anyhow::anyhow!("http server error: {e}"))
    });

    let (grpc_res, orchestrator_grpc_res, http_res) = tokio::join!(grpc_task, orchestrator_grpc_task, http_task);
    grpc_res.map_err(|e| anyhow::anyhow!("admin gRPC task join error: {e}"))??;
    orchestrator_grpc_res.map_err(|e| anyhow::anyhow!("orchestrator gRPC task join error: {e}"))??;
    http_res.map_err(|e| anyhow::anyhow!("http task join error: {e}"))??;

    Ok(())
}
