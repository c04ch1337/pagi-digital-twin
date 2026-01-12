# backend-rust-gateway

Rust-based WebSocket Gateway service for the PAGI Digital Twin platform. Bridges WebSocket connections to HTTP services.

## Overview

The Gateway is the external-facing entry point that:
- Accepts WebSocket connections from the frontend
- Maintains long-lived user sessions
- Proxies chat messages to the Orchestrator HTTP service
- Translates between WebSocket and HTTP protocols

## Features

- **WebSocket Server**: Axum-based WebSocket handler for chat connections
- **Protocol Translation**: Converts WebSocket messages to HTTP requests
- **Session Management**: Maintains WebSocket connections per user
- **Error Handling**: Graceful error handling and client notification
- **Health Check**: Service health monitoring endpoint

## Protocol

### WebSocket Endpoint

- `GET /ws/chat/:user_id`: WebSocket connection for chat

### Message Flow

1. **Client → Gateway (WebSocket)**: `ChatRequest` JSON
2. **Gateway → Orchestrator (HTTP)**: `POST /chat` with `OrchestratorRequest`
3. **Orchestrator → Gateway (HTTP)**: `OrchestratorResponse`
4. **Gateway → Client (WebSocket)**: `ChatResponse` JSON

### Request Format (WebSocket)

```json
{
  "session_id": "uuid",
  "user_id": "twin-sentinel",
  "timestamp": "2024-01-10T10:00:00Z",
  "message": "Search for recent SSH attacks"
}
```

### Response Format (WebSocket)

```json
{
  "type": "complete_message",
  "id": "uuid",
  "content": "Found 3 memory results...",
  "is_final": true,
  "latency_ms": 150,
  "source_memories": ["Memory query: 3 results found"],
  "issued_command": null
}
```

## Configuration

Environment variables:

- `GATEWAY_PORT`: Gateway server port (default: `8181`)
- `ORCHESTRATOR_URL`: Orchestrator HTTP service URL (default: `http://127.0.0.1:8182`)
- `TELEMETRY_URL`: Telemetry service URL (default: `http://127.0.0.1:8183`)
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
GATEWAY_PORT=8181 \
ORCHESTRATOR_URL=http://127.0.0.1:8182 \
TELEMETRY_URL=http://127.0.0.1:8183 \
cargo run
```

## Architecture

```
Frontend (WebSocket Client)
  ↓ WebSocket
Gateway (this service)
  ↓ HTTP POST /chat
Orchestrator
  ↓ gRPC
  ├── Memory Service
  └── Tools Service
```

## Future Enhancements

- **SSE Telemetry Proxy**: Full implementation of SSE streaming proxy
- **Connection Pooling**: Reuse HTTP connections to Orchestrator
- **Rate Limiting**: Per-user rate limiting
- **Authentication**: Add authentication/authorization layer
- **Metrics**: Prometheus metrics for monitoring
