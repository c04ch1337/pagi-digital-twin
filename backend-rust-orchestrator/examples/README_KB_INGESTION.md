# Knowledge Base Ingestion Scripts

This directory contains scripts to ingest various knowledge bases into the Qdrant Memory Service for use by The Blue Flame orchestrator.

## Available Knowledge Bases

| KB Name | Namespace | Script | Description |
|---------|-----------|--------|-------------|
| **Leadership KB** | `corporate_context` | `ingest_leadership_kb.rs` | Leadership knowledge, company history, organizational values |
| **Threat Intelligence KB** | `threat_intel` | `ingest_threat_intel_kb.rs` | Security indicators, IOCs, threat analysis frameworks |
| **Incident Response KB** | `incident_response` | `ingest_incident_response_kb.rs` | Incident response playbooks and procedures |
| **System Configuration KB** | `system_config` | `ingest_system_config_kb.rs` | Platform architecture, service configuration, environment variables |

## Prerequisites

1. **Memory Service Running**: The memory service (backend-rust-memory) must be running
   - Default gRPC endpoint: `http://127.0.0.1:50052`
   - Override via `MEMORY_GRPC_URL` environment variable

2. **Qdrant Running** (if using Qdrant backend):
   - Default: `http://127.0.0.1:6334`
   - Override via `QDRANT_URL` environment variable

## Quick Start

### Ingest All Knowledge Bases

```bash
cd backend-rust-orchestrator

# Ingest Leadership KB
cargo run --example ingest_leadership_kb

# Ingest Threat Intelligence KB
cargo run --example ingest_threat_intel_kb

# Ingest Incident Response KB
cargo run --example ingest_incident_response_kb

# Ingest System Configuration KB
cargo run --example ingest_system_config_kb
```

### With Custom Memory Service URL

```bash
MEMORY_GRPC_URL=http://127.0.0.1:50052 cargo run --example ingest_leadership_kb
```

## Individual KB Documentation

- [Leadership KB](README_LEADERSHIP_KB.md) - Corporate context and leadership knowledge
- [Threat Intelligence KB](README_THREAT_INTEL_KB.md) - Security indicators and threat analysis
- [Incident Response KB](README_INCIDENT_RESPONSE_KB.md) - Incident response playbooks
- [System Configuration KB](README_SYSTEM_CONFIG_KB.md) - Platform architecture and configuration

## Verification

After ingestion, verify each KB is accessible by querying The Blue Flame orchestrator:

**Leadership KB:**
```
"Who in leadership would best understand a technical resource request?"
```

**Threat Intelligence KB:**
```
"What are the indicators of C2 beaconing activity?"
```

**Incident Response KB:**
```
"What are the steps for responding to a ransomware incident?"
```

**System Configuration KB:**
```
"What ports does the Gateway service use?"
```

## System Prompt Integration

The system prompt (`config/system_prompt.txt`) has been updated to document all available namespaces:

- `corporate_context` - Leadership & organizational knowledge
- `threat_intel` - Security indicators & threat intelligence
- `incident_response` - Incident tracking & remediation
- `system_config` - System configuration history
- `insights` - Transcript summaries & analysis (auto-generated)
- `default` - General-purpose queries

## Re-ingestion

To update any KB:

1. Edit the chunk content in the corresponding `ingest_*.rs` file
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
- Ensure you're querying the correct namespace
- Check memory service logs for query errors
