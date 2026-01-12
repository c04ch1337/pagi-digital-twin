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
mod llm_client;

// Import actual Agent, Memory, and Protocol types from the core crate
use pagi_digital_twin_core::{
    agent::DigitalTwinAgent,
    memory::{MemorySystem, DebugMemorySystem},
    agent::{ExternalLLM, ChatRequest, ChatResponse},
};

// Use the local LLM client implementation
use llm_client::AxumLLMClient;

// --- API Shared State ---
// Stores the active, long-running Digital Twin Agent instance for each user/session.
// The LLM client and Memory System are shared across all agents for efficient resource usage.
pub struct ApiState {
    active_agents: Mutex<HashMap<String, Arc<DigitalTwinAgent>>>,
    llm_client: Arc<dyn ExternalLLM>,
    memory_system: Arc<dyn MemorySystem>,
}

impl ApiState {
    /// Creates a new ApiState with the provided LLM client and Memory System.
    /// Both are shared across all agent instances.
    pub fn new(llm_client: Arc<dyn ExternalLLM>, memory_system: Arc<dyn MemorySystem>) -> Self {
        Self {
            active_agents: Mutex::new(HashMap::new()),
            llm_client,
            memory_system,
        }
    }

    /// Gets or creates a DigitalTwinAgent for the given user and session.
    /// Each agent instance shares the LLM client and Memory System.
    pub async fn get_or_create_agent(
        &self,
        user_id: &str,
        session_id: &str,
    ) -> Result<Arc<DigitalTwinAgent>> {
        let mut map = self.active_agents.lock().await;
        
        // Use a composite key for user_id + session_id to support multiple sessions per user
        let agent_key = format!("{}:{}", user_id, session_id);
        
        if let Some(agent) = map.get(&agent_key) {
            return Ok(agent.clone());
        }
        
        // --- Critical Wiring Point ---
        // This is where the real core logic is instantiated with the LLM client and Memory System.
        let new_agent = Arc::new(
            DigitalTwinAgent::new(
                user_id.to_string(),
                self.llm_client.clone(),
                self.memory_system.clone(),
            )
        );
        map.insert(agent_key, new_agent.clone());
        info!(
            user_id = user_id,
            session_id = session_id,
            "New Digital Twin session created"
        );
        Ok(new_agent)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_target(false).init();

    // --- LLM Service Configuration ---
    // Environment variable to source the LLM endpoint (Bare Metal Compliance)
    let llm_url = std::env::var("LLM_SERVICE_URL").unwrap_or_else(|_| {
        let default_url = "http://127.0.0.1:8000/llm/inference".to_string();
        info!("LLM_SERVICE_URL not set. Using default: {}", default_url);
        default_url
    });

    info!(llm_url = %llm_url, "Initializing LLM client");

    // Create the LLM client that will be shared across all agent instances
    let llm_client: Arc<dyn ExternalLLM> = Arc::new(AxumLLMClient::new(llm_url));
    info!("LLM Client initialized.");

    // Initialize the shared Memory System
    let memory_system = Arc::new(DebugMemorySystem::new()) as Arc<dyn MemorySystem>;
    info!("Memory System (DebugMemorySystem) initialized.");

    // Create API state with the shared LLM client and Memory System
    let state = Arc::new(ApiState::new(llm_client, memory_system));

    let app = Router::new()
        .route("/api/health", get(health_check))
        .route("/ws/chat/:user_id", get(ws_handler))
        .with_state(state);

    let bind_addr = std::env::var("PAGI_CHAT_BACKEND_BIND")
        .unwrap_or_else(|_| "127.0.0.1:8181".to_string());
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    info!("🚀 PAGI Chat Desktop Backend running on http://{}", bind_addr);

    axum::serve(listener, app.into_make_service()).await?;
    Ok(())
}

async fn health_check() -> impl IntoResponse {
    (axum::http::StatusCode::OK, "PAGI Chat Backend Operational")
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    Path(user_id): Path<String>,
    State(state): State<Arc<ApiState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, user_id, state))
}

async fn handle_socket(mut socket: WebSocket, user_id: String, state: Arc<ApiState>) {
    info!(user_id = %user_id, "WebSocket connection established");

    // Note: We'll get the session_id from the first ChatRequest
    // For now, we'll use a default session_id and update it when we receive the first message
    let mut current_session_id: Option<String> = None;

    while let Some(Ok(msg)) = socket.recv().await {
        match msg {
            Message::Text(text) => {
                let req: ChatRequest = match serde_json::from_str(&text) {
                    Ok(r) => r,
                    Err(e) => {
                        error!(user_id = %user_id, error = %e, "Invalid ChatRequest JSON");
                        let resp = ChatResponse::StatusUpdate {
                            status: "error".to_string(),
                            details: Some(format!("Invalid JSON payload: {}", e)),
                        };
                        let _ = socket
                            .send(Message::Text(serde_json::to_string(&resp).unwrap_or_else(|_| {
                                "{\"type\":\"status_update\",\"status\":\"error\",\"details\":\"serialization_error\"}"
                                    .to_string()
                            })))
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
                        status: "error".to_string(),
                        details: Some("User ID mismatch between path and request payload.".to_string()),
                    };
                    let _ = socket
                        .send(Message::Text(
                            serde_json::to_string(&resp).unwrap_or_else(|_| {
                                "{\"type\":\"status_update\",\"status\":\"error\"}"
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

                // Update current session_id if this is the first message or session changed
                let session_id_str = req.session_id.to_string();
                if current_session_id.as_ref() != Some(&session_id_str) {
                    current_session_id = Some(session_id_str.clone());
                }

                // Get or create agent for this user + session combination
                let agent = match state
                    .get_or_create_agent(&user_id, &session_id_str)
                    .await
                {
                    Ok(a) => a,
                    Err(e) => {
                        error!(user_id = %user_id, error = %e, "Failed to initialize DigitalTwinAgent");
                        let resp = ChatResponse::StatusUpdate {
                            status: "error".to_string(),
                            details: Some(format!("Failed to initialize DigitalTwinAgent: {}", e)),
                        };
                        let _ = socket
                            .send(Message::Text(serde_json::to_string(&resp).unwrap_or_else(|_| {
                                "{\"type\":\"status_update\",\"status\":\"error\"}"
                                    .to_string()
                            })))
                            .await;
                        continue;
                    }
                };

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
                                    status: "error".to_string(),
                                    details: Some(format!("Failed to serialize response: {}", e)),
                                };
                                let _ = socket
                                    .send(Message::Text(
                                        serde_json::to_string(&resp).unwrap_or_else(|_| {
                                            "{\"type\":\"status_update\",\"status\":\"error\"}"
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
                            status: "error".to_string(),
                            details: Some(format!("Agent failed to process input: {}", e)),
                        };
                        let _ = socket.send(Message::Text(serde_json::to_string(&resp).unwrap_or_else(|_| {
                            "{\"type\":\"status_update\",\"status\":\"error\"}"
                                .to_string()
                        }))).await;
                    }
                    Err(e) => {
                        error!(user_id = %user_id, error = %e, "Agent processing task panicked");
                        let resp = ChatResponse::StatusUpdate {
                            status: "error".to_string(),
                            details: Some("Fatal: Agent task failed.".to_string()),
                        };
                        let _ = socket.send(Message::Text(serde_json::to_string(&resp).unwrap_or_else(|_| {
                            "{\"type\":\"status_update\",\"status\":\"error\"}"
                                .to_string()
                        }))).await;
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

