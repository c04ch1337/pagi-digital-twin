# Build Service (backend-rust-build)

This service exists to isolate **high-privilege operations** (code writing + compilation) away from the Orchestrator.

## Call chain / lifecycle

1. **Plan** (Orchestrator): decide the tool name + Rust code.
2. **Build** (Build Service):
   - write `TOOLS_REPO_DIR/<tool_name>/src/main.rs`
   - compile with `cargo build` in `TOOLS_REPO_DIR/<tool_name>`
   - return `stdout`, `stderr`, `exit_code`
3. **Register/Execute** (Tools Service): once compilation succeeds, the Orchestrator should register the tool artifact and/or execute it via the tools subsystem.

## Hardening

The build service implements a basic queue to avoid compilation-flooding:

- `BUILD_MAX_CONCURRENT`: maximum concurrent `cargo build` processes (default: 1)
- `BUILD_MAX_PENDING`: maximum in-flight requests (waiting + running) before returning `RESOURCE_EXHAUSTED`

This provides backpressure and protects the host from CPU/RAM exhaustion.

## Notes / assumptions

- `tool_name` is restricted to `[A-Za-z0-9_-]` to prevent path traversal and keep crate names sane.
- `tool_code` is written directly to `src/main.rs`. If it does **not** contain `fn main`, it is wrapped into `fn main() { ... }` as a best-effort convenience.
- The service does not currently accept dependencies; generated `Cargo.toml` contains no `[dependencies]`. Tool code must compile with `std` only.

