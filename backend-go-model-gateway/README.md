# `backend-go-model-gateway`

Go Model Gateway (gRPC) for routing model/LLM requests and (now) hosting the initial plumbing for Vector DB / RAG retrieval.

## Runtime Interfaces

### gRPC

The primary interface is gRPC (consumed by the Python Agent).

- Port: `MODEL_GATEWAY_GRPC_PORT` (default: `50051`)

### Temporary HTTP (Vector DB test)

For early integration/testing, the gateway also starts a small HTTP server with a temporary endpoint:

- Port: `MODEL_GATEWAY_HTTP_PORT` (default: `8005`)
- Endpoint: `GET /api/v1/vector-test?query=...&k=...`

Example:

```bash
curl "http://localhost:8005/api/v1/vector-test?query=hello&k=3"
```

This endpoint currently calls a mock Vector DB client and returns 2 hardcoded matches (useful for wiring validation).

## Environment Variables

### Core

- `MODEL_GATEWAY_GRPC_PORT` (default: `50051`)
- `MODEL_GATEWAY_HTTP_PORT` (default: `8005`) — temporary HTTP server for vector DB testing
- `REQUEST_TIMEOUT_SECONDS` (default: `5`) — timeout for the upstream LLM call

### LLM Provider Selection

- `LLM_PROVIDER` (default: `openrouter`) — supported: `openrouter`, `ollama`

OpenRouter:

- `OPENROUTER_API_KEY` (required when `LLM_PROVIDER=openrouter`)
- `OPENROUTER_MODEL_NAME` (default: `mistralai/mistral-7b-instruct:free`)

Ollama:

- `OLLAMA_BASE_URL` (default: `http://localhost:11434`)
- `OLLAMA_MODEL_NAME` (default: `llama3`)

### Vector DB (Mock / Future)

These are placeholders for the next phase (real Pinecone/Weaviate/etc.). The current implementation is a mock.

- `VECTOR_DB_PROVIDER` (default: `mock`)
- `VECTOR_DB_BASE_URL` (unused for now)
- `VECTOR_DB_API_KEY` (unused for now)
- `VECTOR_DB_INDEX` (unused for now)

