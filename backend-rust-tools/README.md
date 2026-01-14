d# backend-rust-tools

Rust-based gRPC Secure Execution Bridge for the PAGI Digital Twin platform.

## Overview

This service provides secure, policy-enforced tool execution via gRPC, serving as the Secure Execution Bridge for the Digital Twin architecture. It enforces authorization policies and executes commands in a sandboxed environment.

## Features

- **gRPC Interface**: Tonic-based gRPC server for efficient inter-service communication
- **Policy Enforcement**: Twin-based authorization with configurable command whitelists
- **Safe Mode**: Global safe mode flag to restrict dangerous operations
- **Sandbox Execution**: Real process execution with working-directory isolation inside `SANDBOX_DIR`
- **Execution History**: Tracks last 1000 executions for audit purposes

## Protocol

The service implements the `ToolExecutorService` gRPC interface:

- `RequestExecution`: Execute a tool/command with policy checks
- `HealthCheck`: Service health status

See `proto/tools.proto` for the complete protocol definition.

## Policy Configuration

The service enforces a policy matrix based on twin IDs:

- **twin-aegis** (Orchestrator): Can execute all commands (`*`)
- **twin-sentinel**: Can execute `file_write`, `command_exec`, `vector_query`
- **twin-trace**: Can execute `vector_query` (read-only)

Restricted commands (always blocked):
- `rm`, `delete`, `format`, `shutdown`, `reboot`

## Configuration

Environment variables:

- `TOOLS_GRPC_PORT`: gRPC server port (default: `50054`)
- `SANDBOX_DIR`: Sandbox directory for execution (default: `/tmp/pagi-sandbox`)
- `SAFE_MODE`: Enable safe mode to restrict all dangerous operations (default: `false`)
- `TOOLS_EXEC_TIMEOUT_MS`: Hard timeout for a single execution (default: `10000`)
- `TOOLS_USE_BWRAP`: Enable bubblewrap sandboxing on Linux (default: `false`)
- `LOG_LEVEL`: Logging level (default: `info`)

## Building

```bash
cargo build --release
```

## Running

```bash
cargo run
```

Or with environment variables:

```bash
TOOLS_GRPC_PORT=50054 SANDBOX_DIR=/tmp/pagi-sandbox SAFE_MODE=false cargo run
```

## Security

The current implementation provides **cwd-based sandboxing** by setting the child process working directory to a per-execution folder under `SANDBOX_DIR`.

This is **not** equivalent to a true jail (e.g., `chroot`, bubblewrap, nsjail). A malicious process can still access the wider filesystem using absolute paths.

### bubblewrap (bwrap) sandboxing

When `TOOLS_USE_BWRAP=true` (Linux only), the service wraps executions in `bwrap` to provide OS-level isolation (namespaces + mount sandboxing). See [`ToolExecutorServiceImpl::execute_command_sandboxed()`](backend-rust-tools/src/main.rs:188).

The executed form is:

```bash
bwrap --unshare-all --unshare-net --die-with-parent --bind <SANDBOX_DIR>/<twin_id>/ / --setenv PATH /usr/bin -- <COMMAND> <ARGS>
```

If `bwrap` is not installed or the platform is not Linux, the service automatically falls back to the legacy cwd isolation.

#### Installing bubblewrap (local testing)

- Ubuntu/Debian:
  ```bash
  sudo apt-get update && sudo apt-get install -y bubblewrap
  ```
- Fedora:
  ```bash
  sudo dnf install -y bubblewrap
  ```
- Arch:
  ```bash
  sudo pacman -S bubblewrap
  ```

Future versions should implement:

- **Process isolation**: Using gVisor or Firecracker for secure execution
- **Resource limits**: CPU, memory, and timeout constraints
- **File system sandboxing**: Isolated file system access
- **Network restrictions**: Limited network access for executed commands

## Development

The service is designed to be a drop-in replacement for the Python tool execution service, providing better performance, security, and type safety through Rust's type system.
