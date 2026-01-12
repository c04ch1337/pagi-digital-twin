# Qdrant Integration Mini-Test (Rust Memory Service)

This mini-test verifies that [`backend-rust-memory/src/main.rs`](../backend-rust-memory/src/main.rs) is persisting vectors into Qdrant and that queries return stored points.

## 1) Start Qdrant

### Option A: Docker Compose (recommended)

```bash
docker compose up -d qdrant-db
```

Confirm Qdrant is healthy via REST:

```bash
curl http://localhost:6333/healthz
```

Expected: `ok`.

### Option B: Existing local Qdrant

If you run Qdrant locally, ensure its gRPC endpoint is available at the URL you configure (default: `http://127.0.0.1:6334`).

## 2) Start the Rust Memory Service

### Option A: Docker Compose

```bash
docker compose up -d rust-memory-service
```

### Option B: Bare-metal

From the repo root:

```bash
set QDRANT_URL=http://127.0.0.1:6334
set EMBEDDING_MODEL_DIM=384
set MEMORY_GRPC_PORT=50052

cargo run --manifest-path backend-rust-memory/Cargo.toml
```

## 3) Verify gRPC calls with grpcurl

Install `grpcurl` (native), or use Docker.

### 3.1 HealthCheck

```bash
grpcurl -plaintext -proto backend-rust-memory/proto/memory.proto \
  localhost:50052 memory.MemoryService/HealthCheck
```

Expected: a response with `status: "healthy"`.

### 3.2 CommitMemory (upsert into Qdrant)

```bash
grpcurl -plaintext -proto backend-rust-memory/proto/memory.proto \
  -d "{\"content\":\"hello qdrant\",\"namespace\":\"qdrant_test\",\"twin_id\":\"twin-1\",\"memory_type\":\"Episodic\",\"risk_level\":\"Low\",\"metadata\":{\"source\":\"qdrant_test.md\"}}" \
  localhost:50052 memory.MemoryService/CommitMemory
```

Expected: `success: true` and a non-empty `memory_id`.

### 3.3 QueryMemory (search in Qdrant)

```bash
grpcurl -plaintext -proto backend-rust-memory/proto/memory.proto \
  -d "{\"query\":\"hello\",\"namespace\":\"qdrant_test\",\"twin_id\":\"twin-1\",\"top_k\":5}" \
  localhost:50052 memory.MemoryService/QueryMemory
```

Expected: `total_count >= 1` and at least one `results` entry containing the committed payload fields.

## 4) If you don’t have grpcurl (Docker-based grpcurl)

If the memory service is listening on your host at `localhost:50052`:

```bash
docker run --rm fullstorydev/grpcurl:latest \
  -plaintext -proto /work/backend-rust-memory/proto/memory.proto \
  -import-path /work \
  -d "{\"query\":\"hello\",\"namespace\":\"qdrant_test\",\"top_k\":5}" \
  host.docker.internal:50052 memory.MemoryService/QueryMemory \
  -max-time 5 \
  -vv \
  -H "content-type: application/grpc"
```

Mounting the repo is required (so the container can read the `.proto`):

```bash
docker run --rm -v %cd%:/work fullstorydev/grpcurl:latest --help
```

