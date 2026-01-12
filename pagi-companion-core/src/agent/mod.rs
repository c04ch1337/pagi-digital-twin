use anyhow::Result;
use std::sync::Arc;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::memory::{MemorySystem, MemoryQuery, MemoryQueryResult, MemoryBlock, MemoryType};

// --- Protocol and LLM Client Trait Placeholders (Required to break circular dependency) ---
// These are defined here so the Agent can use them without depending directly on the backend wrapper's implementation.

/// Trait for external LLM services (Tactical LLM, Reflection LLM).
/// This allows the core agent to communicate with LLM services without depending on the backend wrapper.
#[async_trait]
pub trait ExternalLLM: Send + Sync {
    /// Calls the external LLM service with the prompt and expected schema.
    async fn call(&self, input: LLMCallInput) -> Result<LLMCallOutput>;
}

/// Represents the request data sent to the external LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMCallInput {
    pub prompt: String,
    /// Used to inform the external LLM service which structured JSON output is expected.
    pub schema_name: String,
    pub temperature: f32,
}

/// Represents the structured response received from the external LLM.
#[derive(Debug, Clone, Deserialize)]
pub struct LLMCallOutput {
    pub raw_response: String,
    /// This is the structured JSON output (e.g., CommandSequence or ReflectionOutput)
    pub json_body: serde_json::Value,
}

// --- Protocol Types (Required for Agent Communication) ---
// These types mirror the backend wrapper's protocol to allow the agent to process requests
// without creating a circular dependency.

/// Request from the frontend/user to the agent.
#[derive(Debug, Clone, Deserialize)]
pub struct ChatRequest {
    pub session_id: Uuid,
    pub user_id: String,
    pub timestamp: DateTime<Utc>,
    pub message: String,
}

/// Response from the agent to the frontend/user.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum ChatResponse {
    /// For final, complete responses
    #[serde(rename = "complete_message")]
    CompleteMessage {
        id: Uuid,
        content: String,
        is_final: bool,
        latency_ms: u64,
        source_memories: Vec<String>, // RAG sources cited
        issued_command: Option<AgentCommand>,
    },
    /// For streaming responses (if desired later)
    #[serde(rename = "message_chunk")]
    MessageChunk {
        id: Uuid,
        content_chunk: String,
        is_final: bool,
    },
    /// For status updates (e.g., LLM call failed, memory loaded)
    #[serde(rename = "status_update")]
    StatusUpdate {
        status: String,
        details: Option<String>,
    },
}

/// Structured command that the agent can issue to control the UI.
#[derive(Debug, Clone, Serialize)]
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

// --- The Digital Twin Agent ---

/// The core AGI agent that processes user input through the cognitive loop:
/// 1. RAG Context Retrieval (MemorySystem)
/// 2. Tactical LLM Planning (ExternalLLM)
/// 3. Command Execution (placeholder)
/// 4. Reflection LLM (ExternalLLM)
/// 5. Memory Storage (MemorySystem)
pub struct DigitalTwinAgent {
    pub user_id: String,
    /// The LLM client for Tactical and Reflection LLM calls
    llm_client: Arc<dyn ExternalLLM>,
    /// The memory system for RAG retrieval and episodic/semantic storage
    memory_system: Arc<dyn MemorySystem>,
}

// Manual Debug implementation since trait objects can't derive Debug
impl std::fmt::Debug for DigitalTwinAgent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DigitalTwinAgent")
            .field("user_id", &self.user_id)
            .field("llm_client", &"Arc<dyn ExternalLLM>")
            .field("memory_system", &"Arc<dyn MemorySystem>")
            .finish()
    }
}

impl DigitalTwinAgent {
    /// Creates a new DigitalTwinAgent with the provided LLM client and memory system.
    pub fn new(
        user_id: String,
        llm_client: Arc<dyn ExternalLLM>,
        memory_system: Arc<dyn MemorySystem>,
    ) -> Self {
        tracing::info!(
            user_id = %user_id,
            "DigitalTwinAgent created with LLM client and memory system"
        );
        Self {
            user_id,
            llm_client,
            memory_system,
        }
    }

    /// The core cognitive function of the AGI Digital Twin.
    /// Processes user input through the complete cognitive loop.
    pub async fn process_user_input(&self, request: ChatRequest) -> Result<ChatResponse> {
        let started = std::time::Instant::now();
        tracing::info!(
            user_id = %self.user_id,
            session_id = %request.session_id,
            message = %request.message,
            "Processing user input through AGI cognitive loop"
        );

        // --- Step 1: RAG Context Retrieval (MemorySystem interaction) ---
        let query = MemoryQuery {
            user_id: self.user_id.clone(),
            query_text: request.message.clone(),
            retrieval_limit: 3,
            types_to_include: vec![MemoryType::Semantic, MemoryType::RAGSource],
        };

        let query_result = self.memory_system.retrieve(&query).await.unwrap_or_else(|e| {
            tracing::error!(
                user_id = %self.user_id,
                error = %e,
                "Memory retrieval failed, using empty context"
            );
            // On failure, use empty context
            MemoryQueryResult {
                retrieved_blocks: vec![],
                context_summary: String::new(),
            }
        });

        let context = query_result.context_summary;
        let source_memories: Vec<String> = query_result
            .retrieved_blocks
            .iter()
            .map(|block| format!("{}: {}", format!("{:?}", block.memory_type), &block.content))
            .collect();

        tracing::debug!(
            context_length = context.len(),
            retrieved_blocks = query_result.retrieved_blocks.len(),
            "Retrieved RAG context from memory system"
        );

        // --- Step 2: Tactical LLM Call (ExternalLLM interaction) ---
        let prompt = format!(
            "Context from Memory: {}\n\nUser Input: {}\n\nGenerate a CommandSequence to respond to the user's request.",
            context,
            request.message
        );

        let llm_input = LLMCallInput {
            prompt,
            schema_name: "CommandSequence".to_string(),
            temperature: 0.7,
        };

        let llm_output = self.llm_client.call(llm_input).await.map_err(|e| {
            tracing::error!(
                user_id = %self.user_id,
                error = %e,
                "Tactical LLM call failed"
            );
            e
        })?;

        tracing::debug!(
            raw_response_length = llm_output.raw_response.len(),
            json_body_keys = ?llm_output.json_body.as_object().map(|o| o.keys().collect::<Vec<_>>()),
            "Received response from Tactical LLM"
        );

        // --- Step 3: Store Episodic Memory (MemorySystem interaction) ---
        let episodic_memory = MemoryBlock {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            memory_type: MemoryType::Episodic,
            content: format!(
                "User said: '{}'. Agent LLM output: {}",
                request.message,
                llm_output.raw_response.chars().take(200).collect::<String>()
            ),
            embedding: vec![],
        };

        if let Err(e) = self.memory_system.store(episodic_memory).await {
            tracing::warn!(
                user_id = %self.user_id,
                error = %e,
                "Failed to store episodic memory (non-critical)"
            );
        }

        // --- Step 4: Reflection LLM Call (Optional, for semantic memory) ---
        // This is non-critical, so we continue even if it fails
        let reflection_prompt = format!(
            "User input: {}\n\nAgent response: {}\n\nGenerate a ReflectionOutput summarizing key insights.",
            request.message,
            llm_output.raw_response.chars().take(500).collect::<String>()
        );

        let reflection_input = LLMCallInput {
            prompt: reflection_prompt,
            schema_name: "ReflectionOutput".to_string(),
            temperature: 0.5,
        };

        if let Ok(reflection_output) = self.llm_client.call(reflection_input).await {
            tracing::debug!("Received ReflectionOutput from Reflection LLM");
            
            // Store reflection as semantic memory
            let reflection_memory = MemoryBlock {
                id: Uuid::new_v4(),
                timestamp: Utc::now(),
                memory_type: MemoryType::Reflection,
                content: format!(
                    "Reflection: {}",
                    reflection_output.raw_response.chars().take(200).collect::<String>()
                ),
                embedding: vec![],
            };

            if let Err(e) = self.memory_system.store(reflection_memory).await {
                tracing::warn!(
                    user_id = %self.user_id,
                    error = %e,
                    "Failed to store reflection memory (non-critical)"
                );
            }
        } else {
            tracing::warn!("Reflection LLM call failed (non-critical, continuing)");
        }

        // --- Step 5: Construct Response ---
        // TODO: Parse CommandSequence from llm_output.json_body and execute commands
        // For now, we return a complete message with the LLM response

        let latency_ms = started.elapsed().as_millis() as u64;

        Ok(ChatResponse::CompleteMessage {
            id: Uuid::new_v4(),
            content: format!(
                "I've processed your request: '{}'\n\nResponse: {}",
                request.message,
                llm_output.raw_response
            ),
            is_final: true,
            latency_ms,
            source_memories,
            issued_command: None, // TODO: Parse from CommandSequence
        })
    }
}
