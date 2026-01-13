# Leadership Knowledge Base Ingestion

This script ingests the Ferrellgas Leadership Knowledge Base into the Qdrant Memory Service for use by The Blue Flame orchestrator.

## Overview

The leadership KB is structured into four semantic chunks stored in the `corporate_context` namespace:

1. **James E. Ferrell - Chairman**: Leadership tenure, achievements, and transformation of the company from a small-town shop to a leading propane supplier
2. **Tamria A. Zertuche - CEO and President**: IT background and career path (strategic unlock for technical escalations)
3. **Board of Directors and People-First Philosophy**: Organizational values and decision-making framework
4. **Family Values and Company History**: Company founding in 1939, family heritage, employee-ownership culture, and core values

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
cargo run --example ingest_leadership_kb
```

### With custom memory service URL:

```bash
MEMORY_GRPC_URL=http://127.0.0.1:50052 cargo run --example ingest_leadership_kb
```

## What It Does

1. Connects to the Memory Service gRPC endpoint
2. Creates/ensures the `corporate_context` namespace (collection) exists in Qdrant
3. Ingests three leadership KB chunks with metadata:
   - `source: leadership_kb`
   - `priority: high`
   - Role-specific metadata (e.g., `it_background: true` for CEO)

## Verification

After ingestion, you can verify the KB is accessible by querying The Blue Flame orchestrator:

**Example Query:**
```
"Who in leadership would best understand a technical resource request for more RAM?"
```

**Expected Response:**
The orchestrator should reference CEO Tamria A. Zertuche's IT background and suggest directing the request to her, as she has experience as Senior Director of IT and CIO.

## Integration with System Prompt

The system prompt (`config/system_prompt.txt`) has been updated to include:

- **Identity & Context**: Recognition of The Blue Flame's role within Ferrellgas
- **Operational Values**: People-First philosophy and Strategic IT alignment
- **Leadership Awareness**: Instructions to query `corporate_context` namespace for leadership knowledge

## Re-ingestion

If you need to update the leadership KB:

1. Edit the chunk content in `examples/ingest_leadership_kb.rs`
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
- Ensure you're querying the `corporate_context` namespace
- Check memory service logs for query errors
