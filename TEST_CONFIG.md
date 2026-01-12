# Test Configuration - Port Layout

This document summarizes the port layout for the **Ferrellgas AGI Multi Digital Twin Platform** services.

## Service Port Matrix

| Service | Port | Protocol | Connected To | Role |
| --- | --- | --- | --- | --- |
| **Frontend** | `3000` | HTTP | Gateway (`8181`) | UI, AGI Interaction |
| **Gateway** | `8181` | WS, SSE, HTTP | Orchestrator, Telemetry | Protocol Bridge |
| **Orchestrator** | `8182` | HTTP, gRPC Client | Memory, Tools | AGI Core, Planning |
| **Memory Service** | `50052` | gRPC | Orchestrator | Semantic Lookup |
| **Tools Service** | `50054` | gRPC | Orchestrator | Secure Execution |
| **Telemetry** | `8183` | HTTP (SSE) | Gateway (Proxy) | Observability Stream |

## Communication Flow

```
Frontend (3000)
  ↓ WebSocket
Gateway (8181)
  ↓ HTTP POST /chat
Orchestrator (8182)
  ↓ gRPC
  ├── Memory Service (50052)
  └── Tools Service (50054)
```

## Environment Variables

### Gateway Service
- `GATEWAY_PORT=8181`
- `ORCHESTRATOR_URL=http://orchestrator:8182`
- `TELEMETRY_URL=http://telemetry:8183`

### Orchestrator Service
- `ORCHESTRATOR_HTTP_PORT=8182`
- `MEMORY_GRPC_ADDR=http://memory-service:50052`
- `TOOLS_GRPC_ADDR=http://tools-service:50054`

### Memory Service
- `MEMORY_GRPC_PORT=50052`

### Tools Service
- `TOOLS_GRPC_PORT=50054`
- `SANDBOX_DIR=/sandbox`
- `SAFE_MODE=true`

### Frontend
- `VITE_WS_URL=ws://localhost:8181/ws/chat`
- `VITE_SSE_URL=http://localhost:8181/v1/telemetry/stream`

## Bare-Metal Deployment

For bare-metal deployment (without Docker), ensure:

1. **Port Availability**: All listed ports must be available
2. **Service Order**: Start services in dependency order:
   - Memory Service (50052)
   - Tools Service (50054)
   - Orchestrator (8182)
   - Gateway (8181)
   - Frontend (3000)

3. **Environment Variables**: Set all required environment variables as listed above

## Docker Compose Deployment

Use the provided `docker-compose.yml` to launch all services:

```bash
docker-compose up --build
```

This will:
- Build all Rust services
- Configure all environment variables
- Set up service networking
- Expose required ports

## Testing

### Health Checks

- Gateway: `curl http://localhost:8181/api/health`
- Orchestrator: `curl http://localhost:8182/health`
- Memory Service: `grpc_health_probe -addr localhost:50052`
- Tools Service: `grpc_health_probe -addr localhost:50054`

### WebSocket Test

```bash
# Connect to Gateway WebSocket
wscat -c ws://localhost:8181/ws/chat/test-user

# Send a test message
{"session_id":"test-session","user_id":"test-user","timestamp":"2024-01-10T10:00:00Z","message":"Hello"}
```

## Notes

- All gRPC services use internal Docker networking (service names)
- WebSocket connections are exposed on host ports for frontend access
- Frontend should connect to `localhost` ports when running locally
- In production, use proper domain names and TLS for WebSocket connections
