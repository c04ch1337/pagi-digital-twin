# Centralized Configuration Guide

## Overview

The **Ferrellgas AGI Multi Digital Twin Platform** uses a unified `.env` file for all environment configuration. This provides:

- **Single source of truth** for all configuration
- **Bare-metal and Docker compatibility** - same file works for both
- **Security** - sensitive keys in one place, excluded from version control
- **Ease of deployment** - copy `.env.example` to `.env` and customize

## Setup

### 1. Create `.env` File

```bash
cp .env.example .env
```

### 2. Configure OpenRouter API Key

Edit `.env` and set your OpenRouter API key:

```bash
OPENROUTER_API_KEY=sk-your-actual-api-key-here
```

Get your API key from: https://openrouter.ai/

### 3. Customize (Optional)

Adjust ports, model selection, or other settings as needed. See `.env.example` for all available options.

## Service Configuration

### Rust Services

All Rust services use `dotenvy` to automatically load `.env`:

- `backend-rust-gateway`
- `backend-rust-orchestrator`
- `backend-rust-memory`
- `backend-rust-tools`

They will automatically load `.env` from the project root when started.

### Docker Compose

All services in `docker-compose.yml` use `env_file: .env` to load configuration:

```yaml
services:
  rust-orchestrator:
    env_file:
      - .env
    environment:
      - OPENROUTER_API_KEY=${OPENROUTER_API_KEY}
      # ... other vars
```

Docker Compose automatically:
- Loads variables from `.env`
- Overrides service URLs with Docker service names
- Provides fallback defaults for optional variables

## Variable Reference

### Required

| Variable | Description | Example |
|----------|-------------|---------|
| `OPENROUTER_API_KEY` | OpenRouter API key for LLM | `sk-or-...` |

### Optional (with defaults)

| Variable | Default | Description |
|----------|---------|-------------|
| `OPENROUTER_MODEL` | `google/gemini-2.0-flash-exp` | LLM model for planning |
| `OPENROUTER_URL` | `https://openrouter.ai/api/v1/chat/completions` | OpenRouter API endpoint |
| `LOG_LEVEL` | `info` | Logging verbosity |
| `SAFE_MODE` | `false` | Enable safe mode for tools |
| `SANDBOX_DIR` | `/tmp/pagi-sandbox` | Sandbox directory |
| `GATEWAY_PORT` | `8181` | Gateway HTTP/WS port |
| `ORCHESTRATOR_HTTP_PORT` | `8182` | Orchestrator HTTP port |
| `MEMORY_GRPC_PORT` | `50052` | Memory service gRPC port |
| `TOOLS_GRPC_PORT` | `50054` | Tools service gRPC port |

### Service URLs (Bare-Metal)

| Variable | Default | Description |
|----------|---------|-------------|
| `ORCHESTRATOR_URL` | `http://127.0.0.1:8182` | Orchestrator HTTP endpoint |
| `MEMORY_GRPC_ADDR` | `http://127.0.0.1:50052` | Memory service gRPC address |
| `TOOLS_GRPC_ADDR` | `http://127.0.0.1:50054` | Tools service gRPC address |
| `VITE_WS_URL` | `ws://127.0.0.1:8181/ws/chat` | Frontend WebSocket URL |
| `VITE_SSE_URL` | `http://127.0.0.1:8181/v1/telemetry/stream` | Frontend SSE URL |

**Note:** Docker Compose automatically overrides these with service names (e.g., `http://rust-orchestrator:8182`).

## Deployment

### Bare-Metal

1. **Source the environment:**
   ```bash
   source .env
   # or
   export $(cat .env | grep -v '^#' | xargs)
   ```

2. **Start services:**
   ```bash
   # Terminal 1: Memory Service
   cd backend-rust-memory && cargo run
   
   # Terminal 2: Tools Service
   cd backend-rust-tools && cargo run
   
   # Terminal 3: Orchestrator
   cd backend-rust-orchestrator && cargo run
   
   # Terminal 4: Gateway
   cd backend-rust-gateway && cargo run
   
   # Terminal 5: Frontend
   cd frontend-digital-twin && npm run dev
   ```

### Docker Compose

1. **Ensure `.env` exists:**
   ```bash
   cp .env.example .env
   # Edit .env and set OPENROUTER_API_KEY
   ```

2. **Start all services:**
   ```bash
   docker-compose up --build
   ```

Docker Compose automatically loads `.env` for all services.

## Verification

### Check Environment Variables

**Bare-metal:**
```bash
echo $OPENROUTER_API_KEY
echo $ORCHESTRATOR_HTTP_PORT
```

**Docker:**
```bash
docker-compose exec rust-orchestrator env | grep OPENROUTER
```

### Test Service Configuration

```bash
# Health check
curl http://localhost:8182/health

# Should return service status
```

## Troubleshooting

### Variables Not Loading

1. **Bare-metal:**
   - Ensure `.env` is in project root
   - Source the file: `source .env`
   - Check `dotenvy::dotenv().ok()` is called in service code

2. **Docker:**
   - Verify `.env` exists in project root
   - Check `env_file: .env` in `docker-compose.yml`
   - Restart containers: `docker-compose down && docker-compose up`

### Port Conflicts

If ports are in use, change them in `.env`:

```bash
GATEWAY_PORT=8185
ORCHESTRATOR_HTTP_PORT=8186
```

Then update service URLs accordingly.

### OpenRouter Errors

- Verify API key is correct: `echo $OPENROUTER_API_KEY`
- Check key is valid at https://openrouter.ai/
- Ensure model name is available: `OPENROUTER_MODEL=google/gemini-2.0-flash-exp`

## Security Best Practices

1. **Never commit `.env`** - it's in `.gitignore`
2. **Use `.env.example`** for documentation
3. **Rotate API keys** regularly
4. **Use different keys** for dev/prod
5. **Restrict file permissions:** `chmod 600 .env`

## Migration from Legacy Config

If you have existing environment variables:

1. Review `.env.example` for all current variables
2. Copy relevant values to new `.env`
3. Remove legacy variables (e.g., `PY_AGENT_PORT`, `GO_BFF_URL`)
4. Update service startup scripts to use `.env`

---

**Last Updated:** 2024-01-10  
**Version:** 1.0
