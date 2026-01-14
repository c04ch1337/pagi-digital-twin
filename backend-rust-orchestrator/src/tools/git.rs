use std::path::{Path, PathBuf};
use std::env;
use tracing::{info, warn, error};
use git2::{Repository, Signature, Commit, Error as GitError, Cred, CredentialType, RemoteCallbacks, PushOptions, build::RepoBuilder};
use crate::security::PrivacyFilter;

/// Git operations for committing playbooks to GitHub
pub struct GitOperations;

/// Safety check: Only allow pushing files within allowed directories
fn is_allowed_path(file_path: &Path, repo_root: &Path, allowed_dirs: &[&str]) -> bool {
    if let Ok(relative) = file_path.strip_prefix(repo_root) {
        let path_str = relative.to_string_lossy();
        // Check if the path starts with any allowed directory
        allowed_dirs.iter().any(|&dir| {
            path_str.starts_with(dir) || path_str.starts_with(&format!("{}/", dir))
        })
    } else {
        false
    }
}

/// SSH credential callback for git2
/// Uses ~/.ssh/id_rsa key with explicit public key at ~/.ssh/id_rsa.pub
fn ssh_credential_callback(
    _url: &str,
    username_from_url: Option<&str>,
    _allowed_types: CredentialType,
) -> Result<Cred, GitError> {
    let username = username_from_url.unwrap_or("git");
    
    // Get home directory (cross-platform)
    let home = env::var("HOME")
        .or_else(|_| env::var("USERPROFILE")) // Windows fallback
        .map_err(|_| GitError::from_str("Could not find home directory. Set HOME or USERPROFILE environment variable."))?;
    
    let ssh_dir = PathBuf::from(&home).join(".ssh");
    let private_key_path = ssh_dir.join("id_rsa");
    let public_key_path = ssh_dir.join("id_rsa.pub");
    
    // Check if SSH keys exist
    if !private_key_path.exists() {
        return Err(GitError::from_str(
            format!(
                "SSH private key not found at {}. Please ensure ~/.ssh/id_rsa exists and is accessible.",
                private_key_path.display()
            ).as_str()
        ));
    }
    
    // Use explicit public key path if it exists, otherwise None
    let pub_key_opt = if public_key_path.exists() {
        Some(public_key_path.as_path())
    } else {
        warn!(
            public_key = %public_key_path.display(),
            "Public key not found, attempting without it"
        );
        None
    };
    
    // Create SSH key credential
    Cred::ssh_key(
        username,
        pub_key_opt,
        &private_key_path,
        None, // No passphrase - could be enhanced to support passphrase-protected keys
    )
    .map_err(|e| {
        GitError::from_str(
            format!(
                "Failed to create SSH credential: {}. Verify that ~/.ssh/id_rsa is a valid SSH private key.",
                e
            ).as_str()
        )
    })
}

impl GitOperations {
    /// Initialize a git repository if it doesn't exist, or open existing one
    pub fn open_or_init_repo(path: &Path) -> Result<Repository, String> {
        match Repository::open(path) {
            Ok(repo) => {
                info!(path = %path.display(), "Opened existing git repository");
                Ok(repo)
            }
            Err(_) => {
                info!(path = %path.display(), "Initializing new git repository");
                Repository::init(path)
                    .map_err(|e| format!("Failed to initialize git repository: {}", e))
            }
        }
    }

    /// Stage all files in a directory with safety checks
    pub fn stage_all(repo: &Repository, dir: &Path) -> Result<(), String> {
        use std::fs;
        
        // Get relative path from repo root
        let repo_workdir = repo.workdir()
            .ok_or_else(|| "Repository has no workdir".to_string())?;
        
        // Safety check: Only allow staging files in /playbooks or /agent-templates
        let allowed_dirs = ["playbooks", "agent-templates"];
        let relative_dir = dir.strip_prefix(repo_workdir)
            .map_err(|e| format!("Directory is not within repo: {}", e))?;
        
        let dir_str = relative_dir.to_string_lossy();
        let is_allowed = allowed_dirs.iter().any(|&allowed| {
            dir_str == allowed || dir_str.starts_with(&format!("{}/", allowed))
        });
        
        if !is_allowed {
            return Err(format!(
                "Safety check failed: Only files in 'playbooks' or 'agent-templates' directories can be staged. Attempted: {}",
                dir_str
            ));
        }
        
        // Verify all files in the directory are within allowed paths
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries {
                if let Ok(entry) = entry {
                    let file_path = entry.path();
                    if !is_allowed_path(&file_path, repo_workdir, &allowed_dirs) {
                        return Err(format!(
                            "Safety check failed: File '{}' is outside allowed directories",
                            file_path.display()
                        ));
                    }
                }
            }
        }
        
        let mut index = repo.index()
            .map_err(|e| format!("Failed to get repository index: {}", e))?;

        // Add all files in the directory recursively
        let pattern = format!("{}", relative_dir.to_string_lossy());
        
        index.add_all(&[&pattern], git2::IndexAddOption::DEFAULT, None)
            .map_err(|e| format!("Failed to add files to index: {}", e))?;

        index.write()
            .map_err(|e| format!("Failed to write index: {}", e))?;

        info!(dir = %dir.display(), "Staged all files in directory (safety checks passed)");
        Ok(())
    }

    /// Commit staged changes
    pub fn commit(
        repo: &Repository,
        message: &str,
        author_name: &str,
        author_email: &str,
    ) -> Result<Commit, String> {
        let signature = Signature::now(author_name, author_email)
            .map_err(|e| format!("Failed to create signature: {}", e))?;

        let tree_id = {
            let mut index = repo.index()
                .map_err(|e| format!("Failed to get index: {}", e))?;
            index.write_tree()
                .map_err(|e| format!("Failed to write tree: {}", e))?
        };

        let tree = repo.find_tree(tree_id)
            .map_err(|e| format!("Failed to find tree: {}", e))?;

        // Get the HEAD commit (if it exists) for parent
        let parent_commit = repo.head()
            .ok()
            .and_then(|head| head.target())
            .and_then(|oid| repo.find_commit(oid).ok());

        let parents: Vec<&Commit> = if let Some(ref parent) = parent_commit {
            vec![parent]
        } else {
            vec![]
        };

        let commit_id = repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            &parents.iter().map(|p| p).collect::<Vec<_>>(),
        )
        .map_err(|e| format!("Failed to create commit: {}", e))?;

        let commit = repo.find_commit(commit_id)
            .map_err(|e| format!("Failed to find commit: {}", e))?;

        info!(
            commit_id = %commit_id,
            message = %message,
            "Created commit"
        );

        Ok(commit)
    }

    /// Push to remote repository with SSH authentication
    pub fn push(repo: &Repository, remote_name: &str, branch: &str) -> Result<(), String> {
        let mut remote = repo.find_remote(remote_name)
            .map_err(|e| format!("Failed to find remote '{}': {}", remote_name, e))?;

        let refspec = format!("refs/heads/{}:refs/heads/{}", branch, branch);
        
        info!(
            remote = %remote_name,
            branch = %branch,
            refspec = %refspec,
            "Pushing to remote with SSH authentication"
        );

        // Set up SSH credential callback
        let mut callbacks = RemoteCallbacks::new();
        callbacks.credentials(|url, username_from_url, allowed_types| {
            ssh_credential_callback(url, username_from_url, allowed_types)
        });

        let mut push_options = PushOptions::new();
        push_options.remote_callbacks(callbacks);

        remote.push(&[&refspec], Some(&mut push_options))
            .map_err(|e| format!("Failed to push to remote: {}. Make sure SSH key is configured at ~/.ssh/id_rsa or SSH agent is running.", e))?;

        info!(
            remote = %remote_name,
            branch = %branch,
            "Successfully pushed to remote"
        );

        Ok(())
    }

    /// Add, commit, and push playbooks in one operation with safety checks
    /// This is a convenience wrapper that maintains backward compatibility
    pub async fn commit_and_push_playbooks(
        repo_path: &Path,
        playbooks_dir: &Path,
        commit_message: &str,
        remote_name: &str,
        branch: &str,
        author_name: &str,
        author_email: &str,
    ) -> Result<(), String> {
        // If playbooks_dir is the repo root or we want to push both directories,
        // use the new push_to_origin function
        let repo_workdir = {
            let repo = Self::open_or_init_repo(repo_path)?;
            repo.workdir()
                .ok_or_else(|| "Repository has no workdir".to_string())?
                .to_path_buf()
        };
        
        // Check if playbooks_dir is the repo root (indicating we want to push everything)
        if playbooks_dir == &repo_workdir {
            // Use the new push_to_origin function which handles both directories
            let repo_path = repo_path.to_path_buf();
            tokio::task::spawn_blocking(move || {
                Self::push_to_origin(&repo_path)
            })
            .await
            .map_err(|e| format!("Task join error: {}", e))?
        } else {
            // Legacy behavior: push specific directory
            let repo = Self::open_or_init_repo(repo_path)?;

            // Safety check: Ensure playbooks_dir is within allowed directories
            let relative_dir = playbooks_dir.strip_prefix(&repo_workdir)
                .map_err(|e| format!("Playbooks directory is not within repo: {}", e))?;
            
            let allowed_dirs = ["playbooks", "agent-templates"];
            let dir_str = relative_dir.to_string_lossy();
            let is_allowed = allowed_dirs.iter().any(|&allowed| {
                dir_str == allowed || dir_str.starts_with(&format!("{}/", allowed))
            });
            
            if !is_allowed {
                return Err(format!(
                    "Safety check failed: Only files in 'playbooks' or 'agent-templates' directories can be pushed. Attempted: {}",
                    dir_str
                ));
            }

            // Stage all playbook files (with additional safety checks)
            Self::stage_all(&repo, playbooks_dir)?;

            // Check if there are any changes to commit
            let mut index = repo.index()
                .map_err(|e| format!("Failed to get index: {}", e))?;
            
            if index.len() == 0 {
                info!("No changes to commit");
                return Ok(());
            }

            // Commit
            Self::commit(&repo, commit_message, author_name, author_email)?;

            // Push with SSH authentication
            Self::push(&repo, remote_name, branch)?;

            info!(
                repo_path = %repo_path.display(),
                playbooks_dir = %playbooks_dir.display(),
                "Successfully committed and pushed playbooks"
            );

            Ok(())
        }
    }

    /// Async wrapper for push_to_origin
    pub async fn push_to_origin_async(repo_path: &Path) -> Result<(), String> {
        let repo_path = repo_path.to_path_buf();
        tokio::task::spawn_blocking(move || {
            Self::push_to_origin(&repo_path)
        })
        .await
        .map_err(|e| format!("Task join error: {}", e))?
    }

    /// Ensure remote is configured for the repository
    pub fn ensure_remote(repo: &Repository, remote_name: &str, remote_url: &str) -> Result<(), String> {
        if repo.find_remote(remote_name).is_err() {
            info!(remote = %remote_name, url = %remote_url, "Adding remote");
            repo.remote(remote_name, remote_url)
                .map_err(|e| format!("Failed to add remote '{}': {}", remote_name, e))?;
        } else {
            // Update remote URL if it exists
            let mut remote = repo.find_remote(remote_name)
                .map_err(|e| format!("Failed to find remote '{}': {}", remote_name, e))?;
            remote.set_url(remote_url)
                .map_err(|e| format!("Failed to update remote URL: {}", e))?;
        }
        Ok(())
    }

    /// Scrub files in a directory before staging (pre-commit privacy filter)
    /// First applies Rust PrivacyFilter, then Ferrellgas-specific Python scrubber
    fn scrub_directory_files(dir: &Path, filter: &PrivacyFilter) -> Result<(), String> {
        use std::fs;
        use std::process::Command;
        
        if !dir.exists() || !dir.is_dir() {
            return Ok(());
        }

        let entries = fs::read_dir(dir)
            .map_err(|e| format!("Failed to read directory: {}", e))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
            let path = entry.path();

            if path.is_dir() {
                // Recursively scrub subdirectories
                Self::scrub_directory_files(&path, filter)?;
            } else if path.is_file() {
                // Only scrub text files (markdown, yaml, txt, etc.)
                if let Some(ext) = path.extension() {
                    let ext_str = ext.to_string_lossy().to_lowercase();
                    if matches!(ext_str.as_str(), "md" | "yaml" | "yml" | "txt" | "env" | "conf" | "config") {
                        // Step 1: Apply Rust PrivacyFilter
                        let content = fs::read_to_string(&path)
                            .map_err(|e| format!("Failed to read file {}: {}", path.display(), e))?;
                        
                        let scrubbed = filter.scrub_playbook(content);
                        
                        fs::write(&path, scrubbed)
                            .map_err(|e| format!("Failed to write scrubbed file {}: {}", path.display(), e))?;
                        
                        // Step 2: Apply Ferrellgas-specific Python scrubber
                        // Find the scripts directory relative to the workspace root
                        let script_path = if let Ok(current_dir) = std::env::current_dir() {
                            // Try to find scripts/ferrellgas_scrubber.py
                            let script = current_dir.join("scripts").join("ferrellgas_scrubber.py");
                            if script.exists() {
                                script
                            } else {
                                // Try parent directory (if we're in backend-rust-orchestrator)
                                current_dir.parent()
                                    .map(|p| p.join("scripts").join("ferrellgas_scrubber.py"))
                                    .unwrap_or_else(|| script)
                            }
                        } else {
                            Path::new("scripts").join("ferrellgas_scrubber.py")
                        };

                        if script_path.exists() {
                            match Command::new("python3")
                                .arg(script_path.to_string_lossy().as_ref())
                                .arg(path.to_string_lossy().as_ref())
                                .output()
                            {
                                Ok(output) => {
                                    if !output.status.success() {
                                        warn!(
                                            file = %path.display(),
                                            stderr = %String::from_utf8_lossy(&output.stderr),
                                            "Ferrellgas scrubber returned non-zero exit code"
                                        );
                                    } else {
                                        info!(file = %path.display(), "Applied Ferrellgas scrubber");
                                    }
                                }
                                Err(e) => {
                                    // If python3 is not available, try python
                                    match Command::new("python")
                                        .arg(script_path.to_string_lossy().as_ref())
                                        .arg(path.to_string_lossy().as_ref())
                                        .output()
                                    {
                                        Ok(output) => {
                                            if !output.status.success() {
                                                warn!(
                                                    file = %path.display(),
                                                    stderr = %String::from_utf8_lossy(&output.stderr),
                                                    "Ferrellgas scrubber returned non-zero exit code"
                                                );
                                            } else {
                                                info!(file = %path.display(), "Applied Ferrellgas scrubber");
                                            }
                                        }
                                        Err(_) => {
                                            warn!(
                                                file = %path.display(),
                                                error = %e,
                                                "Failed to run Ferrellgas scrubber (Python not available or script not found)"
                                            );
                                            // Continue without Ferrellgas scrubbing - Rust filter already applied
                                        }
                                    }
                                }
                            }
                        } else {
                            // Script not found - this is OK, just log a warning
                            warn!(
                                script_path = %script_path.display(),
                                "Ferrellgas scrubber script not found, skipping Ferrellgas-specific scrubbing"
                            );
                        }
                        
                        info!(file = %path.display(), "Scrubbed file for privacy");
                    }
                }
            }
        }

        Ok(())
    }

    /// Push to origin with comprehensive error handling
    /// Handles git add for /playbooks and /agent-templates, commits, and pushes
    /// Automatically scrubs sensitive information before committing
    pub fn push_to_origin(repo_path: &Path) -> Result<(), String> {
        use std::fs;
        use chrono::Utc;
        
        // Initialize privacy filter
        let privacy_filter = PrivacyFilter::new();
        
        // Open repository with retry logic for locked index
        let repo = Self::open_or_init_repo(repo_path)?;
        
        let repo_workdir = repo.workdir()
            .ok_or_else(|| "Repository has no workdir".to_string())?;
        
        // Ensure remote is configured
        const REMOTE_URL: &str = "git@github.com:c04ch1337/pagi-agent-repo.git";
        Self::ensure_remote(&repo, "origin", REMOTE_URL)?;
        
        // Scrub and stage playbooks directory
        let playbooks_dir = repo_workdir.join("playbooks");
        if playbooks_dir.exists() {
            info!("Scrubbing and staging playbooks directory");
            // Scrub files before staging
            Self::scrub_directory_files(&playbooks_dir, &privacy_filter)
                .map_err(|e| format!("Failed to scrub playbooks: {}", e))?;
            Self::stage_all(&repo, &playbooks_dir)
                .map_err(|e| format!("Failed to stage playbooks: {}", e))?;
        } else {
            warn!("Playbooks directory does not exist, skipping");
        }
        
        // Scrub and stage agent-templates directory
        let agent_templates_dir = repo_workdir.join("agent-templates");
        if agent_templates_dir.exists() {
            info!("Scrubbing and staging agent-templates directory");
            // Scrub files before staging
            Self::scrub_directory_files(&agent_templates_dir, &privacy_filter)
                .map_err(|e| format!("Failed to scrub agent-templates: {}", e))?;
            Self::stage_all(&repo, &agent_templates_dir)
                .map_err(|e| format!("Failed to stage agent-templates: {}", e))?;
        } else {
            warn!("Agent-templates directory does not exist, skipping");
        }
        
        // Check for locked index with retry logic
        let mut index = loop {
            match repo.index() {
                Ok(idx) => break idx,
                Err(e) => {
                    let err_msg = e.message();
                    if err_msg.contains("locked") || err_msg.contains("index.lock") {
                        warn!("Index is locked, waiting and retrying...");
                        std::thread::sleep(std::time::Duration::from_millis(500));
                        // Try to remove lock file if it exists
                        let lock_path = repo_workdir.join(".git").join("index.lock");
                        if lock_path.exists() {
                            if let Err(rm_err) = fs::remove_file(&lock_path) {
                                warn!("Failed to remove lock file: {}", rm_err);
                            } else {
                                info!("Removed stale lock file");
                            }
                        }
                        continue;
                    } else {
                        return Err(format!("Failed to get repository index: {}", e));
                    }
                }
            }
        };
        
        // Check if there are any changes to commit
        let has_changes = {
            // Write index to check for changes
            index.write()
                .map_err(|e| format!("Failed to write index: {}", e))?;
            
            // Reload index to get accurate count
            let mut fresh_index = repo.index()
                .map_err(|e| format!("Failed to reload index: {}", e))?;
            fresh_index.len() > 0
        };
        
        if !has_changes {
            info!("No changes to commit");
            return Ok(());
        }
        
        // Create commit message with timestamp
        let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
        let commit_message = format!("Phoenix Update: {} - Distributed Learning", timestamp);
        
        // Commit with retry logic for locked index
        let commit_result = loop {
            match Self::commit(&repo, &commit_message, "Orchestrator", "orchestrator@digital-twin.local") {
                Ok(commit) => break Ok(commit),
                Err(e) => {
                    if e.contains("locked") || e.contains("index.lock") {
                        warn!("Index locked during commit, retrying...");
                        std::thread::sleep(std::time::Duration::from_millis(500));
                        // Remove lock file
                        let lock_path = repo_workdir.join(".git").join("index.lock");
                        if lock_path.exists() {
                            if let Err(rm_err) = fs::remove_file(&lock_path) {
                                warn!("Failed to remove lock file: {}", rm_err);
                            }
                        }
                        continue;
                    } else {
                        break Err(e);
                    }
                }
            }
        };
        
        commit_result?;
        
        // Push with comprehensive error handling
        let push_result = Self::push(&repo, "origin", "main");
        
        match push_result {
            Ok(_) => {
                info!(
                    repo_path = %repo_path.display(),
                    "Successfully pushed to origin"
                );
                Ok(())
            }
            Err(e) => {
                // Check for authentication errors
                if e.contains("authentication") || e.contains("Auth") || e.contains("SSH") {
                    return Err(format!(
                        "Authentication failed: {}. \
                        Please ensure:\n\
                        1. SSH key exists at ~/.ssh/id_rsa\n\
                        2. Public key exists at ~/.ssh/id_rsa.pub\n\
                        3. SSH key is added to your GitHub account\n\
                        4. Test SSH connection: ssh -T git@github.com",
                        e
                    ));
                }
                
                // Check for network errors
                if e.contains("network") || e.contains("connection") || e.contains("timeout") {
                    return Err(format!(
                        "Network error during push: {}. \
                        Please check your internet connection and GitHub accessibility.",
                        e
                    ));
                }
                
                // Generic error
                Err(format!("Failed to push to origin: {}", e))
            }
        }
    }
}
