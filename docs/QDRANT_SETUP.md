# Qdrant Setup (Bare-metal / Production)

The Memory Service ([`backend-rust-memory`](../backend-rust-memory/Cargo.toml:1)) requires a **live Qdrant** connection on startup.

- It will **exit/panic** if it cannot connect to `QDRANT_URL`.
- It runs a background heartbeat (every ~15s) and logs a **CRITICAL error** if Qdrant connectivity is lost.

---

## 1) Install Qdrant (bare-metal)

### Option A — Download the Qdrant executable (recommended)

1. Download the Qdrant binary for your OS from the official Qdrant releases.
2. Extract it and run:

```bash
./qdrant
```

By default Qdrant exposes:

- `6333` (HTTP REST / dashboard)
- `6334` (gRPC)

The Rust Memory Service uses **gRPC**, so you will point `QDRANT_URL` at port `6334`.

### Option B — Run Qdrant as a system service (Linux)

Run Qdrant under a supervisor (e.g., `systemd`) so it auto-restarts and persists storage.

---

## 2) Configure this repo (`.env`)

Set the gRPC endpoint:

```env
QDRANT_URL=http://127.0.0.1:6334
```

If your provider requires an API key:

```env
QDRANT_API_KEY=YOUR_SECRET_KEY
```

---

## 3) Run the Memory Service

```bash
cd backend-rust-memory
cargo run
```

---

## Notes

- Collections are created automatically per `namespace` with vector size `EMBEDDING_MODEL_DIM`.
- For a quick local test, Docker is also fine:

```bash
docker run --name qdrant-local -p 6333:6333 -p 6334:6334 -v qdrant_storage:/qdrant/storage qdrant/qdrant:latest
```

