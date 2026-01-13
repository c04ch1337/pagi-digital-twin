# Incident Response Knowledge Base Ingestion

This script ingests the Incident Response Knowledge Base into the Qdrant Memory Service for use by The Blue Flame orchestrator.

## Overview

The incident response KB is structured into playbook chunks stored in the `incident_response` namespace:

1. **Incident Response Lifecycle**: Six-phase framework (Preparation, Identification, Containment, Eradication, Recovery, Lessons Learned)
2. **Malware Incident Response Playbook**: Step-by-step procedures for malware detection and remediation
3. **Data Exfiltration Response**: Procedures for detecting and responding to data breaches
4. **Ransomware Response Procedures**: Critical steps for ransomware incident handling
5. **Phishing Incident Response**: Procedures for phishing email incidents
6. **Remediation and Recovery Procedures**: Post-incident recovery and system restoration

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
cargo run --example ingest_incident_response_kb
```

### With custom memory service URL:

```bash
MEMORY_GRPC_URL=http://127.0.0.1:50052 cargo run --example ingest_incident_response_kb
```

## What It Does

1. Connects to the Memory Service gRPC endpoint
2. Creates/ensures the `incident_response` namespace (collection) exists in Qdrant
3. Ingests 6 incident response playbook chunks with metadata:
   - `source: incident_response_kb`
   - `priority: high`
   - Incident type classification (malware, data_breach, ransomware, phishing, etc.)
   - Severity levels (critical, high, medium)

## Verification

After ingestion, you can verify the KB is accessible by querying The Blue Flame orchestrator:

**Example Query:**
```
"What are the steps for responding to a ransomware incident?"
```

**Expected Response:**
The orchestrator should reference the ransomware response playbook, including isolation procedures, backup verification, and recovery steps.

## Integration with System Prompt

The system prompt (`config/system_prompt.txt`) has been updated to include:

- **Namespace Documentation**: Instructions to use `incident_response` namespace for incident-related queries
- **Playbook Access**: Framework for retrieving incident response procedures during active incidents

## Re-ingestion

If you need to update the incident response KB:

1. Edit the chunk content in `examples/ingest_incident_response_kb.rs`
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
- Ensure you're querying the `incident_response` namespace
- Check memory service logs for query errors
