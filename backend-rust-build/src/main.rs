use std::env;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use std::process::Stdio;

use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tonic::{transport::Server, Request, Response, Status};
use tracing::{error, info, warn};

// Include the generated proto code
pub mod proto {
    tonic::include_proto!("build");
}

use proto::build_service_server::{BuildService, BuildServiceServer};
use proto::{CreateToolRequest, CreateToolResponse, HealthCheckRequest, HealthCheckResponse};

#[derive(Debug)]
struct BuildLimiter {
    /// Limits concurrent compilations.
    semaphore: Arc<Semaphore>,
    /// Simple queue-capacity guard to avoid unbounded memory/CPU pressure.
    pending: AtomicUsize,
    max_pending: usize,
}

impl BuildLimiter {
    fn new(max_concurrent: usize, max_pending: usize) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(max_concurrent.max(1))),
            pending: AtomicUsize::new(0),
            max_pending: max_pending.max(1),
        }
    }

    async fn acquire(&self) -> Result<BuildGuard<'_>, Status> {
        let cur = self.pending.fetch_add(1, Ordering::SeqCst) + 1;
        if cur > self.max_pending {
            self.pending.fetch_sub(1, Ordering::SeqCst);
            return Err(Status::resource_exhausted("build queue is full"));
        }

        let permit = Arc::clone(&self.semaphore)
            .acquire_owned()
            .await
            .map_err(|_| Status::internal("build limiter closed"))?;
        Ok(BuildGuard {
            limiter: self,
            _permit: permit,
        })
    }
}

/// RAII guard that decrements the pending counter when the request finishes.
struct BuildGuard<'a> {
    limiter: &'a BuildLimiter,
    _permit: OwnedSemaphorePermit,
}

impl Drop for BuildGuard<'_> {
    fn drop(&mut self) {
        self.limiter.pending.fetch_sub(1, Ordering::SeqCst);
    }
}

#[derive(Debug, Clone)]
struct BuildConfig {
    tools_repo_dir: PathBuf,
    build_timeout_ms: u64,
}

impl BuildConfig {
    fn from_env() -> Self {
        let tools_repo_dir = env::var("TOOLS_REPO_DIR")
            .unwrap_or_else(|_| "tools_repo".to_string())
            .into();

        let build_timeout_ms = env::var("BUILD_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(120_000);

        Self {
            tools_repo_dir,
            build_timeout_ms,
        }
    }
}

#[derive(Debug)]
pub struct BuildServiceImpl {
    cfg: BuildConfig,
    limiter: Arc<BuildLimiter>,
}

impl BuildServiceImpl {
    fn new() -> Self {
        let max_concurrent = env::var("BUILD_MAX_CONCURRENT")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(1);

        let max_pending = env::var("BUILD_MAX_PENDING")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(4);

        Self {
            cfg: BuildConfig::from_env(),
            limiter: Arc::new(BuildLimiter::new(max_concurrent, max_pending)),
        }
    }

    fn validate_tool_name(name: &str) -> Result<(), Status> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Status::invalid_argument("tool_name is required"));
        }

        // Prevent path traversal and keep it Cargo-friendly.
        let ok = name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
        if !ok {
            return Err(Status::invalid_argument(
                "tool_name must be [A-Za-z0-9_-] only",
            ));
        }

        Ok(())
    }

    fn render_main_rs(tool_code: &str) -> String {
        let code = tool_code.trim();
        if code.contains("fn main") {
            return format!("{}\n", code);
        }

        // Best-effort wrapper for callers that provide only a function body.
        format!(
            "fn main() {{\n{code}\n}}\n",
            code = code
        )
    }

    fn tool_dir(&self, tool_name: &str) -> PathBuf {
        self.cfg.tools_repo_dir.join(tool_name)
    }

    async fn ensure_tool_skeleton(&self, tool_name: &str) -> Result<PathBuf, Status> {
        let tool_dir = self.tool_dir(tool_name);
        let src_dir = tool_dir.join("src");
        let main_rs = src_dir.join("main.rs");
        let cargo_toml = tool_dir.join("Cargo.toml");

        tokio::fs::create_dir_all(&src_dir)
            .await
            .map_err(|e| Status::internal(format!("failed to create tool dir: {e}")))?;

        if tokio::fs::metadata(&cargo_toml).await.is_err() {
            // Minimal tool crate definition.
            // NOTE: no dependencies are declared; tool_code must compile with std.
            let cargo_contents = format!(
                "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\n",
                name = tool_name
            );

            tokio::fs::write(&cargo_toml, cargo_contents)
                .await
                .map_err(|e| Status::internal(format!("failed to write Cargo.toml: {e}")))?;
        }

        Ok(main_rs)
    }

    async fn compile_tool(&self, tool_dir: &Path) -> Result<(i32, String, String), Status> {
        // Hard timeout to prevent runaway builds.
        let timeout = std::time::Duration::from_millis(self.cfg.build_timeout_ms);

        let mut cmd = Command::new("cargo");
        cmd.arg("build").current_dir(tool_dir);

        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // We preserve the existing environment to keep cargo/rustc workable.
        cmd.kill_on_drop(true);

        let mut child = cmd
            .spawn()
            .map_err(|e| Status::internal(format!("failed to spawn cargo build: {e}")))?;

        let mut stdout = child.stdout.take();
        let mut stderr = child.stderr.take();

        let stdout_task = tokio::spawn(async move {
            let mut buf = Vec::new();
            if let Some(out) = &mut stdout {
                let _ = out.read_to_end(&mut buf).await;
            }
            buf
        });

        let stderr_task = tokio::spawn(async move {
            let mut buf = Vec::new();
            if let Some(err) = &mut stderr {
                let _ = err.read_to_end(&mut buf).await;
            }
            buf
        });

        let status = match tokio::time::timeout(timeout, child.wait()).await {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => {
                return Err(Status::internal(format!("failed to await cargo build: {e}")));
            }
            Err(_) => {
                warn!(timeout_ms = self.cfg.build_timeout_ms, "cargo build timed out; killing process");
                let _ = child.kill().await;
                let _ = child.wait().await;
                let _ = stdout_task.abort();
                let _ = stderr_task.abort();
                return Ok((
                    124,
                    "".to_string(),
                    format!("build timed out after {}ms", self.cfg.build_timeout_ms),
                ));
            }
        };

        let out_bytes = stdout_task.await.unwrap_or_default();
        let err_bytes = stderr_task.await.unwrap_or_default();

        let exit_code: i32 = status.code().unwrap_or(1);
        let stdout = String::from_utf8_lossy(&out_bytes).to_string();
        let stderr = String::from_utf8_lossy(&err_bytes).to_string();

        Ok((exit_code, stdout, stderr))
    }
}

#[tonic::async_trait]
impl BuildService for BuildServiceImpl {
    async fn create_tool(
        &self,
        request: Request<CreateToolRequest>,
    ) -> Result<Response<CreateToolResponse>, Status> {
        let _guard = self.limiter.acquire().await?;

        let req = request.into_inner();
        Self::validate_tool_name(&req.tool_name)?;

        let tool_name = req.tool_name.trim().to_string();
        let tool_dir = self.tool_dir(&tool_name);

        info!(tool_name = %tool_name, tool_dir = %tool_dir.display(), "CreateTool request received");

        let main_rs = self.ensure_tool_skeleton(&tool_name).await?;

        let rendered = Self::render_main_rs(&req.tool_code);
        if rendered.trim().is_empty() {
            return Err(Status::invalid_argument("tool_code is required"));
        }

        if let Err(e) = tokio::fs::write(&main_rs, rendered).await {
            return Err(Status::internal(format!("failed to write main.rs: {e}")));
        }

        let (exit_code, stdout, stderr) = self.compile_tool(&tool_dir).await?;

        let success = exit_code == 0;
        if !success {
            warn!(tool_name = %tool_name, exit_code = exit_code, "tool compilation failed");
        }

        Ok(Response::new(CreateToolResponse {
            success,
            exit_code,
            stdout,
            stderr,
            tool_dir: tool_dir.to_string_lossy().to_string(),
        }))
    }

    async fn health_check(
        &self,
        _request: Request<HealthCheckRequest>,
    ) -> Result<Response<HealthCheckResponse>, Status> {
        Ok(Response::new(HealthCheckResponse {
            status: "healthy".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            message: "Build service is operational".to_string(),
        }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "backend_rust_build=info,tonic=info".into()),
        )
        .init();

    dotenvy::dotenv().ok();

    let grpc_port = env::var("BUILD_SERVICE_PORT")
        .unwrap_or_else(|_| "50055".to_string())
        .parse::<u16>()
        .expect("BUILD_SERVICE_PORT must be a valid port number");

    let addr = format!("0.0.0.0:{}", grpc_port)
        .parse()
        .expect("Invalid address");

    let svc = BuildServiceImpl::new();
    info!(addr = %addr, tools_repo_dir = %svc.cfg.tools_repo_dir.display(), "Starting Build gRPC server");

    if let Err(e) = tokio::fs::create_dir_all(&svc.cfg.tools_repo_dir).await {
        error!(error = %e, dir = %svc.cfg.tools_repo_dir.display(), "Failed to create TOOLS_REPO_DIR");
        return Err(e.into());
    }

    Server::builder()
        .add_service(BuildServiceServer::new(svc))
        .serve(addr)
        .await?;

    Ok(())
}

