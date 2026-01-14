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

### Security Gates (Research Project Access)

These environment variables enable additional capabilities for research purposes. **Use with caution** - these bypass normal security restrictions.

#### Network Scanning Security Gates

- `ALLOW_PUBLIC_NETWORK_SCAN`: Enable public IP network scanning (default: `false`)
  - Set to `1`, `true`, `yes`, or `on` to enable
  - Requires `NETWORK_SCAN_HITL_TOKEN` to be set when enabled
  - Allows scanning of public IP addresses (not just RFC1918 private subnets)

- `NETWORK_SCAN_HITL_TOKEN`: HITL token for public network scans (default: unset)
  - Required when `ALLOW_PUBLIC_NETWORK_SCAN=1`
  - Must be provided in scan requests for public IP targets
  - Use a strong, random token value

- `ALLOW_IPV6_NETWORK_SCAN`: Enable IPv6 network scanning (default: `false`)
  - Set to `1`, `true`, `yes`, or `on` to enable
  - Note: Full IPv6 parsing support is planned but not yet fully implemented
  - Currently returns an informative error message

- `ALLOW_ARBITRARY_PORT_SCAN`: Allow custom port ranges for network scanning (default: `false`)
  - Set to `1`, `true`, `yes`, or `on` to enable
  - By default, only ports 8281-8284 (AGI core ports) are scanned
  - When enabled, allows scanning custom port ranges (e.g., "22,80,443" or "1-65535")

#### HITL (Human-In-The-Loop) Bypass Gates

- `BYPASS_HITL_TOOL_EXEC`: Bypass HITL approval for tool execution (default: `false`)
  - Set to `1`, `true`, `yes`, or `on` to enable
  - Allows `tool_exec` actions to proceed without human approval
  - **WARNING**: This removes a critical safety check

- `BYPASS_HITL_MEMORY`: Bypass HITL approval for memory operations (default: `false`)
  - Set to `1`, `true`, `yes`, or `on` to enable
  - Allows `memory_query` and `memory_commit` actions without human approval
  - **WARNING**: This allows unrestricted memory access

- `BYPASS_HITL_KILL_PROCESS`: Bypass HITL approval for process termination (default: `false`)
  - Set to `1`, `true`, `yes`, or `on` to enable
  - Allows `kill_process` actions without human approval
  - **WARNING**: This allows terminating any process without oversight

- `BYPASS_EMAIL_TEAMS_APPROVAL`: Bypass user approval for email/Teams messages (default: `false`)
  - Set to `1`, `true`, `yes`, or `on` to enable
  - Allows `send_email` and `send_teams_message` actions without user approval
  - **WARNING**: This allows sending messages on your behalf automatically

#### Restricted Commands Security Gate

- `ALLOW_RESTRICTED_COMMANDS`: Allow normally restricted commands (default: `false`)
  - Set to `1`, `true`, `yes`, or `on` to enable
  - Allows execution of: `rm`, `delete`, `format`, `shutdown`, `reboot`
  - **WARNING**: These commands can cause data loss or system disruption
  - Even with this enabled, `SAFE_MODE=true` will still restrict these commands

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
5. **Security Gates are for Research Only**: The security gate environment variables (`ALLOW_*`, `BYPASS_*`) are designed for research projects and should be used with extreme caution. They bypass normal security restrictions and can pose significant risks if misused.

## Security Gates Usage Examples

### Example: Enable Public Network Scanning

```bash
# In .env file
ALLOW_PUBLIC_NETWORK_SCAN=1
NETWORK_SCAN_HITL_TOKEN=your-secure-random-token-here
```

### Example: Enable Arbitrary Port Scanning

```bash
# In .env file
ALLOW_ARBITRARY_PORT_SCAN=1
```

Then in your network scan request, you can specify custom ports:
```json
{
  "target": "192.168.1.0/24",
  "ports": "22,80,443,8080"
}
```

### Example: Bypass HITL for Tool Execution (Research Only)

```bash
# In .env file
BYPASS_HITL_TOOL_EXEC=1
```

**⚠️ WARNING**: This removes human oversight from tool execution. Use only in isolated research environments.

### Example: Allow Restricted Commands (Research Only)

```bash
# In .env file
ALLOW_RESTRICTED_COMMANDS=1
```

**⚠️ WARNING**: This allows destructive commands like `rm`, `shutdown`, `reboot`. Use with extreme caution.

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
