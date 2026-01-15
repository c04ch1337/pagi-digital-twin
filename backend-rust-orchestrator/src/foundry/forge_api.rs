use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response, sse::{Event, Sse}},
    Json,
};
use futures::stream::{self, Stream};
use git2::{Commit, DiffOptions, Repository};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

use super::compliance_monitor::ComplianceMonitor;

#[derive(Clone)]
pub struct ForgeState {
    pub agents_repo_path: PathBuf,
    pub tools_repo_path: PathBuf,
    pub compliance_monitor: Arc<ComplianceMonitor>,
}

#[derive(Serialize, Deserialize)]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub description: String,
    pub prompt: String,
    pub tools: Vec<String>,
    pub status: String,
    pub version: String,
    pub last_modified: String,
    pub modified_by: String,
}

#[derive(Serialize, Deserialize)]
pub struct Tool {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub input_schema: serde_json::Value,
    pub script: String,
    pub status: String,
    pub dependent_agents: Vec<String>,
    pub breaking_changes: bool,
}

#[derive(Serialize)]
pub struct CommitHistory {
    pub hash: String,
    pub author: String,
    pub timestamp: String,
    pub message: String,
    pub files: Vec<String>,
    pub is_active: bool,
}

#[derive(Serialize)]
pub struct DiffResponse {
    pub diff: String,
}

#[derive(Deserialize)]
pub struct RevertRequest {
    pub commit_hash: String,
}

#[derive(Deserialize)]
pub struct TestMissionRequest {
    pub mission: String,
    pub enable_compliance: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceStep {
    pub r#type: String,
    pub content: String,
    pub timestamp: String,
    pub tool: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceResult {
    pub privacy: ComplianceCheck,
    pub efficiency: ComplianceCheck,
    pub tone: ComplianceCheck,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceCheck {
    pub passed: bool,
    pub details: String,
}

// GET /api/agents - List all agents
pub async fn list_agents(State(state): State<Arc<RwLock<ForgeState>>>) -> impl IntoResponse {
    let state = state.read().await;
    
    // TODO: Read from agents repository
    let agents = vec![
        Agent {
            id: "network-scanner".to_string(),
            name: "Network Scanner".to_string(),
            description: "Scans network for devices and vulnerabilities".to_string(),
            prompt: "You are a network security expert...".to_string(),
            tools: vec!["nmap".to_string(), "ping".to_string()],
            status: "active".to_string(),
            version: "1.2.0".to_string(),
            last_modified: "2026-01-14T00:00:00Z".to_string(),
            modified_by: "admin".to_string(),
        },
    ];
    
    Json(agents)
}

// GET /api/agents/:id/history - Get commit history for an agent
pub async fn get_agent_history(
    Path(agent_id): Path<String>,
    State(state): State<Arc<RwLock<ForgeState>>>,
) -> Result<Json<Vec<CommitHistory>>, StatusCode> {
    let state = state.read().await;
    
    let repo = Repository::open(&state.agents_repo_path)
        .map_err(|e| {
            error!("Failed to open agents repository: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    
    let mut revwalk = repo.revwalk()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    revwalk.push_head()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let mut history = Vec::new();
    let head_commit = repo.head()
        .ok()
        .and_then(|h| h.peel_to_commit().ok())
        .map(|c| c.id().to_string());
    
    for oid in revwalk {
        let oid = oid.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let commit = repo.find_commit(oid)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        
        // Filter commits that affect this agent
        let agent_path = format!("agents/{}/", agent_id);
        let tree = commit.tree()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        
        let mut files = Vec::new();
        tree.walk(git2::TreeWalkMode::PreOrder, |root, entry| {
            if let Some(name) = entry.name() {
                let path = format!("{}{}", root, name);
                if path.starts_with(&agent_path) {
                    files.push(path);
                }
            }
            git2::TreeWalkResult::Ok
        }).ok();
        
        if !files.is_empty() {
            history.push(CommitHistory {
                hash: oid.to_string(),
                author: commit.author().name().unwrap_or("Unknown").to_string(),
                timestamp: chrono::DateTime::from_timestamp(commit.time().seconds(), 0)
                    .unwrap_or_default()
                    .to_rfc3339(),
                message: commit.message().unwrap_or("No message").to_string(),
                files,
                is_active: head_commit.as_ref() == Some(&oid.to_string()),
            });
        }
    }
    
    Ok(Json(history))
}

// GET /api/agents/:id/diff/:commit - Get diff for a specific commit
pub async fn get_agent_diff(
    Path((agent_id, commit_hash)): Path<(String, String)>,
    State(state): State<Arc<RwLock<ForgeState>>>,
) -> Result<Json<DiffResponse>, StatusCode> {
    let state = state.read().await;
    
    let repo = Repository::open(&state.agents_repo_path)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let oid = git2::Oid::from_str(&commit_hash)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    
    let commit = repo.find_commit(oid)
        .map_err(|_| StatusCode::NOT_FOUND)?;
    
    let tree = commit.tree()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let parent_tree = commit.parent(0)
        .ok()
        .and_then(|p| p.tree().ok());
    
    let mut diff_opts = DiffOptions::new();
    diff_opts.pathspec(format!("agents/{}/", agent_id));
    
    let diff = repo.diff_tree_to_tree(
        parent_tree.as_ref(),
        Some(&tree),
        Some(&mut diff_opts),
    ).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let mut diff_text = String::new();
    diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
        diff_text.push_str(&String::from_utf8_lossy(line.content()));
        true
    }).ok();
    
    Ok(Json(DiffResponse { diff: diff_text }))
}

// POST /api/agents/:id/revert - Revert agent to a specific commit
pub async fn revert_agent(
    Path(agent_id): Path<String>,
    State(state): State<Arc<RwLock<ForgeState>>>,
    Json(payload): Json<RevertRequest>,
) -> Result<StatusCode, StatusCode> {
    let state = state.read().await;
    
    let repo = Repository::open(&state.agents_repo_path)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let oid = git2::Oid::from_str(&payload.commit_hash)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    
    let commit = repo.find_commit(oid)
        .map_err(|_| StatusCode::NOT_FOUND)?;
    
    // Checkout the specific files from the commit
    let tree = commit.tree()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let mut checkout_builder = git2::build::CheckoutBuilder::new();
    checkout_builder.path(format!("agents/{}/", agent_id));
    checkout_builder.force();
    
    repo.checkout_tree(tree.as_object(), Some(&mut checkout_builder))
        .map_err(|e| {
            error!("Failed to checkout tree: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    
    // Create a new commit
    let signature = repo.signature()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let parent_commit = repo.head()
        .ok()
        .and_then(|h| h.peel_to_commit().ok())
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let tree_id = repo.index()
        .ok()
        .and_then(|mut idx| {
            idx.add_path(&PathBuf::from(format!("agents/{}/", agent_id))).ok()?;
            idx.write_tree().ok()
        })
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let new_tree = repo.find_tree(tree_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        &format!("Revert agent {} to commit {}", agent_id, payload.commit_hash),
        &new_tree,
        &[&parent_commit],
    ).map_err(|e| {
        error!("Failed to create revert commit: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    
    info!("Reverted agent {} to commit {}", agent_id, payload.commit_hash);
    
    Ok(StatusCode::OK)
}

// POST /api/agents/:id/test - Test agent with a mission
pub async fn test_agent(
    Path(agent_id): Path<String>,
    State(state): State<Arc<RwLock<ForgeState>>>,
    Json(payload): Json<TestMissionRequest>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    // Simulate agent execution with trace steps
    let trace_steps = vec![
        TraceStep {
            r#type: "thought".to_string(),
            content: format!("Analyzing mission: {}", payload.mission),
            timestamp: chrono::Utc::now().to_rfc3339(),
            tool: None,
        },
        TraceStep {
            r#type: "action".to_string(),
            content: "Executing network scan...".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            tool: Some("nmap".to_string()),
        },
        TraceStep {
            r#type: "observation".to_string(),
            content: "Found 5 devices on the network".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            tool: None,
        },
    ];
    
    let compliance = if payload.enable_compliance {
        Some(ComplianceResult {
            privacy: ComplianceCheck {
                passed: true,
                details: "No sensitive data accessed".to_string(),
            },
            efficiency: ComplianceCheck {
                passed: true,
                details: "Used optimal tool sequence".to_string(),
            },
            tone: ComplianceCheck {
                passed: true,
                details: "Maintained professional tone".to_string(),
            },
        })
    } else {
        None
    };
    
    // Record compliance test and check for auto-rollback
    let rollback_commit = if let Some(ref comp) = compliance {
        let monitor = state.read().await.compliance_monitor.clone();
        match monitor.record_test(agent_id.clone(), payload.mission.clone(), comp.clone()).await {
            Ok(Some(commit_hash)) => {
                info!(
                    agent_id = %agent_id,
                    commit_hash = %commit_hash,
                    "Auto-rollback triggered"
                );
                Some(commit_hash)
            }
            Ok(None) => None,
            Err(e) => {
                error!(
                    agent_id = %agent_id,
                    error = %e,
                    "Failed to record compliance test"
                );
                None
            }
        }
    } else {
        None
    };
    
    let stream = stream::iter(trace_steps.into_iter().map(|step| {
        Ok(Event::default()
            .json_data(serde_json::json!({
                "type": "trace",
                "step": step
            }))
            .unwrap())
    }).chain(compliance.into_iter().map(|comp| {
        Ok(Event::default()
            .json_data(serde_json::json!({
                "type": "compliance",
                "result": comp
            }))
            .unwrap())
    })).chain(rollback_commit.into_iter().map(|commit_hash| {
        Ok(Event::default()
            .json_data(serde_json::json!({
                "type": "rollback",
                "commit_hash": commit_hash,
                "message": "Agent automatically rolled back due to compliance failure"
            }))
            .unwrap())
    })));
    
    Sse::new(stream)
}

// POST /api/agents/discovery-refresh - Trigger agent discovery refresh
pub async fn discovery_refresh() -> StatusCode {
    info!("Triggering agent discovery refresh");
    // TODO: Implement actual discovery refresh logic
    StatusCode::OK
}

// GET /api/tools - List all tools
pub async fn list_tools(State(state): State<Arc<RwLock<ForgeState>>>) -> impl IntoResponse {
    let state = state.read().await;
    
    // TODO: Read from tools repository
    let tools = vec![
        Tool {
            id: "nmap".to_string(),
            name: "Nmap Scanner".to_string(),
            description: "Network mapping and port scanning tool".to_string(),
            version: "1.0.0".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "target": { "type": "string" },
                    "ports": { "type": "string" }
                }
            }),
            script: "#!/bin/bash\nnmap $1".to_string(),
            status: "active".to_string(),
            dependent_agents: vec!["network-scanner".to_string()],
            breaking_changes: false,
        },
    ];
    
    Json(tools)
}

// PUT /api/tools/:id - Update a tool
pub async fn update_tool(
    Path(tool_id): Path<String>,
    State(state): State<Arc<RwLock<ForgeState>>>,
    Json(tool): Json<Tool>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let state = state.read().await;
    
    // TODO: Detect breaking changes by comparing input schemas
    let breaking_changes = false; // Placeholder
    let affected_agents = if breaking_changes {
        tool.dependent_agents.clone()
    } else {
        vec![]
    };
    
    info!("Updated tool {} to version {}", tool_id, tool.version);
    
    Ok(Json(serde_json::json!({
        "success": true,
        "breakingChanges": breaking_changes,
        "affectedAgents": affected_agents
    })))
}

// POST /api/tools/:id/mark-legacy - Mark tool as legacy
pub async fn mark_tool_legacy(
    Path(tool_id): Path<String>,
    State(state): State<Arc<RwLock<ForgeState>>>,
) -> StatusCode {
    info!("Marked tool {} as legacy", tool_id);
    StatusCode::OK
}

// GET /api/compliance/config - Get auto-rollback configuration
pub async fn get_compliance_config(
    State(state): State<Arc<RwLock<ForgeState>>>,
) -> impl IntoResponse {
    let state = state.read().await;
    let config = state.compliance_monitor.get_config().await;
    Json(config)
}

// PUT /api/compliance/config - Update auto-rollback configuration
pub async fn update_compliance_config(
    State(state): State<Arc<RwLock<ForgeState>>>,
    Json(config): Json<super::compliance_monitor::AutoRollbackConfig>,
) -> impl IntoResponse {
    let state = state.read().await;
    state.compliance_monitor.update_config(config).await;
    StatusCode::OK
}

// GET /api/agents/:id/compliance - Get compliance history for an agent
pub async fn get_agent_compliance(
    Path(agent_id): Path<String>,
    State(state): State<Arc<RwLock<ForgeState>>>,
) -> impl IntoResponse {
    let state = state.read().await;
    let history = state.compliance_monitor.get_agent_history(&agent_id).await;
    Json(history)
}

// GET /api/agents/:id/compliance/stats - Get compliance statistics for an agent
pub async fn get_agent_compliance_stats(
    Path(agent_id): Path<String>,
    State(state): State<Arc<RwLock<ForgeState>>>,
) -> impl IntoResponse {
    let state = state.read().await;
    match state.compliance_monitor.get_agent_stats(&agent_id).await {
        Some(stats) => Json(serde_json::json!({ "stats": stats })),
        None => Json(serde_json::json!({ "error": "No compliance data found" })),
    }
}
