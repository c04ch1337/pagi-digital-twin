use async_trait::async_trait;
use anyhow::Result;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use std::sync::Arc;
use tokio::sync::RwLock;

// --- 1. Memory Block Structures ---

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum MemoryType {
    Episodic, // Direct user conversation/events
    Semantic, // Fact/concept derived from reflection
    RAGSource, // External document/knowledge source
    Reflection, // The output of a reflection step
}

/// The fundamental unit of memory stored in the system.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MemoryBlock {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub memory_type: MemoryType,
    pub content: String,
    pub embedding: Vec<f32>, // Placeholder for vector embedding
}

// --- 2. Query and Result Structures ---

/// Structured input for retrieving memories (RAG).
#[derive(Debug, Clone)]
pub struct MemoryQuery {
    pub user_id: String,
    pub query_text: String,
    pub retrieval_limit: usize,
    pub types_to_include: Vec<MemoryType>,
}

/// Structured output from a memory retrieval operation.
#[derive(Debug, Clone)]
pub struct MemoryQueryResult {
    pub retrieved_blocks: Vec<MemoryBlock>,
    pub context_summary: String, // Context built for the LLM prompt
}

// --- 3. Core Memory Trait ---

#[async_trait]
pub trait MemorySystem: Send + Sync {
    /// Retrieves relevant memory blocks based on the query.
    async fn retrieve(&self, query: &MemoryQuery) -> Result<MemoryQueryResult>;
    
    /// Persists a new memory block to the store.
    async fn store(&self, block: MemoryBlock) -> Result<()>;
}

// --- 4. Debug/Test Implementation ---

/// A simple, non-persistent, in-memory system for immediate integration.
/// This allows the DigitalTwinAgent to be wired immediately without external database dependencies.
#[derive(Debug, Default)]
pub struct DebugMemorySystem {
    // Stores memories in a simple vector for debug purposes
    memories: Arc<RwLock<Vec<MemoryBlock>>>,
}

impl DebugMemorySystem {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl MemorySystem for DebugMemorySystem {
    async fn retrieve(&self, query: &MemoryQuery) -> Result<MemoryQueryResult> {
        tracing::debug!(
            user_id = %query.user_id,
            query_text = %query.query_text,
            retrieval_limit = query.retrieval_limit,
            "Retrieving memories from DebugMemorySystem"
        );

        // Mock retrieval: returns a single fixed block
        // In production, this would perform vector similarity search
        let blocks = vec![
            MemoryBlock {
                id: Uuid::new_v4(),
                timestamp: Utc::now(),
                memory_type: MemoryType::Semantic,
                content: format!(
                    "Semantic Memory: The user {}'s goal is to build an AGI Digital Twin for research.",
                    query.user_id
                ),
                embedding: vec![],
            }
        ];
        
        // Mock context summary
        let context_summary = format!(
            "User {} is engaging in research. Current time: {}. Query: {}",
            query.user_id,
            Utc::now(),
            query.query_text
        );
        
        Ok(MemoryQueryResult {
            retrieved_blocks: blocks,
            context_summary,
        })
    }
    
    async fn store(&self, block: MemoryBlock) -> Result<()> {
        tracing::info!(
            memory_id = %block.id,
            memory_type = ?block.memory_type,
            "Storing memory block in DebugMemorySystem"
        );
        self.memories.write().await.push(block);
        Ok(())
    }
}
