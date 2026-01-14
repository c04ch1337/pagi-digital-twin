//! Playbook Search Routes
//!
//! This module provides file search functionality for the GitHub playbook repository.
//! Uses the in-memory index from the PlaybookIndexer for blazingly fast searches.

use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{error, info, warn};

use crate::services::playbook_indexer::PlaybookIndex;

/// App state for playbook routes
#[derive(Clone)]
pub struct PlaybookAppState {
    pub agents_repo_path: PathBuf,
    pub index: Arc<PlaybookIndex>,
}

/// Playbook search query parameters
#[derive(Debug, Deserialize)]
pub struct PlaybookSearchQuery {
    pub q: String,
}

/// Playbook search result
#[derive(Debug, Serialize)]
pub struct PlaybookSearchResult {
    pub id: String,
    pub name: String,
    pub content: String,
    pub path: String,
    pub line_number: Option<u32>,
    pub snippet: String,
}

/// Playbook search response
#[derive(Debug, Serialize)]
pub struct PlaybookSearchResponse {
    pub playbooks: Vec<PlaybookSearchResult>,
    pub total: usize,
}

/// Search playbooks in the repository using the in-memory index
pub async fn search_playbooks(
    State(state): State<PlaybookAppState>,
    Query(params): Query<PlaybookSearchQuery>,
) -> Result<Json<PlaybookSearchResponse>, StatusCode> {
    let query = params.q.trim();
    if query.is_empty() {
        return Ok(Json(PlaybookSearchResponse {
            playbooks: Vec::new(),
            total: 0,
        }));
    }

    let repo_path = &state.agents_repo_path;
    
    // Use the in-memory index for fast search
    let search_results = state.index.search_with_lines(query).await;
    
    let mut results = Vec::new();
    
    for (file, matching_lines) in search_results {
        for (line_num, line_content) in matching_lines {
            let snippet = truncate_snippet(&line_content, 100);
            
            let file_name = file.path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            let relative_path = file.path
                .strip_prefix(repo_path)
                .unwrap_or(&file.path)
                .to_string_lossy()
                .to_string();

            results.push(PlaybookSearchResult {
                id: format!("{}-{}", relative_path, line_num),
                name: file_name,
                content: file.content.clone(),
                path: relative_path,
                line_number: Some(line_num),
                snippet,
            });
        }
    }

    // Limit to top 20 results
    results.truncate(20);

    Ok(Json(PlaybookSearchResponse {
        total: results.len(),
        playbooks: results,
    }))
}


/// Truncate text to specified length with ellipsis
fn truncate_snippet(text: &str, max_length: usize) -> String {
    if text.len() <= max_length {
        return text.to_string();
    }
    
    // Try to truncate at word boundary
    let truncated = &text[..max_length.min(text.len())];
    if let Some(last_space) = truncated.rfind(' ') {
        format!("{}...", &truncated[..last_space])
    } else {
        format!("{}...", truncated)
    }
}

/// Create playbook API router
pub fn create_playbook_router(state: PlaybookAppState) -> Router {
    Router::new()
        .route("/api/playbooks/search", get(search_playbooks))
        .with_state(state)
}
