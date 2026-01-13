use axum::{
    body::Body,
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Query, Request, State,
    },
    http::HeaderMap,
    response::IntoResponse,
    routing::{delete, get, post},
    Router,
};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::Response;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tracing::{info, error, warn};
use uuid::Uuid;
use futures::StreamExt;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};

// --- Shared State ---
struct AppState {
    orchestrator_url: String, // e.g., http://127.0.0.1:8182
    telemetry_url: String,   // e.g., http://127.0.0.1:8183
    http_client: reqwest::Client,
    signaling_rooms: Mutex<HashMap<String, broadcast::Sender<String>>>,
}

// --- Protocol Types (matching frontend) ---
#[derive(Debug, Deserialize)]
struct ChatRequest {
    session_id: String,
    user_id: String,
    #[serde(default)]
    timestamp: Option<String>,
    message: String,
    #[serde(default)]
    media_active: bool,
    #[serde(default)]
    user_name: Option<String>,
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
    #[serde(default)]
    media_active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_name: Option<String>,
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

// --- WebRTC Signaling Relay (WebSocket room broadcast) ---
async fn signaling_ws_handler(
    ws: WebSocketUpgrade,
    Path(room_id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    info!(room_id = %room_id, "Signaling WebSocket connection requested");
    ws.on_upgrade(move |socket| handle_signaling_socket(socket, room_id, state))
}

async fn handle_signaling_socket(mut socket: WebSocket, room_id: String, state: Arc<AppState>) {
    let (tx, mut rx) = {
        let mut rooms = state.signaling_rooms.lock().await;
        let entry = rooms.entry(room_id.clone()).or_insert_with(|| {
            let (tx, _rx) = broadcast::channel::<String>(128);
            tx
        });
        (entry.clone(), entry.subscribe())
    };

    info!(room_id = %room_id, "Signaling WebSocket connected");

    loop {
        tokio::select! {
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        // Broadcast signaling payload to room.
                        let _ = tx.send(text);
                    }
                    Some(Ok(Message::Close(_))) => break,
                    Some(Ok(_)) => {
                        // ignore binary/ping
                    }
                    Some(Err(e)) => {
                        warn!(room_id = %room_id, error = %e, "Signaling WS receive error");
                        break;
                    }
                    None => break,
                }
            }
            broadcasted = rx.recv() => {
                match broadcasted {
                    Ok(text) => {
                        // Echo is acceptable; clients can ignore their own messages via ids.
                        if let Err(e) = socket.send(Message::Text(text)).await {
                            warn!(room_id = %room_id, error = %e, "Signaling WS send error");
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        warn!(room_id = %room_id, skipped = skipped, "Signaling WS lagged");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }

    info!(room_id = %room_id, "Signaling WebSocket disconnected");
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
                    media_active: chat_request.media_active,
                    user_name: chat_request.user_name.clone(),
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

// --- Media Upload Proxy Handler (stateless) ---
async fn media_upload_options() -> impl IntoResponse {
    // Preflight for browser uploads
    let mut resp = Response::new(Body::empty());
    *resp.status_mut() = StatusCode::NO_CONTENT;
    resp.headers_mut().insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    resp.headers_mut().insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("POST, OPTIONS"),
    );
    resp.headers_mut().insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static("content-type"),
    );
    resp
}

async fn media_upload_proxy_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Body,
) -> impl IntoResponse {
    let telemetry_endpoint = format!(
        "{}/internal/media/store",
        state.telemetry_url.trim_end_matches('/')
    );

    // Stateless proxy: do not buffer the request body.
    // We forward the multipart stream as-is so telemetry can parse & store it.
    let byte_stream = body.into_data_stream().map(|chunk| {
        chunk.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    });

    let mut req = state
        .http_client
        .post(&telemetry_endpoint)
        .body(reqwest::Body::wrap_stream(byte_stream));

    // Forward Content-Type (multipart boundary) so telemetry can parse the form.
    if let Some(ct) = headers.get(header::CONTENT_TYPE).cloned() {
        req = req.header(header::CONTENT_TYPE, ct);
    }

    // Forward content-length when present (optional, but can help upstream).
    if let Some(cl) = headers.get(header::CONTENT_LENGTH).cloned() {
        req = req.header(header::CONTENT_LENGTH, cl);
    }

    info!(endpoint = %telemetry_endpoint, "Proxying streamed multipart upload to telemetry");

    let upstream = req.send().await;

    match upstream {
        Ok(r) => {
            let status = r.status();
            let bytes = r.bytes().await.unwrap_or_default();

            let mut resp = Response::new(Body::from(bytes));
            *resp.status_mut() = status;
            resp.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_static("*"),
            );
            resp
        }
        Err(e) => {
            error!(error = %e, endpoint = %telemetry_endpoint, "Failed to proxy media upload");
            let mut resp = Response::new(Body::from("Telemetry service unavailable"));
            *resp.status_mut() = StatusCode::BAD_GATEWAY;
            resp.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_static("*"),
            );
            resp
        }
    }
}

// --- Media Gallery Proxy Handlers ---

async fn media_list_proxy_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let telemetry_endpoint = format!(
        "{}/v1/media/list",
        state.telemetry_url.trim_end_matches('/')
    );

    // Build query string from params
    let mut query_parts = Vec::new();
    if let Some(twin_id) = params.get("twin_id") {
        query_parts.push(format!("twin_id={}", utf8_percent_encode(twin_id, NON_ALPHANUMERIC)));
    }
    if let Some(limit) = params.get("limit") {
        query_parts.push(format!("limit={}", utf8_percent_encode(limit, NON_ALPHANUMERIC)));
    }
    let query_string = if query_parts.is_empty() {
        String::new()
    } else {
        format!("?{}", query_parts.join("&"))
    };

    let endpoint = format!("{}{}", telemetry_endpoint, query_string);

    match state.http_client.get(&endpoint).send().await {
        Ok(r) => {
            let status = r.status();
            let bytes = r.bytes().await.unwrap_or_default();

            let mut resp = Response::new(Body::from(bytes));
            *resp.status_mut() = status;
            resp.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
            resp.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_static("*"),
            );
            resp
        }
        Err(e) => {
            error!(error = %e, endpoint = %endpoint, "Failed to proxy media list");
            let mut resp = Response::new(Body::from("Telemetry service unavailable"));
            *resp.status_mut() = StatusCode::BAD_GATEWAY;
            resp.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_static("*"),
            );
            resp
        }
    }
}

async fn media_view_proxy_handler(
    State(state): State<Arc<AppState>>,
    Path(filename): Path<String>,
    request: Request,
) -> impl IntoResponse {
    let telemetry_endpoint = format!(
        "{}/internal/media/view/{}",
        state.telemetry_url.trim_end_matches('/'),
        utf8_percent_encode(&filename, NON_ALPHANUMERIC)
    );

    // Forward Range header if present for video scrubbing
    let mut req = state.http_client.get(&telemetry_endpoint);
    if let Some(range) = request.headers().get(header::RANGE) {
        req = req.header(header::RANGE, range);
    }

    match req.send().await {
        Ok(r) => {
            let status = r.status();
            let headers = r.headers().clone();
            let bytes = r.bytes().await.unwrap_or_default();

            let mut resp = Response::new(Body::from(bytes));
            *resp.status_mut() = status;
            
            // Forward important headers for video playback
            if let Some(content_type) = headers.get(header::CONTENT_TYPE) {
                resp.headers_mut().insert(header::CONTENT_TYPE, content_type.clone());
            }
            if let Some(content_length) = headers.get(header::CONTENT_LENGTH) {
                resp.headers_mut().insert(header::CONTENT_LENGTH, content_length.clone());
            }
            if let Some(accept_ranges) = headers.get(header::ACCEPT_RANGES) {
                resp.headers_mut().insert(header::ACCEPT_RANGES, accept_ranges.clone());
            }
            if let Some(content_range) = headers.get(header::CONTENT_RANGE) {
                resp.headers_mut().insert(header::CONTENT_RANGE, content_range.clone());
            }
            
            resp.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_static("*"),
            );
            resp
        }
        Err(e) => {
            error!(error = %e, endpoint = %telemetry_endpoint, "Failed to proxy media view");
            let mut resp = Response::new(Body::from("Telemetry service unavailable"));
            *resp.status_mut() = StatusCode::BAD_GATEWAY;
            resp.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_static("*"),
            );
            resp
        }
    }
}

async fn media_delete_proxy_handler(
    State(state): State<Arc<AppState>>,
    Path(filename): Path<String>,
) -> impl IntoResponse {
    let telemetry_endpoint = format!(
        "{}/internal/media/delete/{}",
        state.telemetry_url.trim_end_matches('/'),
        utf8_percent_encode(&filename, NON_ALPHANUMERIC)
    );

    match state.http_client.delete(&telemetry_endpoint).send().await {
        Ok(r) => {
            let status = r.status();
            let bytes = r.bytes().await.unwrap_or_default();

            let mut resp = Response::new(Body::from(bytes));
            *resp.status_mut() = status;
            resp.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
            resp.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_static("*"),
            );
            resp
        }
        Err(e) => {
            error!(error = %e, endpoint = %telemetry_endpoint, "Failed to proxy media delete");
            let mut resp = Response::new(Body::from("Telemetry service unavailable"));
            *resp.status_mut() = StatusCode::BAD_GATEWAY;
            resp.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_static("*"),
            );
            resp
        }
    }
}

async fn media_transcript_proxy_handler(
    State(state): State<Arc<AppState>>,
    Path(filename): Path<String>,
) -> impl IntoResponse {
    let telemetry_endpoint = format!(
        "{}/internal/media/transcript/{}",
        state.telemetry_url.trim_end_matches('/'),
        utf8_percent_encode(&filename, NON_ALPHANUMERIC)
    );

    match state.http_client.get(&telemetry_endpoint).send().await {
        Ok(r) => {
            let status = r.status();
            let bytes = r.bytes().await.unwrap_or_default();

            let mut resp = Response::new(Body::from(bytes));
            *resp.status_mut() = status;
            resp.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
            resp.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_static("*"),
            );
            resp
        }
        Err(e) => {
            error!(error = %e, endpoint = %telemetry_endpoint, "Failed to proxy transcript");
            let mut resp = Response::new(Body::from("Telemetry service unavailable"));
            *resp.status_mut() = StatusCode::BAD_GATEWAY;
            resp.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_static("*"),
            );
            resp
        }
    }
}

async fn media_summary_proxy_handler(
    State(state): State<Arc<AppState>>,
    Path(filename): Path<String>,
) -> impl IntoResponse {
    let telemetry_endpoint = format!(
        "{}/internal/media/summary/{}",
        state.telemetry_url.trim_end_matches('/'),
        utf8_percent_encode(&filename, NON_ALPHANUMERIC)
    );

    match state.http_client.get(&telemetry_endpoint).send().await {
        Ok(r) => {
            let status = r.status();
            let bytes = r.bytes().await.unwrap_or_default();

            let mut resp = Response::new(Body::from(bytes));
            *resp.status_mut() = status;
            resp.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
            resp.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_static("*"),
            );
            resp
        }
        Err(e) => {
            error!(error = %e, endpoint = %telemetry_endpoint, "Failed to proxy summary");
            let mut resp = Response::new(Body::from("Telemetry service unavailable"));
            *resp.status_mut() = StatusCode::BAD_GATEWAY;
            resp.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_static("*"),
            );
            resp
        }
    }
}

// --- Asset Upload Proxy Handler ---
async fn asset_upload_proxy_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Body,
) -> impl IntoResponse {
    let telemetry_endpoint = format!(
        "{}/internal/assets/upload",
        state.telemetry_url.trim_end_matches('/')
    );

    let byte_stream = body.into_data_stream().map(|chunk| {
        chunk.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    });

    let mut req = state
        .http_client
        .post(&telemetry_endpoint)
        .body(reqwest::Body::wrap_stream(byte_stream));

    if let Some(ct) = headers.get(header::CONTENT_TYPE).cloned() {
        req = req.header(header::CONTENT_TYPE, ct);
    }

    match req.send().await {
        Ok(r) => {
            let status = r.status();
            let bytes = r.bytes().await.unwrap_or_default();
            let mut resp = Response::new(Body::from(bytes));
            *resp.status_mut() = status;
            resp.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_static("*"),
            );
            resp
        }
        Err(e) => {
            error!(error = %e, "Failed to proxy asset upload");
            let mut resp = Response::new(Body::from("Telemetry service unavailable"));
            *resp.status_mut() = StatusCode::BAD_GATEWAY;
            resp.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_static("*"),
            );
            resp
        }
    }
}

// --- Asset View Proxy Handler ---
async fn asset_view_proxy_handler(
    State(state): State<Arc<AppState>>,
    Path(filename): Path<String>,
) -> impl IntoResponse {
    let telemetry_endpoint = format!(
        "{}/internal/assets/{}",
        state.telemetry_url.trim_end_matches('/'),
        utf8_percent_encode(&filename, NON_ALPHANUMERIC)
    );

    match state.http_client.get(&telemetry_endpoint).send().await {
        Ok(r) => {
            let status = r.status();
            let headers = r.headers().clone();
            let bytes = r.bytes().await.unwrap_or_default();
            let mut resp = Response::new(Body::from(bytes));
            *resp.status_mut() = status;
            
            if let Some(content_type) = headers.get(header::CONTENT_TYPE) {
                resp.headers_mut().insert(header::CONTENT_TYPE, content_type.clone());
            }
            
            resp.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_static("*"),
            );
            resp
        }
        Err(e) => {
            error!(error = %e, "Failed to proxy asset view");
            let mut resp = Response::new(Body::from("Asset not found"));
            *resp.status_mut() = StatusCode::NOT_FOUND;
            resp.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_static("*"),
            );
            resp
        }
    }
}

// --- Prompt History Proxy Handler ---
async fn prompt_history_proxy_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let orchestrator_endpoint = format!(
        "{}/v1/prompt/history",
        state.orchestrator_url.trim_end_matches('/')
    );

    match state.http_client.get(&orchestrator_endpoint).send().await {
        Ok(r) => {
            let status = r.status();
            let bytes = r.bytes().await.unwrap_or_default();
            let mut resp = Response::new(Body::from(bytes));
            *resp.status_mut() = status;
            resp.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
            resp.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_static("*"),
            );
            resp
        }
        Err(e) => {
            error!(error = %e, endpoint = %orchestrator_endpoint, "Failed to proxy prompt history");
            let mut resp = Response::new(Body::from("Orchestrator service unavailable"));
            *resp.status_mut() = StatusCode::BAD_GATEWAY;
            resp.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_static("*"),
            );
            resp
        }
    }
}

async fn prompt_current_proxy_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let orchestrator_endpoint = format!(
        "{}/v1/prompt/current",
        state.orchestrator_url.trim_end_matches('/')
    );

    match state.http_client.get(&orchestrator_endpoint).send().await {
        Ok(r) => {
            let status = r.status();
            let bytes = r.bytes().await.unwrap_or_default();
            let mut resp = Response::new(Body::from(bytes));
            *resp.status_mut() = status;
            resp.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
            resp.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_static("*"),
            );
            resp
        }
        Err(e) => {
            error!(error = %e, endpoint = %orchestrator_endpoint, "Failed to proxy prompt current");
            let mut resp = Response::new(Body::from("Orchestrator service unavailable"));
            *resp.status_mut() = StatusCode::BAD_GATEWAY;
            resp.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_static("*"),
            );
            resp
        }
    }
}

async fn prompt_update_proxy_handler(
    State(state): State<Arc<AppState>>,
    body: Body,
) -> impl IntoResponse {
    let orchestrator_endpoint = format!(
        "{}/v1/prompt/update",
        state.orchestrator_url.trim_end_matches('/')
    );

    let byte_stream = body.into_data_stream().map(|chunk| {
        chunk.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    });

    let req = state
        .http_client
        .post(&orchestrator_endpoint)
        .header(header::CONTENT_TYPE, "application/json")
        .body(reqwest::Body::wrap_stream(byte_stream));

    match req.send().await {
        Ok(r) => {
            let status = r.status();
            let bytes = r.bytes().await.unwrap_or_default();
            let mut resp = Response::new(Body::from(bytes));
            *resp.status_mut() = status;
            resp.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
            resp.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_static("*"),
            );
            resp
        }
        Err(e) => {
            error!(error = %e, endpoint = %orchestrator_endpoint, "Failed to proxy prompt update");
            let mut resp = Response::new(Body::from("Orchestrator service unavailable"));
            *resp.status_mut() = StatusCode::BAD_GATEWAY;
            resp.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_static("*"),
            );
            resp
        }
    }
}

async fn prompt_reset_proxy_handler(
    State(state): State<Arc<AppState>>,
    body: Body,
) -> impl IntoResponse {
    let orchestrator_endpoint = format!(
        "{}/v1/prompt/reset",
        state.orchestrator_url.trim_end_matches('/')
    );

    // We accept an (unused) body so a client can POST JSON or empty.
    let byte_stream = body.into_data_stream().map(|chunk| {
        chunk.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    });

    let req = state
        .http_client
        .post(&orchestrator_endpoint)
        .header(header::CONTENT_TYPE, "application/json")
        .body(reqwest::Body::wrap_stream(byte_stream));

    match req.send().await {
        Ok(r) => {
            let status = r.status();
            let bytes = r.bytes().await.unwrap_or_default();
            let mut resp = Response::new(Body::from(bytes));
            *resp.status_mut() = status;
            resp.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
            resp.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_static("*"),
            );
            resp
        }
        Err(e) => {
            error!(error = %e, endpoint = %orchestrator_endpoint, "Failed to proxy prompt reset");
            let mut resp = Response::new(Body::from("Orchestrator service unavailable"));
            *resp.status_mut() = StatusCode::BAD_GATEWAY;
            resp.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_static("*"),
            );
            resp
        }
    }
}

// --- Prompt Restore Proxy Handler ---
async fn prompt_restore_proxy_handler(
    State(state): State<Arc<AppState>>,
    body: Body,
) -> impl IntoResponse {
    let orchestrator_endpoint = format!(
        "{}/v1/prompt/restore",
        state.orchestrator_url.trim_end_matches('/')
    );

    let byte_stream = body.into_data_stream().map(|chunk| {
        chunk.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    });

    let mut req = state
        .http_client
        .post(&orchestrator_endpoint)
        .header(header::CONTENT_TYPE, "application/json")
        .body(reqwest::Body::wrap_stream(byte_stream));

    match req.send().await
    {
        Ok(r) => {
            let status = r.status();
            let bytes = r.bytes().await.unwrap_or_default();
            let mut resp = Response::new(Body::from(bytes));
            *resp.status_mut() = status;
            resp.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
            resp.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_static("*"),
            );
            resp
        }
        Err(e) => {
            error!(error = %e, endpoint = %orchestrator_endpoint, "Failed to proxy prompt restore");
            let mut resp = Response::new(Body::from("Orchestrator service unavailable"));
            *resp.status_mut() = StatusCode::BAD_GATEWAY;
            resp.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_static("*"),
            );
            resp
        }
    }
}

// --- System Snapshot Proxy Handler ---
async fn system_snapshot_proxy_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let orchestrator_endpoint = format!(
        "{}/api/system/snapshot",
        state.orchestrator_url.trim_end_matches('/')
    );

    match state.http_client.get(&orchestrator_endpoint).send().await {
        Ok(r) => {
            let status = r.status();
            let bytes = r.bytes().await.unwrap_or_default();

            let mut resp = Response::new(Body::from(bytes));
            *resp.status_mut() = status;
            resp.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
            resp.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_static("*"),
            );
            resp
        }
        Err(e) => {
            error!(error = %e, endpoint = %orchestrator_endpoint, "Failed to proxy system snapshot");
            let mut resp = Response::new(Body::from("Orchestrator service unavailable"));
            *resp.status_mut() = StatusCode::BAD_GATEWAY;
            resp.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_static("*"),
            );
            resp
        }
    }
}

// --- Sync Metrics Proxy Handler ---
async fn system_sync_metrics_proxy_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let orchestrator_endpoint = format!(
        "{}/api/system/sync-metrics",
        state.orchestrator_url.trim_end_matches('/')
    );

    match state.http_client.get(&orchestrator_endpoint).send().await {
        Ok(r) => {
            let status = r.status();
            let bytes = r.bytes().await.unwrap_or_default();

            let mut resp = Response::new(Body::from(bytes));
            *resp.status_mut() = status;
            resp.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
            resp.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_static("*"),
            );
            resp
        }
        Err(e) => {
            error!(error = %e, endpoint = %orchestrator_endpoint, "Failed to proxy sync metrics");
            let mut resp = Response::new(Body::from("Orchestrator service unavailable"));
            *resp.status_mut() = StatusCode::BAD_GATEWAY;
            resp.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_static("*"),
            );
            resp
        }
    }
}

// --- Memory List Proxy Handler ---
async fn memory_list_proxy_handler(
    State(state): State<Arc<AppState>>,
    body: Body,
) -> impl IntoResponse {
    let orchestrator_endpoint = format!(
        "{}/v1/memory/list",
        state.orchestrator_url.trim_end_matches('/')
    );

    let byte_stream = body.into_data_stream().map(|chunk| {
        chunk.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    });

    let mut req = state
        .http_client
        .post(&orchestrator_endpoint)
        .header(header::CONTENT_TYPE, "application/json")
        .body(reqwest::Body::wrap_stream(byte_stream));

    match req.send().await {
        Ok(r) => {
            let status = r.status();
            let bytes = r.bytes().await.unwrap_or_default();
            let mut resp = Response::new(Body::from(bytes));
            *resp.status_mut() = status;
            resp.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
            resp.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_static("*"),
            );
            resp
        }
        Err(e) => {
            error!(error = %e, endpoint = %orchestrator_endpoint, "Failed to proxy memory list");
            let mut resp = Response::new(Body::from("Orchestrator service unavailable"));
            *resp.status_mut() = StatusCode::BAD_GATEWAY;
            resp.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_static("*"),
            );
            resp
        }
    }
}

// --- Memory Delete Proxy Handler ---
async fn memory_delete_proxy_handler(
    State(state): State<Arc<AppState>>,
    body: Body,
) -> impl IntoResponse {
    let orchestrator_endpoint = format!(
        "{}/v1/memory/delete",
        state.orchestrator_url.trim_end_matches('/')
    );

    let byte_stream = body.into_data_stream().map(|chunk| {
        chunk.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    });

    let mut req = state
        .http_client
        .post(&orchestrator_endpoint)
        .header(header::CONTENT_TYPE, "application/json")
        .body(reqwest::Body::wrap_stream(byte_stream));

    match req.send().await {
        Ok(r) => {
            let status = r.status();
            let bytes = r.bytes().await.unwrap_or_default();
            let mut resp = Response::new(Body::from(bytes));
            *resp.status_mut() = status;
            resp.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
            resp.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_static("*"),
            );
            resp
        }
        Err(e) => {
            error!(error = %e, endpoint = %orchestrator_endpoint, "Failed to proxy memory delete");
            let mut resp = Response::new(Body::from("Orchestrator service unavailable"));
            *resp.status_mut() = StatusCode::BAD_GATEWAY;
            resp.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_static("*"),
            );
            resp
        }
    }
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
        signaling_rooms: Mutex::new(HashMap::new()),
    });

    // Create router
    let app = Router::new()
        .route("/api/health", get(health_check))
        .route("/api/system/snapshot", get(system_snapshot_proxy_handler))
        .route("/api/system/sync-metrics", get(system_sync_metrics_proxy_handler))
        .route("/ws/chat/:user_id", get(ws_handler))
        .route("/ws/signaling/:room_id", get(signaling_ws_handler))
        .route("/v1/telemetry/stream", get(telemetry_proxy_handler))
        .route("/api/media/upload", post(media_upload_proxy_handler).options(media_upload_options))
        .route("/api/media/list", get(media_list_proxy_handler))
        .route("/api/media/view/:filename", get(media_view_proxy_handler))
        .route("/api/media/delete/:filename", delete(media_delete_proxy_handler))
        .route("/api/media/transcript/:filename", get(media_transcript_proxy_handler))
        .route("/api/media/summary/:filename", get(media_summary_proxy_handler))
        .route("/api/assets/upload", post(asset_upload_proxy_handler))
        .route("/api/assets/:filename", get(asset_view_proxy_handler))
        .route("/api/prompt/current", get(prompt_current_proxy_handler))
        .route("/api/prompt/history", get(prompt_history_proxy_handler))
        .route("/api/prompt/update", post(prompt_update_proxy_handler))
        .route("/api/prompt/restore", post(prompt_restore_proxy_handler))
        .route("/api/prompt/reset", post(prompt_reset_proxy_handler))
        .route("/api/memory/list", post(memory_list_proxy_handler))
        .route("/api/memory/delete", post(memory_delete_proxy_handler))
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
