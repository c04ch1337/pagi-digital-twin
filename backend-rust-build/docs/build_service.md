# Build Service (backend-rust-build)

This service is the **high-privilege** component responsible for **writing** and **compiling** new tool crates.

It exists specifically to keep code-writing/compilation out of the Orchestrator process.

## Responsibilities

- Accept raw Rust source from the Orchestrator.
- Write it into a tool crate under `TOOLS_REPO_DIR/<tool_name>/src/main.rs`.
- Run `cargo build` in that tool crate directory.
- Return compiler/build output (`stdout`, `stderr`) and `exit_code`.

## Orchestrator integration plan

The Orchestrator should use a chained flow:

1. **Plan** (LLM / policy)
2. **Build Service** (compile): `CreateTool(tool_name, tool_code)`
3. **Tools Service** (register/execute): register the resulting tool (or point the tools runtime at the compiled crate/binary) and execute

Rationale:

- The Orchestrator should *not* directly write files or run `cargo`.
- Build output is returned to the Orchestrator for logging/UX and for deciding whether to retry/fix.

## API (gRPC)

Proto: [`backend-rust-build/proto/build.proto`](backend-rust-build/proto/build.proto)

### `CreateTool`

Request:

- `tool_name`: crate/directory name (validated to `[A-Za-z0-9_-]`)
- `tool_code`: Rust source string written to `src/main.rs`

Response:

- `success`: `exit_code == 0`
- `exit_code`: cargo exit status (or `124` for timeout)
- `stdout`, `stderr`: captured process output
- `tool_dir`: path to the created tool directory

### `HealthCheck`

Simple liveness probe.

## Hardening / flood protection

The service implements a basic queue + concurrency limiter:

- `BUILD_MAX_CONCURRENT`: max concurrent compilations (default `1`)
- `BUILD_MAX_PENDING`: max in-flight requests (default `4`), beyond which requests receive `RESOURCE_EXHAUSTED`

This prevents the Orchestrator from flooding the host with `cargo build` processes.

## Configuration

- `BUILD_SERVICE_PORT` (default `50055`)
- `TOOLS_REPO_DIR` (default `tools_repo`)
- `BUILD_TIMEOUT_MS` (default `120000`)

