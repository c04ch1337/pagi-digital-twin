# backend-rust-orchestrator

Rust-based Orchestrator service for the PAGI Digital Twin platform. This is the AGI core that coordinates Memory and Tools services.

## Overview

The Orchestrator is the central decision-making component that:
- Receives chat requests from the frontend via HTTP/REST
- Uses **OpenRouter API** for real AI-powered planning and decision-making
- Delegates to Memory and Tools services via gRPC
- Manages job queues and execution tracking

## Features

- **HTTP/REST Interface**: Axum-based HTTP server for frontend communication
- **gRPC Clients**: Connects to Memory and Tools services
- **OpenRouter LLM Integration**: Real AI-powered planning using OpenRouter API
- **Structured Output**: LLM returns JSON matching `LLMAction` enum for type-safe parsing
- **Job Management**: Tracks execution jobs with progress and logs
- **Action Delegation**: Routes to appropriate services based on LLM decisions

## Protocol

### HTTP Endpoints

- `POST /chat`: Process chat requests and orchestrate actions
- `GET /health`: Health check endpoint

### Request Format

```json
{
  "message": "Search for recent SSH attacks",
  "twin_id": "twin-sentinel",
  "session_id": "uuid",
  "namespace": "threat_intel_v24"
}
```

### Response Format

```json
{
  "response": "Found 3 memory results...",
  "job_id": "uuid",
  "actions_taken": ["Memory query: 3 results found"],
  "status": "completed"
}
```

## Configuration

Environment variables:

- `ORCHESTRATOR_HTTP_PORT`: HTTP server port (default: `8182`)
- `MEMORY_GRPC_ADDR`: Memory service gRPC address (default: `http://127.0.0.1:50052`)
- `TOOLS_GRPC_ADDR`: Tools service gRPC address (default: `http://127.0.0.1:50054`)
- `OPENROUTER_API_KEY`: **REQUIRED** - OpenRouter API key for LLM access
- `OPENROUTER_MODEL`: OpenRouter model name (default: `google/gemini-2.0-flash-exp`)
- `OPENROUTER_URL`: OpenRouter API endpoint (default: `https://openrouter.ai/api/v1/chat/completions`)
- `LOG_LEVEL`: Logging level (default: `info`)

## Building

```bash
cargo build --release
```

## Running

```bash
cargo run
```

Or with environment variables:

```bash
ORCHESTRATOR_HTTP_PORT=8182 \
MEMORY_GRPC_ADDR=http://127.0.0.1:50052 \
TOOLS_GRPC_ADDR=http://127.0.0.1:50054 \
OPENROUTER_API_KEY=your-api-key-here \
OPENROUTER_MODEL=google/gemini-2.0-flash-exp \
cargo run
```

**Note:** `OPENROUTER_API_KEY` is **required**. Get your API key from [OpenRouter.ai](https://openrouter.ai/).

## OpenRouter LLM Integration

The Orchestrator uses **OpenRouter API** for real AI-powered planning and decision-making:

- **Structured Output**: The LLM is prompted to return JSON matching the `LLMAction` enum
- **Action Types**:
  - `ActionMemory`: For searches, queries, and memory lookups
  - `ActionTool`: For system commands, file operations, and tool execution
  - `ActionResponse`: For conversational responses
- **Model Selection**: Configurable via `OPENROUTER_MODEL` (default: `google/gemini-2.0-flash-exp`)
- **Error Handling**: Falls back gracefully if LLM call fails

### Example LLM Response Format

```json
{
  "action_type": "ActionMemory",
  "details": {
    "query": "recent SSH attacks"
  }
}
```

## Architecture

```
Frontend (WebSocket)
  ↓ HTTP/REST
Orchestrator (this service)
  ↓ gRPC
  ├── Memory Service (semantic search)
  └── Tools Service (secure execution)
```

## Future Integration

- **Real LLM**: Replace mock planning with actual LLM calls (Gemini, OpenAI, etc.)
- **Job Persistence**: Store jobs in database for recovery
- **Streaming**: Support streaming responses for real-time updates
- **Multi-turn**: Support conversation context across multiple turns
