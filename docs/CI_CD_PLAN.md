# CI/CD Integration Plan (Bare-Metal First)

This document describes a **high-level CI/CD pipeline** for building, testing, and deploying:

* **Frontend**: `frontend-digital-twin` (Vite)
* **Five Rust backend services**:
  * `backend-rust-memory`
  * `backend-rust-tools`
  * `backend-rust-orchestrator`
  * `backend-rust-gateway`
  * `backend-rust-telemetry`

The plan assumes **bare-metal Linux deployment** (systemd-managed processes). Docker is **optional** and used only for dependencies (e.g., Qdrant) during CI and/or on the target server.

---

## 1) Principles / Constraints

### Bare Metal First
* Production services run as **native Linux binaries** under **systemd**.
* Docker is not required to run the Rust services or frontend.
* Docker **may** be used for:
  * Qdrant (`qdrant-db`) and other infra dependencies
  * ephemeral CI environments

### Build Once, Deploy Many
* CI builds **versioned artifacts** (Rust binaries + frontend static bundle).
* Deploy stage transfers artifacts to the server without rebuilding.

### Repeatable + Fast
* Use caching for:
  * Cargo registry + target
  * Node `~/.npm` (or equivalent)
* Prefer parallel builds/tests when possible, but keep dependency checks and E2E ordering strict.

---

## 2) Pipeline Overview (Stages)

1. **Preflight / Setup**
2. **Build**
3. **Test**
   * Unit/Integration tests
   * Dependency checks (Qdrant + bubblewrap)
   * End-to-End (E2E) validation
4. **Package / Artifact publish**
5. **Deploy (bare metal)**
6. **Post-deploy verification + rollback hooks**

---

## 3) Preflight / Setup Stage

### Runner requirements (CI)
* Linux runner (preferred; matches target environment)
* Toolchains
  * Rust (per repo policy; the E2E script suggests Rust 1.78+)
  * Node.js 20+
* Optional but recommended tools
  * `grpc_health_probe` (for gRPC readiness checks)
  * `curl`
  * `jq` (for structured API validation)

### Secrets / Environment
* Load environment variables from CI secrets where applicable (API keys, deployment SSH key, etc.).
* Keep `.env` out of git; CI should provide production/staging environment values.

---

## 4) Build Stage

### 4.1 Frontend build (`frontend-digital-twin`)

Order:
1. `npm install`
   * CI can use `npm ci` instead if you want lockfile-strict installs.
2. `npm run build`

Outputs:
* Typical Vite output: `frontend-digital-twin/dist/`

Notes:
* Configure Vite env vars at build time if required (e.g., `VITE_WS_URL`, `VITE_SSE_URL`).

### 4.2 Rust builds (five services)

Build strategy options:

**Option A (simple, explicit per-service builds):**
Run `cargo build --release` in each service directory:
* `backend-rust-memory`
* `backend-rust-tools`
* `backend-rust-orchestrator`
* `backend-rust-gateway`
* `backend-rust-telemetry`

**Option B (parallelized in CI):**
Use CI matrix/parallel jobs, one per service, each running `cargo build --release`.

Outputs:
* `target/release/<binary>` per service

Notes:
* If any service requires generated code (protobuf build scripts), ensure the runner has required build deps.
* Prefer `--locked` in CI to guarantee deterministic dependency resolution.

---

## 5) Test Stage

### 5.1 Unit + Integration tests (Rust)

Run tests for each Rust service:
* `cargo test` (per service directory)

Recommended improvements over time:
* Split fast unit tests vs slower integration tests via Cargo features or test naming.
* Add service-level smoke tests (e.g., start server, hit `/health`, run `grpc_health_probe`).

### 5.2 Dependency check (runner prerequisites for E2E)

The E2E stage assumes certain dependencies are available.

#### Qdrant availability (optional Docker dependency)
If using Docker in CI for dependencies:
1. Start Qdrant only (faster than bringing everything up):
   * `docker compose up -d qdrant-db`
2. Wait for readiness (examples):
   * REST: `curl -f http://127.0.0.1:6333/` (or a known health endpoint)
   * gRPC: use a probe against `127.0.0.1:6334`

If not using Docker:
* Ensure the runner has a reachable Qdrant endpoint and set `QDRANT_URL` accordingly.

#### bubblewrap availability
Tools execution security typically expects `bubblewrap` (`bwrap`) to be installed.
* Check with: `command -v bwrap`
* If missing:
  * Fail the E2E stage (recommended), or
  * Skip the subset of E2E that requires sandboxing (not recommended for production parity).

### 5.3 End-to-End (E2E) validation

Execute the repository E2E validation guide:
* `tests/e2e_test_script.md`

CI-friendly E2E approach:
1. Start dependencies (e.g., Qdrant via Docker) if required.
2. Start all five Rust services in dependency order:
   1. Memory (gRPC)
   2. Tools (gRPC)
   3. Orchestrator (HTTP)
   4. Telemetry (HTTP/SSE)
   5. Gateway (WS + HTTP proxy)
3. Start the frontend as a static build served by a lightweight server OR use `npm run dev` only in non-CI contexts.
4. Validate:
   * Cross-service gRPC calls (Orchestrator → Memory/Tools)
   * WebSocket chat path (Frontend → Gateway)
   * SSE telemetry path (Frontend → Gateway → Telemetry)

Recommended automation:
* Convert the manual steps in `tests/e2e_test_script.md` into a non-interactive script (shell/python) that:
  * boots services in background
  * polls health endpoints
  * runs WebSocket + SSE checks
  * exits non-zero on failure

---

## 6) Packaging / Artifact Stage

Create a single deployable bundle (tarball/zip) containing:
* Rust release binaries for the five services
* Frontend `dist/` static files
* Deployment metadata
  * git SHA
  * build time
  * config template (no secrets)

Suggested artifact layout:

```text
artifact/
  bin/
    backend-rust-memory
    backend-rust-tools
    backend-rust-orchestrator
    backend-rust-gateway
    backend-rust-telemetry
  frontend/
    dist/...
  deploy/
    systemd/
    env/
    VERSION
```

Publish artifacts:
* CI artifact store (for later deploy jobs)
* Optional: attach to release tags

---

## 7) Deployment Stage (Bare-Metal Linux)

### 7.1 Target server prerequisites

On the bare-metal server:
* Create a service user (e.g., `pagi`) with limited permissions.
* Create directories:
  * `/opt/pagi/bin`
  * `/opt/pagi/frontend`
  * `/etc/pagi/` (env/config)
* systemd installed and enabled.
* Optional Docker + Compose installed if Qdrant will be containerized.

### 7.2 Transfer artifacts

From CI to server (typical approach):
* `scp` or `rsync` the artifact bundle
* Unpack to a versioned release directory, e.g. `/opt/pagi/releases/<sha>/`
* Update a `current` symlink:
  * `/opt/pagi/current -> /opt/pagi/releases/<sha>/`

### 7.3 Install/update systemd units

Use systemd units for each service (one unit per process). Typical pattern:
* `backend-rust-memory.service`
* `backend-rust-tools.service`
* `backend-rust-orchestrator.service`
* `backend-rust-telemetry.service`
* `backend-rust-gateway.service`

For the frontend on bare metal:
* Serve `dist/` with Nginx/Caddy OR a node static server managed by systemd.

### 7.4 Restart strategy (dependency order)

Restart order (low-level dependencies first):
1. (optional) Qdrant
2. Memory
3. Tools
4. Orchestrator
5. Telemetry
6. Gateway
7. Frontend

Implement via systemd:
* `systemctl daemon-reload`
* `systemctl restart <unit>`
* `systemctl status <unit> --no-pager`

### 7.5 Post-deploy verification

Run health checks:
* Gateway: `/api/health`
* Orchestrator: `/health`
* Telemetry stream endpoint (SSE) proxied via gateway
* gRPC health probes for memory/tools

If verification fails:
* Roll back `current` symlink to previous release
* Restart services

---

## 8) Example CI Job Graph (Conceptual)

```text
           +------------------+
           |  preflight/setup |
           +--------+---------+
                    |
          +---------+----------+
          |                    |
  +-------v--------+   +-------v--------+
  | build_frontend |   | build_rust(*)  |
  +-------+--------+   +-------+--------+
          |                    |
          +---------+----------+
                    |
           +--------v---------+
           |  test_rust(*)     |
           +--------+---------+
                    |
           +--------v---------+
           | deps_check (E2E) |
           +--------+---------+
                    |
           +--------v---------+
           | e2e_validation   |
           +--------+---------+
                    |
           +--------v---------+
           | package/artifact |
           +--------+---------+
                    |
           +--------v---------+
           | deploy (ssh)     |
           +------------------+

  (*) can be parallelized by service
```

---

## 9) Minimal Commands Reference (non-authoritative)

Frontend:

```bash
cd frontend-digital-twin
npm ci
npm run build
```

Rust services (repeat per service directory):

```bash
cargo build --release --locked
cargo test --locked
```

Dependency (Qdrant):

```bash
docker compose up -d qdrant-db
```

---

## 10) Future Enhancements

* Convert manual E2E steps into an automated runner script (non-interactive).
* Add binary signing and checksum publishing.
* Add staged deployments (staging → prod) with approvals.
* Add canary deploy + gradual rollout.
* Add structured log/metrics assertions as part of post-deploy verification.

