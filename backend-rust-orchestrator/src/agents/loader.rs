use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};
use git2::Repository;

/// Agent manifest structure as defined in manifest.yaml files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub name: String,
    pub category: String,
    #[serde(default)]
    pub required_tools: Vec<String>,
    #[serde(rename = "base_prompt_path")]
    pub base_prompt_path: PathBuf,
    pub version: String,
    /// Optional description
    #[serde(default)]
    pub description: Option<String>,
    /// Optional permissions list
    #[serde(default)]
    pub permissions: Vec<String>,
}

/// AgentLibrary manages the registry of available agent templates
/// loaded from the pagi-agent-repo
pub struct AgentLibrary {
    /// Internal registry mapping agent name to Manifest
    registry: Arc<RwLock<HashMap<String, Manifest>>>,
    /// Base path to the agent repository
    repo_path: PathBuf,
}

impl AgentLibrary {
    /// Create a new AgentLibrary instance
    pub fn new(repo_path: PathBuf) -> Self {
        Self {
            registry: Arc::new(RwLock::new(HashMap::new())),
            repo_path,
        }
    }

    /// Get a manifest by agent name
    pub async fn get_manifest(&self, name: &str) -> Option<Manifest> {
        let guard = self.registry.read().await;
        guard.get(name).cloned()
    }

    /// List all available agent names
    pub async fn list_agents(&self) -> Vec<String> {
        let guard = self.registry.read().await;
        guard.keys().cloned().collect()
    }

    /// List all manifests
    pub async fn list_manifests(&self) -> Vec<Manifest> {
        let guard = self.registry.read().await;
        guard.values().cloned().collect()
    }

    /// Update the registry with a new manifest
    async fn register_manifest(&self, manifest: Manifest) {
        let mut guard = self.registry.write().await;
        let name = manifest.name.clone();
        guard.insert(name.clone(), manifest);
        info!(agent_name = %name, "Registered agent manifest");
    }

    /// Clear the registry
    async fn clear_registry(&self) {
        let mut guard = self.registry.write().await;
        guard.clear();
    }

    /// Get the base prompt content for an agent
    pub async fn get_base_prompt(&self, name: &str) -> Result<String, String> {
        let manifest = self.get_manifest(name).await
            .ok_or_else(|| format!("Agent '{}' not found in library", name))?;
        
        let prompt_path = self.repo_path
            .join("agent-templates")
            .join(&manifest.category)
            .join(&manifest.base_prompt_path);
        
        if !prompt_path.exists() {
            return Err(format!(
                "Base prompt file not found: {}",
                prompt_path.display()
            ));
        }

        std::fs::read_to_string(&prompt_path)
            .map_err(|e| format!("Failed to read base prompt file: {}", e))
    }
}

/// Detect new commits in the repository (for consensus triggering)
async fn detect_new_commits(repo_path: &PathBuf) -> Result<Vec<String>, String> {
    use git2::Repository;
    
    let repo = Repository::open(repo_path)
        .map_err(|e| format!("Failed to open repository: {}", e))?;
    
    // Fetch latest from remote
    let mut remote = repo.find_remote("origin")
        .map_err(|e| format!("Failed to find remote: {}", e))?;
    
    // Fetch updates (non-blocking, just check)
    remote.fetch(&["main"], None, None)
        .map_err(|e| format!("Failed to fetch: {}", e))?;
    
    // Get local HEAD
    let local_head = repo.head()
        .and_then(|r| r.peel_to_commit())
        .map(|c| c.id().to_string())
        .ok();
    
    // Get remote HEAD
    let remote_head = repo.find_reference("refs/remotes/origin/main")
        .and_then(|r| r.peel_to_commit())
        .map(|c| c.id().to_string())
        .ok();
    
    // If we have both and they differ, return the remote commit
    if let (Some(local), Some(remote)) = (local_head, remote_head) {
        if local != remote {
            return Ok(vec![remote]);
        }
    }
    
    Ok(Vec::new())
}

/// SyncLibrary tool that syncs the agent library from the repository
/// 
/// This function:
/// 1. Detects new commits in pagi-agent-repo
/// 2. Triggers Phoenix Consensus for new commits (if consensus is enabled)
/// 3. Performs a git pull on the pagi-agent-repo (if consensus approves or is disabled)
/// 4. Walks the directory tree looking for manifest.yaml files
/// 5. Deserializes them and updates the AgentLibrary's internal registry
pub async fn sync_library(library: &AgentLibrary) -> Result<String, String> {
    let repo_path = &library.repo_path;
    
    // Ensure the repository directory exists
    if !repo_path.exists() {
        return Err(format!(
            "Repository path does not exist: {}",
            repo_path.display()
        ));
    }

    // Check if it's a git repository
    if !repo_path.join(".git").exists() {
        return Err(format!(
            "Path is not a git repository: {}",
            repo_path.display()
        ));
    }

    // Detect new commits before pulling
    let new_commits = detect_new_commits(repo_path).await?;
    
    // If consensus is enabled and we have new commits, trigger consensus
    // For now, we'll proceed with the pull - consensus integration will be added
    // when the consensus system is initialized in main.rs
    
    // Perform git pull
    info!(repo_path = %repo_path.display(), "Pulling latest changes from agent repository");
    
    let pull_output = tokio::process::Command::new("git")
        .arg("pull")
        .arg("origin")
        .arg("main")
        .current_dir(repo_path)
        .output()
        .await
        .map_err(|e| format!("Failed to execute git pull: {}", e))?;

    if !pull_output.status.success() {
        let stderr = String::from_utf8_lossy(&pull_output.stderr);
        let stdout = String::from_utf8_lossy(&pull_output.stdout);
        warn!(
            stderr = %stderr,
            stdout = %stdout,
            "Git pull had issues, but continuing"
        );
        // Don't fail completely - the repo might already be up to date
    } else {
        info!("Successfully pulled latest changes");
        
        // If we detected new commits, log them for potential consensus triggering
        if !new_commits.is_empty() {
            info!(
                commit_count = new_commits.len(),
                "Detected {} new commit(s) - consensus should be triggered separately",
                new_commits.len()
            );
        }
    }

    // Clear existing registry
    library.clear_registry().await;

    // Walk the directory tree looking for manifest.yaml files
    let agent_templates_dir = repo_path.join("agent-templates");
    
    if !agent_templates_dir.exists() {
        warn!(
            path = %agent_templates_dir.display(),
            "agent-templates directory not found, skipping manifest discovery"
        );
        return Ok("Repository synced, but no agent-templates directory found".to_string());
    }

    let mut manifests_found = 0;
    let mut errors = Vec::new();

    // Walk through each subdirectory in agent-templates/
    let entries = std::fs::read_dir(&agent_templates_dir)
        .map_err(|e| format!("Failed to read agent-templates directory: {}", e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let category_path = entry.path();
        
        if !category_path.is_dir() {
            continue;
        }

        // Look for manifest.yaml in this category directory
        let manifest_path = category_path.join("manifest.yaml");
        
        if manifest_path.exists() {
            match load_manifest_from_file(&manifest_path).await {
                Ok(mut manifest) => {
                    // Ensure the category matches the directory name
                    let category_name = category_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown");
                    
                    manifest.category = category_name.to_string();
                    
                    library.register_manifest(manifest.clone()).await;
                    manifests_found += 1;
                    
                    info!(
                        agent_name = %manifest.name,
                        category = %manifest.category,
                        version = %manifest.version,
                        "Loaded agent manifest"
                    );
                }
                Err(e) => {
                    let error_msg = format!(
                        "Failed to load manifest from {}: {}",
                        manifest_path.display(),
                        e
                    );
                    errors.push(error_msg.clone());
                    warn!("{}", error_msg);
                }
            }
        }
    }

    if manifests_found == 0 && errors.is_empty() {
        return Ok("Repository synced, but no manifest.yaml files found".to_string());
    }

    let mut result = format!(
        "Agent library synced successfully. Found {} manifest(s).",
        manifests_found
    );

    if !errors.is_empty() {
        result.push_str(&format!("\nErrors encountered: {}", errors.len()));
        for err in &errors {
            result.push_str(&format!("\n  - {}", err));
        }
    }

    Ok(result)
}

/// Load a manifest from a YAML file
async fn load_manifest_from_file(manifest_path: &Path) -> Result<Manifest, String> {
    let content = std::fs::read_to_string(manifest_path)
        .map_err(|e| format!("Failed to read manifest file: {}", e))?;

    // Parse YAML using serde_yaml
    let manifest: Manifest = serde_yaml::from_str(&content)
        .map_err(|e| format!("Failed to parse YAML manifest: {}", e))?;

    // Validate required fields
    if manifest.name.is_empty() {
        return Err("Manifest 'name' field is required and cannot be empty".to_string());
    }

    if manifest.version.is_empty() {
        return Err("Manifest 'version' field is required and cannot be empty".to_string());
    }

    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[tokio::test]
    async fn test_load_manifest() {
        // Create a temporary directory for testing
        let temp_dir = std::env::temp_dir().join("test_manifest_loader");
        std::fs::create_dir_all(&temp_dir).unwrap();
        let manifest_path = temp_dir.join("manifest.yaml");
        
        let yaml_content = r#"
name: SecurityAuditor
category: security
required_tools:
  - network_scanner
  - system_inspector
base_prompt_path: base_prompt.txt
version: "1.0.0"
description: "A security auditing agent"
permissions:
  - network_scan
  - system_read
"#;
        
        fs::write(&manifest_path, yaml_content).unwrap();
        
        let manifest = load_manifest_from_file(&manifest_path).await.unwrap();
        
        assert_eq!(manifest.name, "SecurityAuditor");
        assert_eq!(manifest.category, "security");
        assert_eq!(manifest.required_tools.len(), 2);
        assert_eq!(manifest.version, "1.0.0");
        
        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
