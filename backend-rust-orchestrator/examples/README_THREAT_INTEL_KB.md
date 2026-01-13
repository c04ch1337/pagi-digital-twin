# Threat Intelligence Knowledge Base Ingestion

This script ingests the Threat Intelligence Knowledge Base into the Qdrant Memory Service for use by The Blue Flame orchestrator.

## Overview

The threat intelligence KB is structured into semantic chunks stored in the `threat_intel` namespace:

1. **C2 Beaconing Indicators**: Detection methods and response procedures for command and control communication
2. **Lateral Movement Indicators**: Techniques and detection for attacker lateral movement within networks
3. **Malware Analysis Framework**: Systematic approach to malware analysis (static, dynamic, behavioral)
4. **PowerShell Obfuscation Detection**: Detection and analysis of obfuscated PowerShell attacks
5. **Zero-Trust Policy Framework**: Implementation principles and enforcement guidelines

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
cargo run --example ingest_threat_intel_kb
```

### With custom memory service URL:

```bash
MEMORY_GRPC_URL=http://127.0.0.1:50052 cargo run --example ingest_threat_intel_kb
```

## What It Does

1. Connects to the Memory Service gRPC endpoint
2. Creates/ensures the `threat_intel` namespace (collection) exists in Qdrant
3. Ingests 5 threat intelligence chunks with metadata:
   - `source: threat_intel_kb`
   - `priority: high`
   - Category-specific metadata (IOC, analysis framework, policy)
   - Severity levels (critical, high, medium)

## Verification

After ingestion, you can verify the KB is accessible by querying The Blue Flame orchestrator:

**Example Query:**
```
"What are the indicators of C2 beaconing activity?"
```

**Expected Response:**
The orchestrator should reference the C2 beaconing indicators from the threat intelligence KB, including network connection patterns, DNS queries, and detection methods.

## Integration with System Prompt

The system prompt (`config/system_prompt.txt`) has been updated to include:

- **Namespace Documentation**: Instructions to use `threat_intel` namespace for security-related queries
- **Threat Analysis Guidance**: Framework for querying threat intelligence when analyzing security incidents

## Re-ingestion

If you need to update the threat intelligence KB:

1. Edit the chunk content in `examples/ingest_threat_intel_kb.rs`
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
- Ensure you're querying the `threat_intel` namespace
- Check memory service logs for query errors
