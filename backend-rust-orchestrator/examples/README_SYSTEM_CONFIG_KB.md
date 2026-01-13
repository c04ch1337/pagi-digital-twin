# System Configuration Knowledge Base Ingestion

This script ingests the System Configuration Knowledge Base into the Qdrant Memory Service for use by The Blue Flame orchestrator.

## Overview

The system configuration KB is structured into documentation chunks stored in the `system_config` namespace:

1. **PAGI Digital Twin Platform Architecture**: Overview of microservices and their roles
2. **Service Port Configuration**: Default ports and environment variable overrides
3. **Memory Service Configuration**: Qdrant setup, namespaces, and memory types
4. **Orchestrator LLM Configuration**: OpenRouter API setup and system prompt management
5. **Tools Service Sandbox Configuration**: Sandbox environment and policy configuration
6. **Telemetry Service Configuration**: Monitoring, media recording, and asset management
7. **Environment Variable Reference**: Complete list of configuration variables

## Prerequisites

1. **Memory Service Running**: The memory service (backend-rust-memory) must be running and accessible
   - Default gRPC endpoint: `http://127.0.0.1:50052`
   - Override via `MEMORY_GRPC_URL` environment variable

2. **Qdrant Running** (if using Qdrant backend):
   - Default: `http://127.0.0.1:6334`
   - Override via `QDRANT_URL` environment variable

## Usage

### From the orchestrator directory:

```bash
cd backend-rust-orchestrator
cargo run --example ingest_system_config_kb
```

### With custom memory service URL:

```bash
MEMORY_GRPC_URL=http://127.0.0.1:50052 cargo run --example ingest_system_config_kb
```

## What It Does

1. Connects to the Memory Service gRPC endpoint
2. Creates/ensures the `system_config` namespace (collection) exists in Qdrant
3. Ingests 7 system configuration chunks with metadata:
   - `source: system_config_kb`
   - `priority: high`
   - Component classification (platform, networking, service_config, etc.)

## Verification

After ingestion, you can verify the KB is accessible by querying The Blue Flame orchestrator:

**Example Query:**
```
"What ports does the Gateway service use?"
```

**Expected Response:**
The orchestrator should reference the service port configuration, indicating the Gateway uses port 8181 by default.

## Integration with System Prompt

The system prompt (`config/system_prompt.txt`) has been updated to include:

- **Namespace Documentation**: Instructions to use `system_config` namespace for configuration queries
- **Configuration Access**: Framework for retrieving system configuration when troubleshooting or making changes

## Re-ingestion

If you need to update the system configuration KB:

1. Edit the chunk content in `examples/ingest_system_config_kb.rs`
2. Re-run the ingestion script
3. The script will create new memory entries (old entries remain unless explicitly deleted)

## Troubleshooting

**Error: "Failed to connect to Memory Service"**
- Ensure `backend-rust-memory` is running
- Check `MEMORY_GRPC_URL` environment variable

**Error: "Failed to commit memory"**
- Check Qdrant is running (if using Qdrant backend)
- Verify `QDRANT_URL` environment variable
- Check memory service logs for details

**No results when querying**
- Verify ingestion completed successfully (check script output)
- Ensure you're querying the `system_config` namespace
- Check memory service logs for query errors
