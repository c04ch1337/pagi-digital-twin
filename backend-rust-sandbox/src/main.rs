use axum::{
    extract::Json,
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Router,
};
use serde::Serialize;
use std::{env, net::SocketAddr};
use tracing::{info, Level};
use tracing_subscriber::{prelude::*, Registry};

mod tool;
mod tool_executor;
mod tool_web_search;
mod tool_service;
use tool::{execute_mock_tool, ToolExecutionRequest, ToolExecutionResponse};

const DEFAULT_PORT: u16 = 8001;
const DEFAULT_GRPC_PORT: u16 = 50053;
const SERVICE_NAME: &str = "backend-rust-sandbox";
const VERSION: &str = "1.0.0";

#[derive(Serialize)]
struct HealthResponse {
    service: &'static str,
    status: &'static str,
    version: &'static str,
}

async fn health_check() -> (StatusCode, Json<HealthResponse>) {
    (
        StatusCode::OK,
        Json(HealthResponse {
            service: SERVICE_NAME,
            status: "ok",
            version: VERSION,
        }),
    )
}

async fn handle_execute_tool(
    headers: HeaderMap,
    Json(payload): Json<ToolExecutionRequest>,
) -> (StatusCode, Json<ToolExecutionResponse>) {
    let request_id = headers
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("none");

    // Log structured JSON request details
    info!(
        request_id = request_id,
        method = "POST",
        tool_name = payload.tool_name,
        message = "Received tool execution request."
    );

    let response = execute_mock_tool(payload).await;
    (StatusCode::OK, Json(response))
}

fn init_logging(log_level: &str) {
    let level = log_level.parse::<Level>().unwrap_or(Level::INFO);

    let subscriber = Registry::default().with(
        tracing_subscriber::fmt::layer()
            .json()
            .with_current_span(false)
            .with_target(true)
            .with_level(true)
            .with_filter(
                tracing_subscriber::EnvFilter::from_default_env().add_directive(level.into()),
            ),
    );
    tracing::subscriber::set_global_default(subscriber)
        .expect("Unable to set global tracing subscriber");
}

#[tokio::main]
async fn main() {
    // Load .env for bare metal if needed
    dotenvy::dotenv().ok();

    let port_str = env::var("RUST_SANDBOX_PORT").unwrap_or_else(|_| DEFAULT_PORT.to_string());
    let grpc_port_str =
        env::var("RUST_SANDBOX_GRPC_PORT").unwrap_or_else(|_| DEFAULT_GRPC_PORT.to_string());
    let log_level = env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string());
    let port = port_str.parse::<u16>().unwrap_or(DEFAULT_PORT);
    let grpc_port = grpc_port_str.parse::<u16>().unwrap_or(DEFAULT_GRPC_PORT);

    init_logging(&log_level);

    // Bind to all interfaces so it works in Docker and bare metal.
    let http_addr = SocketAddr::from(([0, 0, 0, 0], port));
    let grpc_addr = SocketAddr::from(([0, 0, 0, 0], grpc_port));

    info!(
        service = SERVICE_NAME,
        version = VERSION,
        http_port = port,
        grpc_port = grpc_port,
        message = "Starting servers..."
    );

    let app = Router::new()
        .route("/health", get(health_check))
        // New primary route used by the Python Agent.
        .route("/execute-tool", post(handle_execute_tool))
        // Backwards-compatible route used elsewhere in the stack.
        .route("/api/v1/execute_tool", post(handle_execute_tool));

    let http_task = tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind(&http_addr).await.unwrap();
        axum::serve(listener, app).await.unwrap();
    });

    let grpc_task = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(tool_service::tool_service_server())
            .serve(grpc_addr)
            .await
            .unwrap();
    });

    // Run both servers until one of them exits.
    let _ = tokio::join!(http_task, grpc_task);
}

