//! pagi-companion-core
//!
//! The core AGI logic for the PAGI Digital Twin system.
//! This crate provides the foundational components for memory, planning, and reflection.

pub mod companion {
    pub mod agent {
        use anyhow::Result;
        use tracing::info;

        #[derive(Debug, Clone)]
        pub struct CompanionAgent {
            user_id: String,
        }

        impl CompanionAgent {
            pub async fn new(user_id: &str) -> Result<Self> {
                info!(user_id = user_id, "CompanionAgent initialized (core stub crate)");
                Ok(Self {
                    user_id: user_id.to_string(),
                })
            }

            pub async fn process_user_input(&self, input: String) -> Result<String> {
                info!(user_id = %self.user_id, input = %input, "Processing input (core stub crate)");
                tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                Ok(format!("ACK(core): {}", input))
            }
        }
    }
}

/// Memory system module for RAG, episodic memory, and semantic memory.
pub mod memory;

/// Agent module containing the DigitalTwinAgent and related protocol types.
pub mod agent;

// Re-export key types for convenience
pub use memory::{MemorySystem, MemoryBlock, MemoryQuery, MemoryQueryResult, MemoryType, DebugMemorySystem};
pub use agent::{DigitalTwinAgent, ExternalLLM, LLMCallInput, LLMCallOutput, ChatRequest, ChatResponse, AgentCommand};