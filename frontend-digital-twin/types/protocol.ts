// PAGI Chat Protocol Types
// Matches pagi-chat-desktop-backend/src/protocol.rs exactly

// --- 3. Structured Command (Agent controlling the UI) ---
export type AgentCommand =
  | {
      command: 'show_memory_page';
      memory_id: string; // UUID as string
      query: string;
    }
  | {
      command: 'prompt_for_config';
      config_key: string;
      prompt: string;
    }
  | {
      command: 'execute_tool';
      tool_name: string;
      arguments: any; // serde_json::Value equivalent
    };

// --- 1. Request from Frontend (User Input) ---
export interface ChatRequest {
  session_id: string; // UUID as string
  user_id: string; // Matches path param /ws/chat/:user_id
  timestamp: string; // ISO 8601 datetime string (DateTime<Utc>)
  message: string;
  /**
   * True when the operator is actively recording and/or screensharing.
   * Used by the Orchestrator to reason about multi-modal context.
   */
  media_active?: boolean;
}

// --- 2. Response to Frontend (Agent Output) ---
// Discriminated union matching Rust backend's ChatResponse enum

export interface CompleteMessage {
  type: 'complete_message';
  id: string; // UUID as string
  content: string;
  is_final: boolean;
  latency_ms: number;
  source_memories: string[]; // RAG sources cited
  issued_command: AgentCommand | null;
  raw_orchestrator_decision?: string | null;
}

export interface MessageChunk {
  type: 'message_chunk';
  id: string; // UUID as string
  content_chunk: string;
  is_final: boolean;
}

export interface StatusUpdate {
  type: 'status_update';
  status: string; // e.g., "error", "ready", "busy"
  details: string | null;
}

export type ChatResponse = CompleteMessage | MessageChunk | StatusUpdate;
