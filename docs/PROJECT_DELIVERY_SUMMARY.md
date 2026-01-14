# Project Delivery Summary — PAGI Digital Twin / AGI Platform

This document consolidates the final, high-level architecture and the most critical non-trivial implementation choices for research and operations.

## 1) Architecture Summary: Tri-Layer Phoenix Architecture

The delivered system implements a "Tri-Layer Phoenix Architecture" with an explicit observability plane:

1. **Gateway** (edge protocol + UI integration)
2. **Orchestrator** (planning + policy mediation)
3. **Infrastructure** (memory + tool execution + data stores)
4. **Observability** (telemetry + tracing/metrics plumbing)

### 1.1 Gateway Layer

**Primary responsibilities**

- WebSocket ingress for the UI.
- HTTP proxy to the Orchestrator.
- Server-Sent Events (SSE) proxy for telemetry.

**Key implementation anchors**

- WebSocket entrypoint: [`ws_handler()`](backend-rust-gateway/src/main.rs:88)
- Orchestrator proxy: [`proxy_to_orchestrator()`](backend-rust-gateway/src/main.rs:210)
- Telemetry SSE proxy: [`telemetry_proxy_handler()`](backend-rust-gateway/src/main.rs:253)
- Docker Compose wiring: [`rust-gateway`](docker-compose.yml:214)

### 1.2 Orchestrator Layer

**Primary responsibilities**

- Single decision point for “what happens next” (respond vs. memory vs. tool).
- Structured planning via LLM provider (OpenRouter) with deterministic fallback.
- Human-in-the-loop (HITL) gating for memory queries and tool execution.

**Key implementation anchors**

- Orchestrator request handler: [`handle_chat_request()`](backend-rust-orchestrator/src/main.rs:292)
- Action contract (typed JSON): [`LLMAction`](backend-rust-orchestrator/src/main.rs:116)
- Docker Compose wiring: [`rust-orchestrator`](docker-compose.yml:191)

### 1.3 Infrastructure Layer

**Primary responsibilities**

- Durable “Neural Archive” vector storage (Qdrant) behind a gRPC memory API.
- Secure execution of tools/commands behind a gRPC tools API.
- Containerized dependencies and persistent volumes.

**Key implementation anchors**

- Memory service (Qdrant-backed): [`MemoryServiceImpl`](backend-rust-memory/src/main.rs:33) and [`rust-memory-service`](docker-compose.yml:152)
- Vector DB dependency (Qdrant): [`qdrant-db`](docker-compose.yml:37)
- Tools execution service: [`ToolExecutorServiceImpl`](backend-rust-tools/src/main.rs:109) and [`rust-tools-service`](docker-compose.yml:172)
- Tools sandbox storage: [`tools_sandbox`](docker-compose.yml:274)

### 1.4 Observability Plane

**Primary responsibilities**

- Real-time host telemetry via SSE.
- Pluggable trace/metrics infrastructure (Jaeger + Prometheus) for production hardening.

**Key implementation anchors**

- Telemetry SSE producer: [`sse_stream()`](backend-rust-telemetry/src/main.rs:34) and [`telemetry`](docker-compose.yml:234)
- Optional tracing/metrics stack: [`jaeger`](docker-compose.yml:8) and [`prometheus`](docker-compose.yml:18)

## 2) Key Implementation Choices (Critical / Non-Trivial)

### 2.1 AGI Core — OpenRouter Structured Planning + Human-in-the-Loop (P34)

**What was implemented**

1. **Structured planning with OpenRouter**: the Orchestrator uses OpenRouter to produce a single **typed JSON decision** that can be parsed into a Rust enum.
   - Planner entrypoint: [`llm_plan_openrouter()`](backend-rust-orchestrator/src/main.rs:191)
   - Typed contract: [`LLMAction`](backend-rust-orchestrator/src/main.rs:116)
   - The planner requests strict JSON output via `response_format: { type: "json_object" }` inside [`llm_plan_openrouter()`](backend-rust-orchestrator/src/main.rs:191)

2. **Deterministic fallback for E2E and offline work**: when OpenRouter is unavailable or disabled, the Orchestrator falls back to a deterministic mock planner.
   - Mock planner: [`llm_plan_mock()`](backend-rust-orchestrator/src/main.rs:135)
   - Selection + fallback logic: [`handle_chat_request()`](backend-rust-orchestrator/src/main.rs:538)

3. **Human-in-the-loop gating (HITL)**: tool and memory actions are *not executed immediately*.
   - Pending state stores:
     - [`AppState::pending_tools`](backend-rust-orchestrator/src/main.rs:87)
     - [`AppState::pending_memories`](backend-rust-orchestrator/src/main.rs:89)
   - HITL flow is mediated in [`handle_chat_request()`](backend-rust-orchestrator/src/main.rs:292):
     - Tool approval messages: `"[TOOL_EXECUTED]"` / `"[TOOL_DENIED]"` parsing in [`handle_chat_request()`](backend-rust-orchestrator/src/main.rs:307)
     - Memory approval messages: `"[MEMORY_SHOWN]"` / `"[MEMORY_DENIED]"` parsing in [`handle_chat_request()`](backend-rust-orchestrator/src/main.rs:419)
   - The Orchestrator returns an explicit UI command payload for approval:
     - Tool request payload (UI modal trigger): [`issued_command`](backend-rust-orchestrator/src/main.rs:622)
     - Memory request payload (UI modal trigger): [`issued_command`](backend-rust-orchestrator/src/main.rs:587)

**Why it matters (ops + research impact)**

- **Safety + controllability**: HITL provides a hard “authorization boundary” between intent/planning and execution.
- **Auditability**: the system preserves the *raw* planner JSON in responses for transparency (see [`ChatResponse::raw_orchestrator_decision`](backend-rust-orchestrator/src/main.rs:50)).
- **Interoperability**: OpenRouter provides a single API surface for swapping models without rewriting the Orchestrator.

### 2.2 Neural Archive — Mandatory Persistent Qdrant Integration (P32)

**What was implemented**

1. **Qdrant is mandatory (fail-fast)**: the Rust Memory Service requires a live Qdrant connection on startup.
   - Mandatory dependency documented in struct field comment: [`MemoryServiceImpl`](backend-rust-memory/src/main.rs:33)
   - Startup connection + retry loop: [`MemoryServiceImpl::new()`](backend-rust-memory/src/main.rs:40)
   - Hard failure if Qdrant cannot be reached: [`main()`](backend-rust-memory/src/main.rs:532)

2. **Operational heartbeat**: the service continuously checks Qdrant connectivity and emits a CRITICAL log when connectivity is lost.
   - Heartbeat implementation: [`spawn_qdrant_heartbeat()`](backend-rust-memory/src/main.rs:117)

3. **Namespace-based collections**: collections are created lazily per namespace to support tenant/workstream isolation.
   - Collection ensure/create: [`ensure_collection()`](backend-rust-memory/src/main.rs:159)

4. **Persistence validation hooks**: the repo includes a mini-test for verifying upsert + query behavior.
   - Mini-test doc: [`tests/qdrant_test.md`](tests/qdrant_test.md:1)
   - Ops setup guide: [`docs/QDRANT_SETUP.md`](docs/QDRANT_SETUP.md:1)

**Known limitation (intentional, next step)**

- Embeddings are currently mocked for wiring determinism (see [`generate_mock_embedding()`](backend-rust-memory/src/main.rs:150)). Production deployment should replace this with a real embedding provider.

### 2.3 Secure Execution — bubblewrap OS-Level Sandboxing in Tools Service (P33)

**What was implemented**

1. **Default safe behavior + policy enforcement**: tool execution is gated by a twin-scoped allowlist and restricted command list.
   - Policy model: [`PolicyConfig`](backend-rust-tools/src/main.rs:21)
   - Authorization decision: [`PolicyConfig::is_authorized()`](backend-rust-tools/src/main.rs:66)

2. **Two-tier sandboxing model**

- **Baseline**: cross-platform *cwd isolation* (per-execution directory under a sandbox root).
  - Core implementation: [`ToolExecutorServiceImpl::execute_command_sandboxed()`](backend-rust-tools/src/main.rs:188)
  - Security caveat documented inline: [`ToolExecutorServiceImpl::execute_command_sandboxed()`](backend-rust-tools/src/main.rs:182)

- **Hardened path (Linux)**: bubblewrap (“bwrap”) provides OS-level isolation using namespaces and a mount sandbox.
  - Feature flag: `TOOLS_USE_BWRAP` (documented in [`backend-rust-tools/README.md`](backend-rust-tools/README.md:41))
  - Activation logic + fallback: [`ToolExecutorServiceImpl::execute_command_sandboxed()`](backend-rust-tools/src/main.rs:234)
  - `bwrap` invocation details (including `--unshare-net`): [`ToolExecutorServiceImpl::execute_command_sandboxed()`](backend-rust-tools/src/main.rs:257)
  - Bubblewrap availability check: [`ToolExecutorServiceImpl::is_bwrap_available()`](backend-rust-tools/src/main.rs:383)
  - High-level docs: bubblewrap section in [`backend-rust-tools/README.md`](backend-rust-tools/README.md:72)

3. **Runaway control**: hard timeout is enforced to prevent indefinite executions.
   - Timeout logic: [`ToolExecutorServiceImpl::execute_command_sandboxed()`](backend-rust-tools/src/main.rs:324)

**Why it matters (ops + research impact)**

- The Tools Service is where “model intent meets the OS.” bubblewrap provides a strong security boundary when running on Linux.
- The implementation is explicitly designed to degrade safely (falls back to cwd isolation when bubblewrap is unavailable).

## 3) Recommended Next Steps (Production Deployment + Maintenance)

### 3.1 Production Readiness / Security Hardening

- **Enforce authn/authz at the edge**
  - Add authentication to Gateway WS/HTTP endpoints (start at [`ws_handler()`](backend-rust-gateway/src/main.rs:88)).
  - Require API keys or JWTs for Orchestrator endpoints (start at [`handle_chat_request()`](backend-rust-orchestrator/src/main.rs:292)).

- **Secrets management**
  - Move OpenRouter keys out of the local environment file into a secret store; rotate keys if ever exposed (see template in [`.env.example`](.env.example:1) and compose wiring in [`docker-compose.yml`](docker-compose.yml:191)).

- **Network segmentation**
  - Run Tools and Memory on private networks; expose only Gateway externally (see networks in [`docker-compose.yml`](docker-compose.yml:276)).

- **Secure execution on Linux**
  - Standardize production deployments on Linux nodes and enable `TOOLS_USE_BWRAP=true`.
  - Consider a stronger isolation backend long-term (gVisor/Firecracker) using the current interface as the seam (start at [`ToolExecutorServiceImpl::execute_command_sandboxed()`](backend-rust-tools/src/main.rs:188)).

### 3.2 Data / Memory System Maturation

- **Replace mock embeddings** with a real embedding service while keeping the Qdrant-backed persistence model.
  - Current placeholder: [`generate_mock_embedding()`](backend-rust-memory/src/main.rs:150)

- **Backups + retention policies**
  - Back up the Qdrant storage volume and define retention per namespace (Qdrant volume: [`qdrant_data`](docker-compose.yml:272)).

### 3.3 Observability & Operations

- **Telemetry**
  - Expand telemetry payloads and/or add per-service metrics; current SSE producer is [`sse_stream()`](backend-rust-telemetry/src/main.rs:34).

- **Tracing & metrics**
  - If using Jaeger/Prometheus in production, formalize alert rules and dashboards (compose services: [`jaeger`](docker-compose.yml:8), [`prometheus`](docker-compose.yml:18)).

### 3.4 Release Engineering

- Establish a CI pipeline that:
  - Builds all services.
  - Runs Qdrant integration checks (start from [`tests/qdrant_test.md`](tests/qdrant_test.md:1)).
  - Runs security linting and dependency scans.

