use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// --- 1. Request from Frontend (User Input) ---
#[derive(Debug, Deserialize, Clone)]
pub struct ChatRequest {
    pub session_id: Uuid,
    pub user_id: String,
    pub timestamp: DateTime<Utc>,
    pub message: String,
}

// --- 2. Response to Frontend (Agent Output) ---
#[derive(Debug, Serialize, Clone)]
#[serde(tag = "type")]
pub enum ChatResponse {
    // For final, complete responses
    #[serde(rename = "complete_message")]
    CompleteMessage {
        id: Uuid,
        content: String,
        is_final: bool,
        latency_ms: u64,
        source_memories: Vec<String>, // RAG sources cited
        issued_command: Option<AgentCommand>,
    },

    // For streaming responses (if desired later)
    #[serde(rename = "message_chunk")]
    MessageChunk {
        id: Uuid,
        content_chunk: String,
        is_final: bool,
    },

    // For status updates (e.g., LLM call failed, memory loaded)
    #[serde(rename = "status_update")]
    StatusUpdate {
        status: String,
        details: Option<String>,
    },
}

// --- 3. Structured Command (Agent controlling the UI) ---
#[derive(Debug, Serialize, Clone)]
#[serde(tag = "command")]
pub enum AgentCommand {
    #[serde(rename = "show_memory_page")]
    ShowMemoryPage { memory_id: Uuid, query: String },

    #[serde(rename = "prompt_for_config")]
    PromptForConfig { config_key: String, prompt: String },

    #[serde(rename = "execute_tool")]
    ExecuteTool {
        tool_name: String,
        arguments: serde_json::Value,
    },
}

