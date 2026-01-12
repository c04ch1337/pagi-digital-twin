# backend-rust-build

High-privilege build service responsible for **writing** and **compiling** new tool crates.

## gRPC API

Proto: [`backend-rust-build/proto/build.proto`](backend-rust-build/proto/build.proto)

### `CreateTool`

Input:

- `tool_name`: folder/crate name under `TOOLS_REPO_DIR` (default: `tools_repo/`)
- `tool_code`: Rust source code string written to `src/main.rs`

Output:

- `stdout`, `stderr`, `exit_code` from `cargo build`

## Configuration

- `BUILD_SERVICE_PORT` (default `50055`)
- `TOOLS_REPO_DIR` (default `tools_repo`)
- `BUILD_TIMEOUT_MS` (default `120000`)
- `BUILD_MAX_CONCURRENT` (default `1`)
- `BUILD_MAX_PENDING` (default `4`)

## Orchestrator integration plan

The Orchestrator must chain calls:

`Plan -> Build Service (CreateTool/Compile) -> Tools Service (Register/Execute)`

See full details in [`docs/build_service.md`](docs/build_service.md).

