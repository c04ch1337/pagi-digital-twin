use std::path::{Path, PathBuf};
use std::fs;
use tracing::{error, info, warn};
use yaml_rust::{Yaml, YamlLoader};

/// Agent library manifest structure
#[derive(Debug, Clone)]
pub struct AgentManifest {
    pub agent_name: String,
    pub description: String,
    pub category: String, // "research", "sys-admin", "creative", etc.
    pub required_tools: Vec<String>,
    pub system_prompt_path: Option<String>, // Path to base_prompt.txt
    pub system_prompt: String, // Loaded system prompt content
    pub permissions: Vec<String>,
    pub privacy_filter: Option<bool>, // Whether to apply privacy filtering
}

/// Sync the agent library from GitHub repository
pub async fn sync_agent_library() -> Result<String, String> {
    let repo_url = "git@github.com:c04ch1337/pagi-agent-repo.git";
    let base_path = Path::new("config/agents");
    
    // Ensure base directory exists
    if let Err(e) = fs::create_dir_all(base_path) {
        return Err(format!("Failed to create agents directory: {}", e));
    }

    let repo_path = base_path.join("pagi-agent-repo");

    // Check if repository already exists
    let exists = repo_path.exists() && repo_path.join(".git").exists();

    if exists {
        // Pull latest changes
        info!("Pulling latest changes from agent repository");
        
        // First, ensure we're on the main branch
        let checkout_output = tokio::process::Command::new("git")
            .arg("checkout")
            .arg("main")
            .current_dir(&repo_path)
            .output()
            .await;
        
        if let Err(e) = checkout_output {
            warn!("Failed to checkout main branch (may not exist yet): {}", e);
        }

        // Pull latest changes
        let output = tokio::process::Command::new("git")
            .arg("pull")
            .arg("origin")
            .arg("main")
            .current_dir(&repo_path)
            .output()
            .await
            .map_err(|e| format!("Failed to execute git pull: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            warn!(
                stderr = %stderr,
                stdout = %stdout,
                "Git pull had issues, but continuing"
            );
            // Don't fail completely - the repo might already be up to date
        } else {
            info!("Successfully pulled latest changes");
        }
    } else {
        // Clone the repository
        info!("Cloning agent repository from GitHub");
        let output = tokio::process::Command::new("git")
            .arg("clone")
            .arg(repo_url)
            .arg(&repo_path)
            .output()
            .await
            .map_err(|e| format!("Failed to execute git clone: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            error!(
                stderr = %stderr,
                stdout = %stdout,
                "Git clone failed"
            );
            return Err(format!("Git clone failed: {}. Make sure SSH key is configured and repository is accessible.", stderr));
        }

        info!("Successfully cloned agent repository");
    }

    // Organize into category folders
    organize_agent_categories(&repo_path, base_path).await?;

    Ok(format!("Agent library synced successfully to {}", base_path.display()))
}

/// Organize agent templates into category folders
/// Scans agent-templates/ directory in the repository for category subdirectories
async fn organize_agent_categories(repo_path: &Path, base_path: &Path) -> Result<(), String> {
    // The repository structure has agent-templates/ at the root
    let agent_templates_dir = repo_path.join("agent-templates");
    
    if !agent_templates_dir.exists() {
        warn!(
            path = %agent_templates_dir.display(),
            "agent-templates directory not found in repository, skipping organization"
        );
        return Ok(());
    }

    // Scan for category subdirectories in agent-templates/
    let entries = fs::read_dir(&agent_templates_dir)
        .map_err(|e| format!("Failed to read agent-templates directory: {}", e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let category_path = entry.path();
        
        if !category_path.is_dir() {
            continue;
        }

        let category_name = category_path.file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| "Invalid category name".to_string())?;

        // Create corresponding category directory in base_path
        let dest_category_path = base_path.join(category_name);
        if let Err(e) = fs::create_dir_all(&dest_category_path) {
            warn!("Failed to create category directory {}: {}", category_name, e);
            continue;
        }

        // Look for manifest.yaml in the category directory
        let manifest_path = category_path.join("manifest.yaml");
        if manifest_path.exists() {
            let dest_manifest = dest_category_path.join("manifest.yaml");
            if let Err(e) = fs::copy(&manifest_path, &dest_manifest) {
                warn!("Failed to copy manifest for {}: {}", category_name, e);
            } else {
                info!("Copied manifest.yaml for category: {}", category_name);
            }
        }

        // Copy all files from the category directory (including base_prompt.txt, etc.)
        copy_directory_contents(&category_path, &dest_category_path)?;
        info!(
            category = %category_name,
            "Organized agent template category"
        );
    }

    Ok(())
}

/// Copy directory contents recursively
fn copy_directory_contents(src: &Path, dst: &Path) -> Result<(), String> {
    if !src.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(src).map_err(|e| format!("Failed to read directory: {}", e))? {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let path = entry.path();
        let file_name = entry.file_name();
        let dst_path = dst.join(&file_name);

        if path.is_dir() {
            fs::create_dir_all(&dst_path)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
            copy_directory_contents(&path, &dst_path)?;
        } else {
            fs::copy(&path, &dst_path)
                .map_err(|e| format!("Failed to copy file: {}", e))?;
        }
    }

    Ok(())
}

/// Parse a manifest.yaml file and return AgentManifest
pub fn parse_manifest(manifest_path: &Path) -> Result<AgentManifest, String> {
    let content = fs::read_to_string(manifest_path)
        .map_err(|e| format!("Failed to read manifest file: {}", e))?;

    let docs = YamlLoader::load_from_str(&content)
        .map_err(|e| format!("Failed to parse YAML: {}", e))?;

    if docs.is_empty() {
        return Err("Manifest file is empty".to_string());
    }

    let doc = &docs[0];

    // Parse agent_name (preferred) or fall back to 'name'
    let agent_name = doc["agent_name"]
        .as_str()
        .or_else(|| doc["name"].as_str())
        .ok_or_else(|| "Missing 'agent_name' or 'name' field in manifest".to_string())?
        .to_string();

    let description = doc["description"]
        .as_str()
        .unwrap_or("")
        .to_string();

    let category = doc["category"]
        .as_str()
        .unwrap_or("general")
        .to_string();

    // Parse required_tools (preferred) or fall back to 'tools'
    let required_tools = if let Some(tools_array) = doc["required_tools"].as_vec() {
        tools_array
            .iter()
            .filter_map(|t| t.as_str().map(|s| s.to_string()))
            .collect()
    } else if let Some(tools_array) = doc["tools"].as_vec() {
        tools_array
            .iter()
            .filter_map(|t| t.as_str().map(|s| s.to_string()))
            .collect()
    } else {
        Vec::new()
    };

    // Parse system_prompt_path
    let system_prompt_path = doc["system_prompt_path"]
        .as_str()
        .map(|s| s.to_string());

    // Load system prompt from file if path is provided
    let system_prompt = if let Some(ref prompt_path) = system_prompt_path {
        let manifest_dir = manifest_path.parent()
            .ok_or_else(|| "Manifest has no parent directory".to_string())?;
        let full_prompt_path = manifest_dir.join(prompt_path);
        
        if full_prompt_path.exists() {
            fs::read_to_string(&full_prompt_path)
                .map_err(|e| format!("Failed to read system prompt file '{}': {}", prompt_path, e))?
        } else {
            warn!(
                prompt_path = %full_prompt_path.display(),
                "System prompt file not found, using empty prompt"
            );
            String::new()
        }
    } else if let Some(prompt_str) = doc["system_prompt"].as_str() {
        // Fallback to inline system_prompt field
        prompt_str.to_string()
    } else {
        String::new()
    };

    let permissions = if let Some(perms_array) = doc["permissions"].as_vec() {
        perms_array
            .iter()
            .filter_map(|p| p.as_str().map(|s| s.to_string()))
            .collect()
    } else {
        Vec::new()
    };

    let privacy_filter = doc["privacy_filter"]
        .as_bool();

    Ok(AgentManifest {
        agent_name,
        description,
        category,
        required_tools,
        system_prompt_path,
        system_prompt,
        permissions,
        privacy_filter,
    })
}

/// List all available agent templates from the library
/// Scans all category directories in base_path for manifest.yaml files
pub fn list_agent_templates(base_path: &Path) -> Result<Vec<AgentManifest>, String> {
    let mut manifests = Vec::new();

    // Scan all directories in base_path (each represents a category)
    if !base_path.exists() {
        return Ok(manifests);
    }

    let entries = fs::read_dir(base_path)
        .map_err(|e| format!("Failed to read agents directory: {}", e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let category_path = entry.path();
        
        // Skip the pagi-agent-repo directory itself
        if category_path.file_name()
            .and_then(|n| n.to_str())
            .map(|n| n == "pagi-agent-repo")
            .unwrap_or(false) {
            continue;
        }

        if !category_path.is_dir() {
            continue;
        }

        let manifest_path = category_path.join("manifest.yaml");
        if manifest_path.exists() {
            match parse_manifest(&manifest_path) {
                Ok(manifest) => {
                    info!(
                        category = %manifest.category,
                        agent_name = %manifest.agent_name,
                        "Found agent template"
                    );
                    manifests.push(manifest);
                }
                Err(e) => {
                    let category_name = category_path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown");
                    warn!("Failed to parse manifest for {}: {}", category_name, e);
                }
            }
        }
    }

    Ok(manifests)
}
