use axum::{
    body::Body,
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::Response;
use serde::{Deserialize, Serialize};
use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::{info, error, warn};
use uuid::Uuid;
use futures::StreamExt;

// --- Shared State ---
#[derive(Clone)]
struct AppState {
    orchestrator_url: String, // e.g., http://127.0.0.1:8182
    telemetry_url: String,   // e.g., http://127.0.0.1:8183
    http_client: reqwest::Client,
}

// --- Protocol Types (matching frontend) ---
#[derive(Debug, Deserialize)]
struct ChatRequest {
    session_id: String,
    user_id: String,
    #[serde(default)]
    timestamp: Option<String>,
    message: String,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum ChatResponse {
    #[serde(rename = "complete_message")]
    CompleteMessage {
        id: String,
        content: String,
        is_final: bool,
        latency_ms: u64,
        source_memories: Vec<String>,
        issued_command: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        raw_orchestrator_decision: Option<String>,
    },
    #[serde(rename = "message_chunk")]
    MessageChunk {
        id: String,
        content_chunk: String,
        is_final: bool,
    },
    #[serde(rename = "status_update")]
    StatusUpdate {
        status: String,
        details: Option<String>,
    },
}

// --- Orchestrator Request/Response Types ---
#[derive(Debug, Serialize)]
struct OrchestratorRequest {
    message: String,
    twin_id: String,
    session_id: String,
    namespace: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OrchestratorResponse {
    response: String,
    job_id: Option<String>,
    actions_taken: Vec<String>,
    status: String,
    #[serde(default)]
    issued_command: Option<serde_json::Value>,
    #[serde(default)]
    raw_orchestrator_decision: Option<String>,
}

// --- WebSocket Handler ---
async fn ws_handler(
    ws: WebSocketUpgrade,
    Path(user_id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    info!(user_id = %user_id, "WebSocket connection requested");
    ws.on_upgrade(move |socket| handle_socket(socket, user_id, state))
}

async fn handle_socket(mut socket: WebSocket, user_id: String, state: Arc<AppState>) {
    info!(user_id = %user_id, "WebSocket connection established");

    while let Some(Ok(msg)) = socket.recv().await {
        match msg {
            Message::Text(text) => {
                // Parse ChatRequest from WebSocket
                let chat_request: ChatRequest = match serde_json::from_str::<ChatRequest>(&text) {
                    Ok(req) => {
                        // Validate user_id matches path
                        if req.user_id != user_id {
                            error!(
                                path_user_id = %user_id,
                                body_user_id = %req.user_id,
                                "User ID mismatch"
                            );
                            let error_response = ChatResponse::StatusUpdate {
                                status: "error".to_string(),
                                details: Some("User ID mismatch between path and request payload.".to_string()),
                            };
                            if let Err(e) = socket.send(Message::Text(serde_json::to_string(&error_response).unwrap())).await {
                                error!(error = %e, "Failed to send error response");
                            }
                            continue;
                        }
                        req
                    }
                    Err(e) => {
                        error!(error = %e, "Failed to parse ChatRequest");
                        let error_response = ChatResponse::StatusUpdate {
                            status: "error".to_string(),
                            details: Some(format!("Invalid JSON payload: {}", e)),
                        };
                        if let Err(e) = socket.send(Message::Text(serde_json::to_string(&error_response).unwrap())).await {
                            error!(error = %e, "Failed to send error response");
                        }
                        continue;
                    }
                };

                info!(
                    user_id = %user_id,
                    session_id = %chat_request.session_id,
                    message = %chat_request.message,
                    "Processing chat message"
                );

                // Translate to Orchestrator request
                let orchestrator_request = OrchestratorRequest {
                    message: chat_request.message.clone(),
                    twin_id: chat_request.user_id.clone(),
                    session_id: chat_request.session_id.clone(),
                    namespace: None, // Could be extracted from metadata if needed
                };

                // Proxy to Orchestrator HTTP endpoint
                match proxy_to_orchestrator(&state, &orchestrator_request).await {
                    Ok(orchestrator_response) => {
                        // Convert Orchestrator response to ChatResponse
                        let chat_response = ChatResponse::CompleteMessage {
                            id: Uuid::new_v4().to_string(),
                            content: orchestrator_response.response,
                            is_final: true,
                            latency_ms: 0, // Could track actual latency
                            source_memories: orchestrator_response.actions_taken,
                            issued_command: orchestrator_response.issued_command,
                            raw_orchestrator_decision: orchestrator_response.raw_orchestrator_decision,
                        };

                        let response_json = match serde_json::to_string(&chat_response) {
                            Ok(json) => json,
                            Err(e) => {
                                error!(error = %e, "Failed to serialize ChatResponse");
                                let error_response = ChatResponse::StatusUpdate {
                                    status: "error".to_string(),
                                    details: Some("Failed to serialize response".to_string()),
                                };
                                serde_json::to_string(&error_response).unwrap()
                            }
                        };

                        if let Err(e) = socket.send(Message::Text(response_json)).await {
                            error!(user_id = %user_id, error = %e, "Failed to send response to client");
                            break;
                        }
                    }
                    Err(e) => {
                        error!(user_id = %user_id, error = %e, "Orchestrator request failed");
                        let error_response = ChatResponse::StatusUpdate {
                            status: "error".to_string(),
                            details: Some(format!("Orchestrator error: {}", e)),
                        };
                        if let Err(e) = socket.send(Message::Text(serde_json::to_string(&error_response).unwrap())).await {
                            error!(error = %e, "Failed to send error response");
                            break;
                        }
                    }
                }
            }
            Message::Close(_) => {
                info!(user_id = %user_id, "WebSocket closed by client");
                break;
            }
            _ => {
                warn!(user_id = %user_id, "Received unsupported message type");
            }
        }
    }

    info!(user_id = %user_id, "WebSocket connection closed");
}

// --- HTTP Proxy to Orchestrator ---
async fn proxy_to_orchestrator(
    state: &AppState,
    request: &OrchestratorRequest,
) -> Result<OrchestratorResponse, String> {
    let orchestrator_endpoint = format!("{}/chat", state.orchestrator_url);

    info!(
        endpoint = %orchestrator_endpoint,
        message = %request.message,
        "Proxying request to Orchestrator"
    );

    let response = state
        .http_client
        .post(&orchestrator_endpoint)
        .header("Content-Type", "application/json")
        .header("X-User-ID", &request.twin_id)
        .json(request)
        .send()
        .await
        .map_err(|e| format!("Request to orchestrator failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("Orchestrator returned error status {}: {}", status, error_text));
    }

    let orchestrator_response: OrchestratorResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse orchestrator response: {}", e))?;

    info!(
        response_status = %orchestrator_response.status,
        actions_count = orchestrator_response.actions_taken.len(),
        "Received response from Orchestrator"
    );

    Ok(orchestrator_response)
}

// --- Telemetry SSE Proxy Handler ---
async fn telemetry_proxy_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    info!("Telemetry SSE proxy requested");

    let telemetry_endpoint = format!(
        "{}/v1/telemetry/stream",
        state.telemetry_url.trim_end_matches('/')
    );

    let upstream = match state.http_client.get(&telemetry_endpoint).send().await {
        Ok(r) => r,
        Err(e) => {
            error!(error = %e, endpoint = %telemetry_endpoint, "Failed to connect to telemetry service");
            return (
                StatusCode::BAD_GATEWAY,
                "Telemetry service unavailable",
            )
                .into_response();
        }
    };

    if !upstream.status().is_success() {
        let status = upstream.status();
        let body = upstream.text().await.unwrap_or_default();
        warn!(status = %status, endpoint = %telemetry_endpoint, "Telemetry service returned non-success");
        return (
            StatusCode::BAD_GATEWAY,
            format!("Telemetry upstream error {}: {}", status, body),
        )
            .into_response();
    }

    let stream = upstream.bytes_stream().map(|chunk| {
        chunk.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    });

    let mut resp = Response::new(Body::from_stream(stream));
    *resp.status_mut() = StatusCode::OK;
    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/event-stream"),
    );
    resp.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-cache"),
    );
    resp.headers_mut().insert(
        header::CONNECTION,
        HeaderValue::from_static("keep-alive"),
    );

    // Allow the Vite dev server (different origin) to consume SSE.
    // WebSockets do not require CORS in the same way, but EventSource does.
    resp.headers_mut().insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );

    resp
}

// --- Health Check ---
async fn health_check() -> impl IntoResponse {
    (axum::http::StatusCode::OK, "Gateway operational")
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "backend_rust_gateway=info,axum=info".into()),
        )
        .init();

    // Load environment variables
    dotenvy::dotenv().ok();

    // Get configuration
    let orchestrator_url = env::var("ORCHESTRATOR_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8182".to_string());

    // Defensive trimming: Windows env vars are easy to accidentally set with trailing whitespace.
    let orchestrator_url = orchestrator_url.trim().to_string();

    let telemetry_url = env::var("TELEMETRY_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8183".to_string());

    let telemetry_url = telemetry_url.trim().to_string();

    let gateway_port = env::var("GATEWAY_PORT")
        .unwrap_or_else(|_| "8181".to_string())
        .parse::<u16>()
        .expect("GATEWAY_PORT must be a valid port number");

    info!(
        orchestrator_url = %orchestrator_url,
        telemetry_url = %telemetry_url,
        gateway_port = gateway_port,
        "Initializing WebSocket Gateway"
    );

    // Create HTTP client
    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .expect("Failed to create HTTP client");

    // Create application state
    let app_state = Arc::new(AppState {
        orchestrator_url,
        telemetry_url,
        http_client,
    });

    // Create router
    let app = Router::new()
        .route("/api/health", get(health_check))
        .route("/ws/chat/:user_id", get(ws_handler))
        .route("/v1/telemetry/stream", get(telemetry_proxy_handler))
        .with_state(app_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], gateway_port));

    info!(
        addr = %addr,
        port = gateway_port,
        "Starting WebSocket Gateway server"
    );

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
