use std::env;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::process::{Command as StdCommand, Stdio};
use tokio::process::Command;
use tokio::sync::RwLock;
use tonic::{transport::Server, Request, Response, Status};
use tracing::{error, info, warn};
use uuid::Uuid;

// Include the generated proto code
pub mod proto {
    tonic::include_proto!("tools");
}

use proto::tool_executor_service_server::{ToolExecutorService, ToolExecutorServiceServer};
use proto::{ExecutionRequest, ExecutionResponse, HealthCheckResponse};

// Policy configuration for twin authorization
#[derive(Debug, Clone)]
struct PolicyConfig {
    // Twin ID -> allowed commands mapping
    allowed_commands: HashMap<String, Vec<String>>,
    // Commands that require special authorization
    restricted_commands: Vec<String>,
    // Safe mode flag (restricts all dangerous operations)
    safe_mode: bool,
}

impl PolicyConfig {
    fn new() -> Self {
        let mut allowed_commands = HashMap::new();
        
        // The Blue Flame (orchestrator) - can run anything
        allowed_commands.insert("twin-aegis".to_string(), vec!["*".to_string()]);
        
        // Sentinel Script - can run file operations and analysis tools
        allowed_commands.insert(
            "twin-sentinel".to_string(),
            vec!["file_write".to_string(), "command_exec".to_string(), "vector_query".to_string()],
        );
        
        // Trace Insight - read-only operations
        allowed_commands.insert(
            "twin-trace".to_string(),
            vec!["vector_query".to_string()],
        );

        Self {
            allowed_commands,
            restricted_commands: vec![
                "rm".to_string(),
                "delete".to_string(),
                "format".to_string(),
                "shutdown".to_string(),
                "reboot".to_string(),
            ],
            safe_mode: env::var("SAFE_MODE")
                .unwrap_or_else(|_| "false".to_string())
                .parse::<bool>()
                .unwrap_or(false),
        }
    }

    /// Check if a twin is authorized to execute a command
    fn is_authorized(&self, twin_id: &str, command: &str) -> (bool, String) {
        // Check safe mode
        if self.safe_mode && self.restricted_commands.contains(&command.to_string()) {
            return (false, format!("Command '{}' is restricted in safe mode", command));
        }

        // Check if command is in restricted list
        if self.restricted_commands.contains(&command.to_string()) {
            return (false, format!("Command '{}' is restricted", command));
        }

        // Get allowed commands for this twin
        if let Some(allowed) = self.allowed_commands.get(twin_id) {
            // Check if wildcard is allowed
            if allowed.contains(&"*".to_string()) {
                return (true, String::new());
            }
            
            // Check if specific command is allowed
            if allowed.contains(&command.to_string()) {
                return (true, String::new());
            }
        }

        (false, format!("Twin '{}' is not authorized to execute '{}'", twin_id, command))
    }
}

// Execution context for tracking executions
#[derive(Debug, Clone)]
struct ExecutionContext {
    execution_id: Uuid,
    command: String,
    args: Vec<String>,
    twin_id: String,
    job_id: String,
    sandbox_dir: PathBuf,
    start_time: chrono::DateTime<chrono::Utc>,
}

// Tool Executor Service Implementation
#[derive(Debug)]
pub struct ToolExecutorServiceImpl {
    policy: Arc<PolicyConfig>,
    sandbox_dir: PathBuf,
    execution_history: Arc<RwLock<Vec<ExecutionContext>>>,
}

impl ToolExecutorServiceImpl {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // Get sandbox directory from environment
        let sandbox_dir = env::var("SANDBOX_DIR")
            .unwrap_or_else(|_| "/tmp/pagi-sandbox".to_string())
            .into();

        // Create sandbox directory if it doesn't exist
        std::fs::create_dir_all(&sandbox_dir)?;

        Ok(Self {
            policy: Arc::new(PolicyConfig::new()),
            sandbox_dir,
            execution_history: Arc::new(RwLock::new(Vec::new())),
        })
    }

    fn split_lines_limited(s: &str, max_lines: usize, max_total_bytes: usize) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let mut total: usize = 0;

        for line in s.lines() {
            if out.len() >= max_lines {
                break;
            }

            let line_bytes = line.as_bytes().len();
            if total + line_bytes > max_total_bytes {
                break;
            }

            out.push(line.to_string());
            total += line_bytes;
        }

        out
    }

    fn map_spawn_error_to_response(
        execution_id: Uuid,
        err: std::io::Error,
        command_display: &str,
    ) -> ExecutionResponse {
        use std::io::ErrorKind;

        let (exit_code, message) = match err.kind() {
            ErrorKind::NotFound => (
                127,
                format!("Command not found: {} ({})", command_display, err),
            ),
            ErrorKind::PermissionDenied => (
                126,
                format!("Permission denied executing: {} ({})", command_display, err),
            ),
            _ => (1, format!("Failed to execute: {} ({})", command_display, err)),
        };

        ExecutionResponse {
            success: false,
            exit_code,
            stdout_logs: vec![],
            stderr_logs: vec![message.clone()],
            message,
            execution_id: execution_id.to_string(),
        }
    }

    /// Execute a command inside the sandbox directory.
    ///
    /// Security note: this is *cwd isolation* (not a true `chroot`). It prevents accidental
    /// writes outside the sandbox by well-behaved tools, but does not stop a malicious
    /// binary from accessing the wider filesystem. True isolation should be implemented
    /// with a dedicated sandboxing mechanism (e.g., bubblewrap/nsjail/gVisor).
    async fn execute_command_sandboxed(&self, ctx: &ExecutionContext) -> ExecutionResponse {
        let twin_root_dir = ctx.sandbox_dir.join(&ctx.twin_id);
        let execution_dir = twin_root_dir.join(ctx.execution_id.to_string());

        if let Err(e) = std::fs::create_dir_all(&execution_dir) {
            return ExecutionResponse {
                success: false,
                exit_code: 1,
                stdout_logs: vec![],
                stderr_logs: vec![format!(
                    "Failed to create sandbox execution directory {}: {}",
                    execution_dir.display(),
                    e
                )],
                message: "sandbox_init_failed".to_string(),
                execution_id: ctx.execution_id.to_string(),
            };
        }

        // Compatibility bridge:
        // - If the orchestrator uses a logical tool name `command_exec`, treat args[0]
        //   as a shell command string.
        // - Otherwise, treat `command` as the executable path/name and `args` as argv.
        let (program, argv, command_display): (String, Vec<String>, String) = if ctx.command == "command_exec" {
            let cmdline = ctx.args.get(0).cloned().unwrap_or_default();
            if cfg!(windows) {
                (
                    "cmd".to_string(),
                    vec!["/C".to_string(), cmdline.clone()],
                    format!("cmd /C {}", cmdline),
                )
            } else {
                (
                    "/bin/sh".to_string(),
                    vec!["-c".to_string(), cmdline.clone()],
                    format!("/bin/sh -c {}", cmdline),
                )
            }
        } else {
            (
                ctx.command.clone(),
                ctx.args.clone(),
                format!("{} {:?}", ctx.command, ctx.args),
            )
        };

        let tools_use_bwrap: bool = env::var("TOOLS_USE_BWRAP")
            .unwrap_or_else(|_| "false".to_string())
            .parse::<bool>()
            .unwrap_or(false);

        let can_use_bwrap = tools_use_bwrap && cfg!(target_os = "linux") && Self::is_bwrap_available();
        if tools_use_bwrap && !can_use_bwrap {
            warn!(
                execution_id = %ctx.execution_id,
                twin_id = %ctx.twin_id,
                "TOOLS_USE_BWRAP=true but bubblewrap is not available on this platform/runtime; falling back to cwd isolation"
            );
        }

        info!(
            execution_id = %ctx.execution_id,
            twin_id = %ctx.twin_id,
            job_id = %ctx.job_id,
            sandbox = %execution_dir.display(),
            cmd = %command_display,
            "Executing command (sandboxed)"
        );

        let mut cmd = if can_use_bwrap {
            // bubblewrap sandbox (Linux): isolate mount/user/pid/net/etc namespaces.
            // Root filesystem becomes the twin sandbox dir.
            //
            // Required form:
            //   bwrap --unshare-all --die-with-parent --bind <SANDBOX_DIR>/<twin_id>/ / --setenv PATH /usr/bin -- <COMMAND> <ARGS>
            let twin_root_abs = std::fs::canonicalize(&twin_root_dir).unwrap_or_else(|_| twin_root_dir.clone());
            let exec_id_dir_name = ctx.execution_id.to_string();

            let mut bwrap = Command::new("bwrap");
            bwrap
                .arg("--unshare-all")
                // Explicitly unshare the network namespace (defense-in-depth).
                .arg("--unshare-net")
                .arg("--die-with-parent")
                .arg("--bind")
                .arg(&twin_root_abs)
                .arg("/")
                // Minimal safe execution environment
                .arg("--setenv")
                .arg("PATH")
                .arg("/usr/bin")
                // Run in the per-execution directory under the sandbox root.
                .arg("--chdir")
                .arg(format!("/{}", exec_id_dir_name))
                .arg("--");

            // NOTE: Without additional binds, most dynamically-linked binaries (and /bin/sh)
            // will not exist inside the sandbox. We provide read-only access to common system
            // paths so typical tooling can run, while keeping the sandbox writable only inside
            // the bound root.
            Self::bwrap_ro_bind_if_exists(&mut bwrap, "/usr", "/usr");
            Self::bwrap_ro_bind_if_exists(&mut bwrap, "/bin", "/bin");
            Self::bwrap_ro_bind_if_exists(&mut bwrap, "/lib", "/lib");
            Self::bwrap_ro_bind_if_exists(&mut bwrap, "/lib64", "/lib64");
            Self::bwrap_ro_bind_if_exists(&mut bwrap, "/etc", "/etc");

            // Final execution: bwrap ... -- <COMMAND> <ARGS>
            bwrap.arg(&program).args(&argv);

            bwrap
        } else {
            // Legacy cwd-isolation fallback (cross-platform).
            let mut c = Command::new(&program);
            c.args(&argv).current_dir(&execution_dir);
            c
        };

        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            // Minimal environment (we also set env in bwrap via --setenv)
            .env_clear();

        if !can_use_bwrap {
            // For legacy mode we still set a minimal environment on the executed process.
            if cfg!(windows) {
                cmd.env("TEMP", execution_dir.to_string_lossy().to_string());
                cmd.env("TMP", execution_dir.to_string_lossy().to_string());
            } else {
                cmd.env("HOME", execution_dir.to_string_lossy().to_string());
                cmd.env("TMPDIR", execution_dir.to_string_lossy().to_string());
                cmd.env("PATH", "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin");
            }
        }

        // Hard timeout to avoid runaway processes.
        let timeout_ms: u64 = env::var("TOOLS_EXEC_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(10_000);

        let output = match tokio::time::timeout(std::time::Duration::from_millis(timeout_ms), cmd.output()).await {
            Ok(Ok(out)) => out,
            Ok(Err(e)) => {
                error!(
                    execution_id = %ctx.execution_id,
                    error = %e,
                    cmd = %command_display,
                    "Command execution failed to spawn/run"
                );
                return Self::map_spawn_error_to_response(ctx.execution_id, e, &command_display);
            }
            Err(_) => {
                return ExecutionResponse {
                    success: false,
                    exit_code: 124,
                    stdout_logs: vec![],
                    stderr_logs: vec![format!(
                        "Execution timed out after {}ms: {}",
                        timeout_ms, command_display
                    )],
                    message: "execution_timeout".to_string(),
                    execution_id: ctx.execution_id.to_string(),
                };
            }
        };

        let exit_code: i32 = output.status.code().unwrap_or(1);

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let stdout_logs = Self::split_lines_limited(&stdout, 200, 32 * 1024);
        let stderr_logs = Self::split_lines_limited(&stderr, 200, 32 * 1024);

        let success = output.status.success();
        let elapsed_ms: i64 = (chrono::Utc::now() - ctx.start_time).num_milliseconds();

        let message = if success {
            format!("Execution completed (elapsed_ms={})", elapsed_ms)
        } else {
            format!("Execution failed (exit_code={}, elapsed_ms={})", exit_code, elapsed_ms)
        };

        ExecutionResponse {
            success,
            exit_code,
            stdout_logs,
            stderr_logs,
            message,
            execution_id: ctx.execution_id.to_string(),
        }
    }

    fn is_bwrap_available() -> bool {
        // Cheap availability check without relying on a shell.
        // We intentionally use std::process here; this is called once per execution request.
        StdCommand::new("bwrap")
            .arg("--version")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    fn bwrap_ro_bind_if_exists(cmd: &mut Command, src: &str, dst: &str) {
        if std::path::Path::new(src).exists() {
            cmd.arg("--ro-bind").arg(src).arg(dst);
        }
    }
}

#[tonic::async_trait]
impl ToolExecutorService for ToolExecutorServiceImpl {
    async fn request_execution(
        &self,
        request: Request<ExecutionRequest>,
    ) -> Result<Response<ExecutionResponse>, Status> {
        let req = request.into_inner();
        let execution_id = Uuid::new_v4();

        info!(
            execution_id = %execution_id,
            command = %req.command,
            twin_id = %req.twin_id,
            job_id = %req.job_id,
            "Received execution request"
        );

        // Policy check
        let (authorized, reason) = self.policy.is_authorized(&req.twin_id, &req.command);
        
        if !authorized {
            warn!(
                execution_id = %execution_id,
                twin_id = %req.twin_id,
                command = %req.command,
                reason = %reason,
                "Execution denied by policy"
            );

            return Ok(Response::new(ExecutionResponse {
                success: false,
                exit_code: 1,
                stdout_logs: vec![],
                stderr_logs: vec![format!("Policy violation: {}", reason)],
                message: reason,
                execution_id: execution_id.to_string(),
            }));
        }

        // Create execution context
        let ctx = ExecutionContext {
            execution_id,
            command: req.command.clone(),
            args: req.args.clone(),
            twin_id: req.twin_id.clone(),
            job_id: req.job_id.clone(),
            sandbox_dir: self.sandbox_dir.clone(),
            start_time: chrono::Utc::now(),
        };

        // Record execution in history
        {
            let mut history = self.execution_history.write().await;
            history.push(ctx.clone());
            // Keep only last 1000 executions
            if history.len() > 1000 {
                history.remove(0);
            }
        }

        // Execute command (sandboxed implementation)
        let response = self.execute_command_sandboxed(&ctx).await;

        info!(
            execution_id = %execution_id,
            success = response.success,
            exit_code = response.exit_code,
            "Execution completed"
        );

        Ok(Response::new(response))
    }

    async fn health_check(
        &self,
        _request: Request<()>,
    ) -> Result<Response<HealthCheckResponse>, Status> {
        Ok(Response::new(HealthCheckResponse {
            status: "healthy".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            message: "Tool executor service is operational".to_string(),
        }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "backend_rust_tools=info,tonic=info".into()),
        )
        .init();

    // Load environment variables
    dotenvy::dotenv().ok();

    // Get gRPC port from environment
    let grpc_port = env::var("TOOLS_GRPC_PORT")
        .unwrap_or_else(|_| "50054".to_string())
        .parse::<u16>()
        .expect("TOOLS_GRPC_PORT must be a valid port number");

    let addr = format!("0.0.0.0:{}", grpc_port)
        .parse()
        .expect("Invalid address");

    let service = ToolExecutorServiceImpl::new()?;

    info!(
        addr = %addr,
        port = grpc_port,
        sandbox_dir = %service.sandbox_dir.display(),
        safe_mode = service.policy.safe_mode,
        "Starting Tool Executor gRPC server"
    );

    Server::builder()
        .add_service(ToolExecutorServiceServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
