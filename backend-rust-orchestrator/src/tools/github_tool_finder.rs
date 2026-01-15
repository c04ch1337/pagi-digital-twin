use reqwest;
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::Arc;
use tracing::{info, warn};
use qdrant_client::Qdrant;

/// GitHub API search result for code
#[derive(Debug, Serialize, Deserialize)]
struct GitHubCodeSearchItem {
    name: String,
    path: String,
    sha: String,
    url: String,
    git_url: String,
    html_url: String,
    repository: GitHubRepository,
    score: Option<f64>,
    #[serde(default)]
    text_matches: Vec<TextMatch>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GitHubRepository {
    id: u64,
    name: String,
    full_name: String,
    owner: GitHubOwner,
    html_url: String,
    description: Option<String>,
    language: Option<String>,
    stargazers_count: u32,
    forks_count: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct GitHubOwner {
    login: String,
    #[serde(rename = "type")]
    owner_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct TextMatch {
    fragment: String,
    matches: Vec<Match>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Match {
    text: String,
    indices: Vec<usize>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GitHubCodeSearchResponse {
    total_count: u32,
    incomplete_results: bool,
    items: Vec<GitHubCodeSearchItem>,
}

/// Tool discovery result
#[derive(Debug, Serialize, Deserialize)]
pub struct ToolDiscoveryResult {
    pub tool_name: String,
    pub repository: String,
    pub file_path: String,
    pub language: Option<String>,
    pub description: String,
    pub code_snippet: String,
    pub github_url: String,
    pub raw_url: String,
    pub stars: u32,
    pub relevance_score: f64,
}

/// Search GitHub for tools/scripts matching a query
/// First checks the global playbook store for verified tools before searching GitHub
///
/// # Arguments
/// * `query` - Search query describing the tool/functionality needed
/// * `language` - Optional programming language filter (e.g., "python", "rust", "bash")
/// * `max_results` - Maximum number of results to return (default: 5)
/// * `qdrant_client` - Optional Qdrant client for playbook search
///
/// # Returns
/// * `Ok(Vec<ToolDiscoveryResult>)` - List of discovered tools (playbooks first, then GitHub)
/// * `Err(String)` - Error message if search fails
pub async fn find_github_tool(
    query: String,
    language: Option<String>,
    max_results: Option<usize>,
    qdrant_client: Option<Arc<Qdrant>>,
) -> Result<Vec<ToolDiscoveryResult>, String> {
    info!(query = %query, language = ?language, "Searching for tools (playbooks first, then GitHub)");
    
    let max_results = max_results.unwrap_or(5);
    let mut results = Vec::new();
    
    // First, check global playbooks if Qdrant client is available
    if let Some(qdrant) = qdrant_client {
        match crate::tools::playbook_store::search_playbooks_by_tool(qdrant, &query, Some(max_results)).await {
            Ok(playbook_results) => {
                if !playbook_results.is_empty() {
                    info!(
                        playbook_count = playbook_results.len(),
                        "Found verified playbooks matching query"
                    );
                    
                    // Convert playbooks to ToolDiscoveryResult format
                    for playbook_result in playbook_results {
                        let playbook = playbook_result.playbook;
                        
                        // Only include high-reliability playbooks (>= 0.7) or if explicitly requested
                        if playbook.reliability_score >= 0.7 {
                            let tool_result = ToolDiscoveryResult {
                                tool_name: playbook.tool_name.clone(),
                                repository: playbook.repository.unwrap_or_else(|| "internal_playbook".to_string()),
                                file_path: "verified_playbook".to_string(),
                                language: playbook.language.clone(),
                                description: playbook.description.unwrap_or_else(|| format!("Verified playbook with {}% success rate", (playbook.reliability_score * 100.0) as u32)),
                                code_snippet: format!("Installation: {}\nType: {}\nReliability: {:.1}%", 
                                    playbook.installation_command, 
                                    playbook.installation_type,
                                    playbook.reliability_score * 100.0),
                                github_url: playbook.github_url.unwrap_or_else(|| "internal://playbook".to_string()),
                                raw_url: "internal://playbook".to_string(),
                                stars: (playbook.reliability_score * 100.0) as u32, // Use reliability as "stars"
                                relevance_score: playbook_result.relevance_score * 10.0, // Scale to match GitHub scores
                            };
                            
                            // Mark as internal playbook in description
                            results.push(tool_result);
                        }
                    }
                }
            }
            Err(e) => {
                warn!(error = %e, "Failed to search playbooks, continuing with GitHub search");
            }
        }
    }
    
    // If we found playbooks and have enough results, return early
    if results.len() >= max_results {
        info!(
            playbook_results = results.len(),
            "Returning playbook results only (sufficient matches found)"
        );
        return Ok(results);
    }
    
    // Continue with GitHub search for remaining slots
    let remaining_slots = max_results - results.len();
    info!(query = %query, remaining_slots = remaining_slots, "Searching GitHub for additional tools");

    // Get GitHub token from environment (optional, but recommended for higher rate limits)
    let github_token = env::var("GITHUB_TOKEN").ok();
    
    // Build search query
    let mut search_query = format!("{} in:file", query);
    if let Some(lang) = &language {
        search_query.push_str(&format!(" language:{}", lang));
    }
    
    // Add common tool-related keywords to improve relevance
    search_query.push_str(" (tool OR script OR utility OR function OR execute)");
    
    // GitHub API endpoint
    let url = format!("https://api.github.com/search/code?q={}&per_page={}", 
        urlencoding::encode(&search_query), 
        remaining_slots.min(30) // GitHub API limit
    );

    let client = reqwest::Client::new();
    let mut request = client.get(&url).header("Accept", "application/vnd.github.v3+json");
    
    if let Some(token) = &github_token {
        request = request.header("Authorization", &format!("token {}", token));
    }

    let response = request
        .send()
        .await
        .map_err(|e| format!("Failed to send request to GitHub API: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        
        if status == 403 {
            return Err("GitHub API rate limit exceeded. Set GITHUB_TOKEN environment variable for higher limits.".to_string());
        }
        
        return Err(format!("GitHub API error: {} - {}", status, body));
    }

    let search_response: GitHubCodeSearchResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse GitHub API response: {}", e))?;

    info!(
        total_count = search_response.total_count,
        results = search_response.items.len(),
        "GitHub search completed"
    );

    for (idx, item) in search_response.items.iter().take(remaining_slots).enumerate() {
        // Extract code snippet from text matches
        let code_snippet = item
            .text_matches
            .first()
            .map(|tm| tm.fragment.clone())
            .unwrap_or_else(|| "Code snippet not available".to_string());

        // Build raw file URL
        let raw_url = format!(
            "https://raw.githubusercontent.com/{}/{}/{}",
            item.repository.full_name,
            item.sha,
            item.path
        );

        // Calculate relevance score (combine GitHub score with position)
        let relevance_score = item.score.unwrap_or(0.0) * (1.0 - (idx as f64 * 0.1));

        let result = ToolDiscoveryResult {
            tool_name: item.name.clone(),
            repository: item.repository.full_name.clone(),
            file_path: item.path.clone(),
            language: item.repository.language.clone(),
            description: item.repository.description.clone().unwrap_or_else(|| "No description available".to_string()),
            code_snippet: truncate_snippet(&code_snippet, 500),
            github_url: item.html_url.clone(),
            raw_url,
            stars: item.repository.stargazers_count,
            relevance_score,
        };

        results.push(result);
    }

    // Combine playbook and GitHub results
    if results.is_empty() {
        warn!(query = %query, "No tools found on GitHub");
        return Err(format!("No tools found matching query: '{}'. Try a different search term or language filter.", query));
    }

    info!(
        query = %query,
        found = results.len(),
        "Tool discovery completed"
    );

    Ok(results)
}

/// Propose tool installation by generating a summary and installation instructions
///
/// # Arguments
/// * `tool_result` - The discovered tool result
///
/// # Returns
/// * `Ok(String)` - Formatted proposal message
/// * `Err(String)` - Error message
pub fn propose_tool_installation(tool_result: &ToolDiscoveryResult) -> String {
    format!(
        r#"🔧 **Tool Discovery: {}**

**Repository:** {}
**File:** {}
**Language:** {}
**Stars:** ⭐ {}

**Description:**
{}

**Code Snippet:**
```
{}
```

**Installation Proposal:**
1. Review the tool at: {}
2. Download raw file: {}
3. Integrate into tools repository or execute directly

**Relevance Score:** {:.2}/10.0

Would you like me to:
- Download and review the full tool code?
- Propose integration into the Phoenix tool registry?
- Create a wrapper script for this tool?"#,
        tool_result.tool_name,
        tool_result.repository,
        tool_result.file_path,
        tool_result.language.as_deref().unwrap_or("Unknown"),
        tool_result.stars,
        tool_result.description,
        tool_result.code_snippet,
        tool_result.github_url,
        tool_result.raw_url,
        tool_result.relevance_score
    )
}

/// Truncate text to specified length with ellipsis
fn truncate_snippet(text: &str, max_length: usize) -> String {
    if text.len() <= max_length {
        return text.to_string();
    }
    
    // Try to truncate at a line boundary
    let truncated = &text[..max_length];
    if let Some(last_newline) = truncated.rfind('\n') {
        format!("{}...", &truncated[..last_newline])
    } else {
        format!("{}...", truncated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires network access and GitHub API
    async fn test_find_github_tool() {
        let results = find_github_tool("file scanner python".to_string(), Some("python".to_string()), Some(3))
            .await;
        
        match results {
            Ok(tools) => {
                assert!(!tools.is_empty());
                println!("Found {} tools", tools.len());
                for tool in &tools {
                    println!("- {}: {}", tool.tool_name, tool.repository);
                }
            }
            Err(e) => {
                // This is okay if rate limited or network issues
                println!("Search failed (expected in CI): {}", e);
            }
        }
    }

    #[test]
    fn test_propose_tool_installation() {
        let tool = ToolDiscoveryResult {
            tool_name: "file_scanner.py".to_string(),
            repository: "example/tools".to_string(),
            file_path: "tools/file_scanner.py".to_string(),
            language: Some("python".to_string()),
            description: "A file scanning utility".to_string(),
            code_snippet: "def scan_files(): ...".to_string(),
            github_url: "https://github.com/example/tools/blob/main/tools/file_scanner.py".to_string(),
            raw_url: "https://raw.githubusercontent.com/example/tools/main/tools/file_scanner.py".to_string(),
            stars: 42,
            relevance_score: 8.5,
        };

        let proposal = propose_tool_installation(&tool);
        assert!(proposal.contains("file_scanner.py"));
        assert!(proposal.contains("example/tools"));
        assert!(proposal.contains("42"));
    }
}
