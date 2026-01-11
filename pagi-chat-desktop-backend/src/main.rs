use anyhow::Result;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use tracing::{error, info};

mod protocol;

use protocol::{ChatRequest, ChatResponse};

// === Core Agent Integration ===
//
// This crate is intended to wrap the real Digital Twin core crate when available.
// In this workspace the core package currently lives at:
//   ../pagi-companion-core
// and is aliased (in Cargo.toml) as `pagi-digital-twin-core`.
//
// For now (and for this repo to remain buildable), we provide a mock fallback.

#[cfg(feature = "with-core")]
mod core {
    // TODO: replace with real imports when the Digital Twin core crate is present.
    // Example (expected shape; adjust to real core API):
    // use pagi_digital_twin_core::agent::DigitalTwinAgent;

    use super::protocol::{ChatRequest, ChatResponse};
    use uuid::Uuid;

    // Temporary alias to keep the scaffold compiling while the real types land.
    #[derive(Debug, Clone)]
    pub struct DigitalTwinAgent;

    impl DigitalTwinAgent {
        pub async fn new(user_id: &str) -> anyhow::Result<Self> {
            tracing::info!(user_id = user_id, "DigitalTwinAgent initialized (core stub)");
            Ok(Self)
        }

        pub async fn process_user_input(&self, req: ChatRequest) -> anyhow::Result<ChatResponse> {
            let started = std::time::Instant::now();
            tracing::info!(
                user_id = %req.user_id,
                session_id = %req.session_id,
                input = %req.message,
                "[CORE STUB] Processing input"
            );

            Ok(ChatResponse::CompleteMessage {
                id: Uuid::new_v4(),
                content: format!("ACK(core-stub): {}", req.message),
                is_final: true,
                latency_ms: started.elapsed().as_millis() as u64,
                source_memories: vec![],
                issued_command: None,
            })
        }
    }
}

#[cfg(not(feature = "with-core"))]
mod core {
    use super::protocol::{ChatRequest, ChatResponse};
    use uuid::Uuid;

    #[derive(Debug, Clone)]
    pub struct DigitalTwinAgent;

    impl DigitalTwinAgent {
        pub async fn new(user_id: &str) -> anyhow::Result<Self> {
            tracing::info!(user_id = user_id, "DigitalTwinAgent initialized (mock)");
            Ok(Self)
        }

        pub async fn process_user_input(&self, req: ChatRequest) -> anyhow::Result<ChatResponse> {
            let started = std::time::Instant::now();
            tracing::info!(
                user_id = %req.user_id,
                session_id = %req.session_id,
                input = %req.message,
                "[CORE MOCK] Processing input"
            );
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;

            Ok(ChatResponse::CompleteMessage {
                id: Uuid::new_v4(),
                content: format!(
                    "ACK(mock): Agent processed '{}'. State updated.",
                    req.message
                ),
                is_final: true,
                latency_ms: started.elapsed().as_millis() as u64,
                source_memories: vec![],
                issued_command: None,
            })
        }
    }
}

use core::DigitalTwinAgent;

// --- API Shared State ---
// Stores the active, long-running Digital Twin Agent instance for each user/session.
pub struct ApiState {
    active_agents: Mutex<HashMap<String, Arc<DigitalTwinAgent>>>,
}

impl ApiState {
    pub async fn get_or_create_agent(&self, user_id: &str) -> Result<Arc<DigitalTwinAgent>> {
        let mut map = self.active_agents.lock().await;
        if let Some(agent) = map.get(user_id) {
            return Ok(agent.clone());
        }
        
        // --- Critical Integration Point ---
        // This is where the core logic is instantiated.
        let new_agent = Arc::new(DigitalTwinAgent::new(user_id).await?);
        map.insert(user_id.to_string(), new_agent.clone());
        info!(user_id = user_id, "New Digital Twin session created");
        Ok(new_agent)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_target(false).init();

    let state = Arc::new(ApiState {
        active_agents: Mutex::new(HashMap::new()),
    });

    let app = Router::new()
        .route("/api/health", get(health_check))
        .route("/ws/chat/:user_id", get(ws_handler))
        .with_state(state);

    let bind_addr = std::env::var("PAGI_CHAT_BACKEND_BIND").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    info!(addr = %bind_addr, "PAGI Chat Desktop Backend listening");

    axum::serve(listener, app.into_make_service()).await?;
    Ok(())
}

async fn health_check() -> impl IntoResponse {
    (axum::http::StatusCode::OK, "PAGI Chat Desktop Backend Operational")
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    Path(user_id): Path<String>,
    State(state): State<Arc<ApiState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, user_id, state))
}

async fn handle_socket(mut socket: WebSocket, user_id: String, state: Arc<ApiState>) {
    info!(user_id = %user_id, "WebSocket connected");

    let agent = match state.get_or_create_agent(&user_id).await {
        Ok(a) => a,
        Err(e) => {
            error!(user_id = %user_id, error = %e, "Failed to init agent");
            let _ = socket
                .send(Message::Text("Error initializing agent.".to_string()))
                .await;
            return;
        }
    };

    while let Some(Ok(msg)) = socket.recv().await {
        match msg {
            Message::Text(text) => {
                let req: ChatRequest = match serde_json::from_str(&text) {
                    Ok(r) => r,
                    Err(e) => {
                        error!(user_id = %user_id, error = %e, "Invalid ChatRequest JSON");
                        let resp = ChatResponse::StatusUpdate {
                            status: "invalid_request".to_string(),
                            details: Some(e.to_string()),
                        };
                        let _ = socket
                            .send(Message::Text(serde_json::to_string(&resp).unwrap_or_else(|_| "{\"type\":\"status_update\",\"status\":\"serialization_error\"}".to_string())))
                            .await;
                        continue;
                    }
                };

                if req.user_id != user_id {
                    error!(
                        path_user_id = %user_id,
                        body_user_id = %req.user_id,
                        "user_id mismatch between path and payload"
                    );
                    let resp = ChatResponse::StatusUpdate {
                        status: "user_id_mismatch".to_string(),
                        details: Some("user_id in payload must match /ws/chat/:user_id".to_string()),
                    };
                    let _ = socket
                        .send(Message::Text(
                            serde_json::to_string(&resp).unwrap_or_else(|_| {
                                "{\"type\":\"status_update\",\"status\":\"serialization_error\"}"
                                    .to_string()
                            }),
                        ))
                        .await;
                    continue;
                }

                info!(
                    user_id = %user_id,
                    session_id = %req.session_id,
                    msg = %req.message,
                    "User message"
                );

                let agent_clone = agent.clone();
                let user_id_clone = user_id.clone();

                // Run the long-running core logic without blocking the WS task.
                let response_handle = tokio::spawn(async move {
                    let _ = user_id_clone; // reserved for future cross-checks / tracing fields
                    agent_clone.process_user_input(req).await
                });

                match response_handle.await {
                    Ok(Ok(response)) => {
                        match serde_json::to_string(&response) {
                            Ok(json) => {
                                let _ = socket.send(Message::Text(json)).await;
                            }
                            Err(e) => {
                                error!(user_id = %user_id, error = %e, "Failed to serialize ChatResponse");
                                let resp = ChatResponse::StatusUpdate {
                                    status: "serialization_error".to_string(),
                                    details: Some(e.to_string()),
                                };
                                let _ = socket
                                    .send(Message::Text(
                                        serde_json::to_string(&resp).unwrap_or_else(|_| {
                                            "{\"type\":\"status_update\",\"status\":\"serialization_error\"}"
                                                .to_string()
                                        }),
                                    ))
                                    .await;
                            }
                        }
                    }
                    Ok(Err(e)) => {
                        error!(user_id = %user_id, error = %e, "Agent processing failed");
                        let resp = ChatResponse::StatusUpdate {
                            status: "agent_error".to_string(),
                            details: Some(e.to_string()),
                        };
                        let _ = socket.send(Message::Text(serde_json::to_string(&resp).unwrap_or_else(|_| "{\"type\":\"status_update\",\"status\":\"agent_error\"}".to_string()))).await;
                    }
                    Err(e) => {
                        error!(user_id = %user_id, error = %e, "Agent task panicked/aborted");
                        let resp = ChatResponse::StatusUpdate {
                            status: "agent_task_failed".to_string(),
                            details: Some(e.to_string()),
                        };
                        let _ = socket.send(Message::Text(serde_json::to_string(&resp).unwrap_or_else(|_| "{\"type\":\"status_update\",\"status\":\"agent_task_failed\"}".to_string()))).await;
                    }
                }
            }
            Message::Close(_) => {
                info!(user_id = %user_id, "WebSocket closed by client");
                break;
            }
            _ => {}
        }
    }
}

