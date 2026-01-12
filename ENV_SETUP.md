# Environment Configuration Setup

This document describes how to set up the unified environment configuration for the **Ferrellgas AGI Multi Digital Twin Platform**.

## Quick Start

1. **Copy the example file:**
   ```bash
   cp .env.example .env
   ```

2. **Edit `.env` and set your OpenRouter API key:**
   ```bash
   OPENROUTER_API_KEY=sk-your-actual-api-key-here
   ```

3. **For bare-metal deployment:** Source the file before starting services:
   ```bash
   source .env  # or: export $(cat .env | xargs)
   ```

4. **For Docker Compose:** The `docker-compose.yml` automatically loads `.env` via `env_file: .env`

## Configuration Variables

### Required Variables

- `OPENROUTER_API_KEY`: **REQUIRED** - Your OpenRouter API key
  - Get it from: https://openrouter.ai/
  - Format: `sk-or-...`

### Optional Variables (with defaults)

- `OPENROUTER_MODEL`: LLM model for planning (default: `google/gemini-2.0-flash-exp`)
- `LOG_LEVEL`: Logging verbosity (default: `info`)
- `SAFE_MODE`: Enable safe mode for tools (default: `false`)
- `SANDBOX_DIR`: Sandbox directory path (default: `/tmp/pagi-sandbox`)

### Service Ports

All ports can be customized, but defaults are:
- `GATEWAY_PORT=8181`
- `ORCHESTRATOR_HTTP_PORT=8182`
- `MEMORY_GRPC_PORT=50052`
- `TOOLS_GRPC_PORT=50054`

### Service URLs

For **bare-metal**, these point to `localhost`:
- `ORCHESTRATOR_URL=http://127.0.0.1:8182`
- `MEMORY_GRPC_ADDR=http://127.0.0.1:50052`
- `TOOLS_GRPC_ADDR=http://127.0.0.1:50054`

For **Docker Compose**, these are automatically overridden with service names:
- `ORCHESTRATOR_URL=http://rust-orchestrator:8182`
- `MEMORY_GRPC_ADDR=http://rust-memory-service:50052`
- `TOOLS_GRPC_ADDR=http://rust-tools-service:50054`

## Bare-Metal Deployment

When running services directly (not in Docker), you need to source the `.env` file:

```bash
# Option 1: Source the file
source .env

# Option 2: Export all variables
export $(cat .env | grep -v '^#' | xargs)

# Option 3: Use dotenvy (Rust services do this automatically)
# Services using dotenvy will load .env automatically
```

Then start services:
```bash
cd backend-rust-memory && cargo run
cd backend-rust-tools && cargo run
cd backend-rust-orchestrator && cargo run
cd backend-rust-gateway && cargo run
cd frontend-digital-twin && npm run dev
```

## Docker Compose Deployment

Docker Compose automatically loads `.env` for all services via `env_file: .env`.

Simply ensure `.env` exists and run:
```bash
docker-compose up --build
```

## Security Notes

1. **Never commit `.env` to version control** - it's in `.gitignore`
2. **Use `.env.example`** as a template for documentation
3. **Rotate API keys** regularly in production
4. **Use different keys** for development and production

## Troubleshooting

### Services can't find environment variables

**Bare-metal:**
- Ensure `.env` is in the project root
- Source the file: `source .env`
- Check that `dotenvy` is loading the file (Rust services)

**Docker:**
- Ensure `.env` exists in the project root
- Check `docker-compose.yml` has `env_file: .env` for each service
- Verify variable names match exactly (case-sensitive)

### OpenRouter API errors

- Verify `OPENROUTER_API_KEY` is set correctly
- Check the API key is valid at https://openrouter.ai/
- Ensure the model name in `OPENROUTER_MODEL` is available

### Port conflicts

- Check if ports are already in use: `lsof -i :8181`
- Change ports in `.env` if needed
- Update service URLs if you change ports
