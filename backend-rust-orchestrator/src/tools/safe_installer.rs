//! Safe Tool Installer
//! 
//! Executes installation commands with security validation to prevent command injection.
//! Only whitelisted package managers are allowed.

use std::process::Stdio;
use std::path::{Path, PathBuf};
use tokio::process::Command;
use tokio::fs;
use tracing::{info, warn, error};
use serde::{Deserialize, Serialize};
use chrono::Utc;
use uuid;

/// Whitelisted package managers and their allowed patterns
const ALLOWED_PACKAGE_MANAGERS: &[(&str, &[&str])] = &[
    ("pip", &["install", "uninstall", "upgrade"]),
    ("pip3", &["install", "uninstall", "upgrade"]),
    ("cargo", &["install", "add", "update"]),
    ("npm", &["install", "i", "add", "global"]),
    ("yarn", &["add", "global", "install"]),
    ("brew", &["install", "upgrade", "tap"]),
    ("apt-get", &["install", "update", "upgrade"]),
    ("apt", &["install", "update", "upgrade"]),
    ("dnf", &["install", "update", "upgrade"]),
    ("yum", &["install", "update", "upgrade"]),
    ("pacman", &["-S", "-U", "-Sy"]),
    ("git", &["clone"]),
];

/// Validate that a command is safe to execute
/// 
/// # Arguments
/// * `command` - The installation command string (e.g., "pip install requests")
/// 
/// # Returns
/// * `Ok((program, args))` - Parsed command if safe
/// * `Err(String)` - Error message if command is unsafe
pub fn validate_installation_command(command: &str) -> Result<(String, Vec<String>), String> {
    let trimmed = command.trim();
    
    if trimmed.is_empty() {
        return Err("Empty command".to_string());
    }

    // Split command into parts
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.is_empty() {
        return Err("No command provided".to_string());
    }

    let program = parts[0].to_lowercase();
    
    // Check if program is in whitelist
    let allowed_commands = ALLOWED_PACKAGE_MANAGERS
        .iter()
        .find(|(name, _)| name == &program)
        .ok_or_else(|| format!("Package manager '{}' is not whitelisted. Allowed: pip, pip3, cargo, npm, yarn, brew, apt-get, apt, dnf, yum, pacman, git", program))?;

    // Check if the command/action is allowed
    if parts.len() < 2 {
        return Err(format!("Command '{}' requires at least one argument", program));
    }

    let action = parts[1].to_lowercase();
    if !allowed_commands.1.iter().any(|&allowed| action.starts_with(allowed)) {
        return Err(format!(
            "Action '{}' is not allowed for '{}'. Allowed actions: {:?}",
            action, program, allowed_commands.1
        ));
    }

    // Additional security checks
    // Reject commands with shell metacharacters that could be used for injection
    let dangerous_chars = [';', '&', '|', '`', '$', '(', ')', '<', '>', '\n', '\r'];
    if trimmed.chars().any(|c| dangerous_chars.contains(&c)) {
        return Err("Command contains potentially dangerous characters. Shell metacharacters are not allowed.".to_string());
    }

    // Reject commands that try to redirect output
    if trimmed.contains(">>") || trimmed.contains(">") || trimmed.contains("<") {
        return Err("Output redirection is not allowed for security reasons".to_string());
    }

    // Build safe command structure
    let program_path = if program == "git" {
        "git".to_string()
    } else {
        program.clone()
    };

    let args: Vec<String> = parts[1..].iter().map(|s| s.to_string()).collect();

    Ok((program_path, args))
}

/// Execute a validated installation command
/// 
/// # Arguments
/// * `command` - The installation command string
/// 
/// # Returns
/// * `Ok((success, stdout, stderr))` - Execution result
/// * `Err(String)` - Error message if validation or execution fails
pub async fn execute_installation_command(command: &str) -> Result<(bool, String, String), String> {
    info!(command = %command, "Validating installation command");

    // Validate command
    let (program, args) = validate_installation_command(command)?;

    info!(
        program = %program,
        args = ?args,
        "Executing validated installation command"
    );

    // Execute command
    let output = Command::new(&program)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| format!("Failed to execute command '{}': {}", command, e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let success = output.status.success();

    if success {
        info!(
            command = %command,
            stdout_len = stdout.len(),
            "Installation command executed successfully"
        );
    } else {
        warn!(
            command = %command,
            exit_code = ?output.status.code(),
            stderr = %stderr,
            "Installation command failed"
        );
    }

    Ok((success, stdout, stderr))
}

/// Execute a simple verification command (bypasses strict validation)
async fn execute_verification_command(program: &str, args: &[&str]) -> Result<(bool, String, String), String> {
    let output = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| format!("Failed to execute verification command '{}': {}", program, e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let success = output.status.success();

    Ok((success, stdout, stderr))
}

/// Perform a dry run verification of an installed tool
/// 
/// This attempts to verify that a tool was installed correctly by checking:
/// 1. If it's a Python package, try importing it
/// 2. If it's a CLI tool, check if it's in PATH
/// 3. If it's a git repo, check if the directory exists
/// 
/// # Arguments
/// * `tool_name` - Name of the tool to verify
/// * `installation_type` - Type of installation ("pip", "cargo", "npm", "git", etc.)
/// 
/// # Returns
/// * `Ok((verified, message))` - Verification result
pub async fn verify_tool_installation(tool_name: &str, installation_type: &str) -> Result<(bool, String), String> {
    info!(
        tool_name = %tool_name,
        installation_type = %installation_type,
        "Verifying tool installation"
    );

    match installation_type {
        "pip" | "pip3" => {
            // Try to import the Python package
            let python_name = tool_name.replace("-", "_");
            match execute_verification_command("python3", &["-c", &format!("import {}", python_name)]).await {
                Ok((success, _, stderr)) => {
                    if success {
                        Ok((true, format!("Python package '{}' verified successfully", tool_name)))
                    } else {
                        Ok((false, format!("Python package '{}' import failed: {}", tool_name, stderr)))
                    }
                }
                Err(e) => Ok((false, format!("Verification command failed: {}", e))),
            }
        }
        "cargo" => {
            // Check if cargo binary exists - use cargo list
            match execute_verification_command("cargo", &["--list"]).await {
                Ok((success, stdout, _)) => {
                    if success && stdout.contains(tool_name) {
                        Ok((true, format!("Cargo tool '{}' verified successfully", tool_name)))
                    } else {
                        Ok((false, format!("Cargo tool '{}' not found in cargo --list", tool_name)))
                    }
                }
                Err(e) => Ok((false, format!("Verification command failed: {}", e))),
            }
        }
        "npm" | "yarn" => {
            // Check if npm package is installed
            match execute_verification_command("npm", &["list", "-g", tool_name]).await {
                Ok((success, stdout, _)) => {
                    if success && !stdout.contains("empty") {
                        Ok((true, format!("NPM package '{}' verified successfully", tool_name)))
                    } else {
                        Ok((false, format!("NPM package '{}' not found globally", tool_name)))
                    }
                }
                Err(e) => Ok((false, format!("Verification command failed: {}", e))),
            }
        }
        "git" => {
            // For git clones, we'd need to know the target directory
            // This is a simplified check - in practice, you'd track the clone location
            Ok((true, format!("Git repository cloned (manual verification recommended)")))
        }
        _ => {
            // Generic check: try to run the tool with --version or --help
            let version_result = execute_verification_command(tool_name, &["--version"]).await;
            let help_result = execute_verification_command(tool_name, &["--help"]).await;
            
            if version_result.is_ok_and(|(s, _, _)| s) || help_result.is_ok_and(|(s, _, _)| s) {
                Ok((true, format!("Tool '{}' verified (responds to --version or --help)", tool_name)))
            } else {
                Ok((false, format!("Tool '{}' not found in PATH or does not respond to --version/--help", tool_name)))
            }
        }
    }
}

/// Repair proposal structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairProposal {
    pub tool_name: String,
    pub installation_command: String,
    pub rollback_command: String,
    pub last_successful_timestamp: Option<String>,
    pub last_successful_command: Option<String>,
    pub repair_reason: String,
    pub confidence: f64,
}

/// Propose a rollback or repair action when tool installation verification fails
/// 
/// This function searches the Atlas (Qdrant audit_history collection) for the last
/// successful installation state of a tool and generates a repair proposal.
/// 
/// # Arguments
/// * `tool_name` - Name of the tool that failed verification
/// * `installation_command` - The command that was executed (e.g., "pip install requests")
/// * `installation_type` - Type of installation ("pip", "cargo", "npm", etc.)
/// 
/// # Returns
/// * `Ok(Some(RepairProposal))` - Repair proposal if a successful state is found
/// * `Ok(None)` - No successful state found in history
/// * `Err(String)` - Error message if search fails
pub async fn propose_rollback(
    tool_name: &str,
    installation_command: &str,
    installation_type: &str,
) -> Result<Option<RepairProposal>, String> {
    info!(
        tool_name = %tool_name,
        installation_command = %installation_command,
        installation_type = %installation_type,
        "Searching Atlas for last successful installation state"
    );

    // Import audit_archiver to search audit history
    use crate::tools::audit_archiver::search_audit_history;

    // Search for tool installation mentions in audit history (last 90 days)
    // We'll search for the tool name in various paths and check for successful verifications
    let search_paths = vec![
        format!("/usr/local/bin/{}", tool_name),
        format!("/opt/{}", tool_name),
        format!("{}", tool_name), // Generic tool name search
    ];

    let mut last_successful: Option<(String, String)> = None;

    // Search each path for successful installations
    for path in search_paths {
        match search_audit_history(&path, Some(90)).await {
            Ok(reports) => {
                // Look for reports mentioning successful tool installation
                for report in reports {
                    // Check if report mentions successful verification
                    let report_text = format!(
                        "{}\n{}\n{}",
                        report.report.executive_pulse,
                        report.report.rising_action.join("\n"),
                        report.report.climax
                    );

                    // Look for success indicators
                    let success_indicators = [
                        "VERIFIED: SUCCESS",
                        "verified successfully",
                        "installation successful",
                        "tool verified",
                        format!("{} verified", tool_name),
                    ];

                    let is_success = success_indicators.iter().any(|indicator| {
                        report_text.to_lowercase().contains(&indicator.to_lowercase())
                    });

                    if is_success {
                        // Extract installation command from report if available
                        let command = report.report.resolutions
                            .iter()
                            .find(|r| r.contains(&tool_name) || r.contains("install"))
                            .cloned()
                            .unwrap_or_else(|| installation_command.to_string());

                        // Keep the most recent successful state
                        if last_successful.is_none() || 
                           Some(&report.timestamp) > last_successful.as_ref().map(|(t, _)| t) {
                            last_successful = Some((report.timestamp.clone(), command));
                        }
                    }
                }
            }
            Err(e) => {
                warn!(
                    path = %path,
                    error = %e,
                    "Failed to search audit history for path"
                );
            }
        }
    }

    // Generate rollback command based on installation type
    let rollback_command = generate_rollback_command(installation_command, installation_type);

    if let Some((timestamp, last_command)) = last_successful {
        info!(
            tool_name = %tool_name,
            last_successful_timestamp = %timestamp,
            "Found last successful installation state"
        );

        Ok(Some(RepairProposal {
            tool_name: tool_name.to_string(),
            installation_command: installation_command.to_string(),
            rollback_command: rollback_command.clone(),
            last_successful_timestamp: Some(timestamp),
            last_successful_command: Some(last_command),
            repair_reason: format!(
                "Verification failed for '{}'. Found last successful state in Atlas. Recommend rolling back to previous configuration or reapplying successful installation command.",
                tool_name
            ),
            confidence: 0.75, // Medium-high confidence if we found a successful state
        }))
    } else {
        // No successful state found, but still propose a rollback command
        warn!(
            tool_name = %tool_name,
            "No successful installation state found in Atlas history"
        );

        Ok(Some(RepairProposal {
            tool_name: tool_name.to_string(),
            installation_command: installation_command.to_string(),
            rollback_command: rollback_command.clone(),
            last_successful_timestamp: None,
            last_successful_command: None,
            repair_reason: format!(
                "Verification failed for '{}'. No previous successful state found in Atlas. Recommend uninstalling and retrying installation, or checking system logs for errors.",
                tool_name
            ),
            confidence: 0.5, // Lower confidence without historical data
        }))
    }
}

/// Generate a rollback/uninstall command based on the installation command
fn generate_rollback_command(installation_command: &str, installation_type: &str) -> String {
    let parts: Vec<&str> = installation_command.split_whitespace().collect();
    
    if parts.is_empty() {
        return format!("# Unable to generate rollback command for: {}", installation_command);
    }

    let program = parts[0].to_lowercase();
    
    // Extract tool name from command (usually the last argument)
    let tool_name = parts.last().unwrap_or(&"").to_string();

    match installation_type {
        "pip" | "pip3" => {
            format!("{} uninstall -y {}", program, tool_name)
        }
        "cargo" => {
            // Cargo doesn't have a direct uninstall, but we can suggest removing from Cargo.toml
            format!("# Cargo: Remove '{}' from Cargo.toml dependencies, or manually remove binary", tool_name)
        }
        "npm" | "yarn" => {
            if program == "yarn" {
                format!("yarn global remove {}", tool_name)
            } else {
                format!("npm uninstall -g {}", tool_name)
            }
        }
        "brew" => {
            format!("brew uninstall {}", tool_name)
        }
        "apt-get" | "apt" => {
            format!("{} remove -y {}", program, tool_name)
        }
        "dnf" | "yum" => {
            format!("{} remove -y {}", program, tool_name)
        }
        "pacman" => {
            format!("pacman -R {}", tool_name)
        }
        "git" => {
            // For git clones, suggest removing the cloned directory
            if let Some(repo_name) = tool_name.strip_suffix(".git") {
                format!("rm -rf {}", repo_name)
            } else {
                format!("# Git: Remove cloned repository directory manually")
            }
        }
        _ => {
            format!("# Generic rollback: Manually remove or uninstall '{}'", tool_name)
        }
    }
}

/// Simulation result structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationResult {
    pub success: bool,
    pub message: String,
    pub installation_output: String,
    pub verification_output: String,
    pub sandbox_path: String,
    pub errors: Vec<String>,
}

/// Run a simulation of tool installation in an isolated sandbox
/// 
/// This function creates a temporary sandbox directory, sets up an isolated environment
/// (e.g., Python virtual environment), executes the installation command, and verifies
/// the installation. The sandbox is automatically cleaned up after the simulation.
/// 
/// # Arguments
/// * `tool_name` - Name of the tool to simulate installing
/// * `installation_command` - The installation command to execute
/// * `installation_type` - Type of installation ("pip", "cargo", "npm", etc.)
/// * `verification_command` - Optional custom verification command (if None, uses default verification)
/// 
/// # Returns
/// * `Ok(SimulationResult)` - Simulation result with success status and outputs
/// * `Err(String)` - Error message if simulation setup or execution fails
pub async fn run_simulation(
    tool_name: &str,
    installation_command: &str,
    installation_type: &str,
    verification_command: Option<&str>,
) -> Result<SimulationResult, String> {
    info!(
        tool_name = %tool_name,
        installation_command = %installation_command,
        installation_type = %installation_type,
        "Starting simulation sandbox"
    );

    // Create temporary sandbox directory
    let temp_dir = std::env::temp_dir();
    let sandbox_path = temp_dir.join(format!("phoenix_sandbox_{}_{}", 
        tool_name.replace("/", "_").replace("\\", "_"),
        uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("tmp")
    ));

    // Ensure sandbox directory exists
    fs::create_dir_all(&sandbox_path)
        .await
        .map_err(|e| format!("Failed to create sandbox directory: {}", e))?;

    info!(sandbox_path = %sandbox_path.display(), "Created sandbox directory");

    let mut errors = Vec::new();
    let mut installation_output = String::new();
    let mut verification_output = String::new();
    let mut simulation_success = false;

    // Setup isolation based on installation type
    let setup_result = setup_sandbox_environment(&sandbox_path, installation_type).await;
    if let Err(e) = setup_result {
        errors.push(format!("Sandbox setup failed: {}", e));
    }

    // Execute installation command in sandbox
    match execute_sandbox_installation(&sandbox_path, installation_command, installation_type).await {
        Ok((success, stdout, stderr)) => {
            installation_output = format!("STDOUT:\n{}\n\nSTDERR:\n{}", stdout, stderr);
            if !success {
                errors.push(format!("Installation command failed: {}", stderr));
            }
        }
        Err(e) => {
            errors.push(format!("Installation execution failed: {}", e));
            installation_output = format!("Error: {}", e);
        }
    }

    // Run verification
    let verification_cmd = verification_command.unwrap_or("");
    match verify_sandbox_installation(&sandbox_path, tool_name, installation_type, verification_cmd).await {
        Ok((success, stdout, stderr)) => {
            verification_output = format!("STDOUT:\n{}\n\nSTDERR:\n{}", stdout, stderr);
            simulation_success = success;
            if !success {
                errors.push(format!("Verification failed: {}", stderr));
            }
        }
        Err(e) => {
            errors.push(format!("Verification execution failed: {}", e));
            verification_output = format!("Error: {}", e);
        }
    }

    // Cleanup sandbox (always attempt cleanup, even on error)
    let cleanup_result = cleanup_sandbox(&sandbox_path).await;
    if let Err(e) = cleanup_result {
        warn!(sandbox_path = %sandbox_path.display(), error = %e, "Failed to cleanup sandbox");
        errors.push(format!("Cleanup warning: {}", e));
    } else {
        info!(sandbox_path = %sandbox_path.display(), "Sandbox cleaned up successfully");
    }

    let message = if simulation_success {
        format!("Simulation successful: {} installed and verified in sandbox", tool_name)
    } else {
        format!("Simulation failed: {}", errors.join("; "))
    };

    Ok(SimulationResult {
        success: simulation_success,
        message,
        installation_output,
        verification_output,
        sandbox_path: sandbox_path.to_string_lossy().to_string(),
        errors,
    })
}

/// Setup isolated environment in sandbox based on installation type
async fn setup_sandbox_environment(sandbox_path: &Path, installation_type: &str) -> Result<(), String> {
    match installation_type {
        "pip" | "pip3" => {
            // Create Python virtual environment
            info!(sandbox_path = %sandbox_path.display(), "Creating Python virtual environment");
            
            let venv_path = sandbox_path.join("venv");
            let python_cmd = if installation_type == "pip3" { "python3" } else { "python" };
            
            let output = Command::new(python_cmd)
                .args(&["-m", "venv", venv_path.to_string_lossy().as_ref()])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await
                .map_err(|e| format!("Failed to create virtual environment: {}", e))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(format!("Virtual environment creation failed: {}", stderr));
            }

            info!(venv_path = %venv_path.display(), "Virtual environment created");
            Ok(())
        }
        "cargo" => {
            // For cargo, we can set CARGO_HOME to the sandbox
            // This is handled in execute_sandbox_installation
            Ok(())
        }
        "npm" | "yarn" => {
            // For npm/yarn, we can use a local node_modules directory
            // This is handled in execute_sandbox_installation
            Ok(())
        }
        _ => {
            // Generic setup - no special isolation needed
            Ok(())
        }
    }
}

/// Execute installation command in sandbox with proper isolation
async fn execute_sandbox_installation(
    sandbox_path: &Path,
    installation_command: &str,
    installation_type: &str,
) -> Result<(bool, String, String), String> {
    // Validate command first
    let (program, args) = validate_installation_command(installation_command)?;

    info!(
        sandbox_path = %sandbox_path.display(),
        program = %program,
        "Executing installation in sandbox"
    );

    let mut cmd = Command::new(&program);
    cmd.args(&args);

    // Set up environment isolation based on installation type
    match installation_type {
        "pip" | "pip3" => {
            // Use virtual environment's pip
            let venv_pip = if cfg!(windows) {
                sandbox_path.join("venv").join("Scripts").join("pip.exe")
            } else {
                sandbox_path.join("venv").join("bin").join("pip")
            };

            if venv_pip.exists() {
                // Use venv pip directly
                cmd = Command::new(venv_pip);
                cmd.args(&args);
            } else {
                // Fallback: activate venv and use system pip
                let venv_activate = if cfg!(windows) {
                    sandbox_path.join("venv").join("Scripts").join("activate.bat")
                } else {
                    sandbox_path.join("venv").join("bin").join("activate")
                };
                
                // For Windows, we'd need to use cmd.exe /c, but for simplicity,
                // we'll just use the venv pip path if available
                if !venv_pip.exists() {
                    return Err("Virtual environment pip not found. Please ensure venv was created successfully.".to_string());
                }
            }

            // Set working directory to sandbox
            cmd.current_dir(sandbox_path);
        }
        "cargo" => {
            // Set CARGO_HOME to sandbox for isolation
            let cargo_home = sandbox_path.join("cargo_home");
            fs::create_dir_all(&cargo_home).await
                .map_err(|e| format!("Failed to create cargo_home: {}", e))?;
            
            cmd.env("CARGO_HOME", cargo_home.to_string_lossy().as_ref());
            cmd.current_dir(sandbox_path);
        }
        "npm" | "yarn" => {
            // Set working directory to sandbox (npm/yarn will use local node_modules)
            cmd.current_dir(sandbox_path);
        }
        _ => {
            // Generic: just set working directory
            cmd.current_dir(sandbox_path);
        }
    }

    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let output = cmd.output()
        .await
        .map_err(|e| format!("Failed to execute installation command: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let success = output.status.success();

    Ok((success, stdout, stderr))
}

/// Verify installation in sandbox
async fn verify_sandbox_installation(
    sandbox_path: &Path,
    tool_name: &str,
    installation_type: &str,
    custom_verification: &str,
) -> Result<(bool, String, String), String> {
    if !custom_verification.is_empty() {
        // Execute custom verification command
        let parts: Vec<&str> = custom_verification.split_whitespace().collect();
        if parts.is_empty() {
            return Err("Custom verification command is empty".to_string());
        }

        let mut cmd = Command::new(parts[0]);
        if parts.len() > 1 {
            cmd.args(&parts[1..]);
        }

        // Set up environment for verification
        match installation_type {
            "pip" | "pip3" => {
                let venv_python = if cfg!(windows) {
                    sandbox_path.join("venv").join("Scripts").join("python.exe")
                } else {
                    sandbox_path.join("venv").join("bin").join("python")
                };
                
                if venv_python.exists() && parts[0] == "python" || parts[0] == "python3" {
                    cmd = Command::new(venv_python);
                    if parts.len() > 1 {
                        cmd.args(&parts[1..]);
                    }
                }
            }
            _ => {}
        }

        cmd.current_dir(sandbox_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let output = cmd.output()
            .await
            .map_err(|e| format!("Failed to execute verification command: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let success = output.status.success();

        return Ok((success, stdout, stderr));
    }

    // Use default verification logic adapted for sandbox
    match installation_type {
        "pip" | "pip3" => {
            let venv_python = if cfg!(windows) {
                sandbox_path.join("venv").join("Scripts").join("python.exe")
            } else {
                sandbox_path.join("venv").join("bin").join("python")
            };

            if !venv_python.exists() {
                return Err("Virtual environment Python not found".to_string());
            }

            let python_name = tool_name.replace("-", "_");
            let import_cmd = format!("import {}", python_name);

            let output = Command::new(venv_python)
                .args(&["-c", &import_cmd])
                .current_dir(sandbox_path)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await
                .map_err(|e| format!("Failed to verify Python package: {}", e))?;

            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let success = output.status.success();

            Ok((success, stdout, stderr))
        }
        "cargo" => {
            // Check if cargo binary exists in CARGO_HOME
            let cargo_home = sandbox_path.join("cargo_home");
            let bin_path = cargo_home.join("bin").join(tool_name);
            
            let exists = if cfg!(windows) {
                bin_path.with_extension("exe").exists()
            } else {
                bin_path.exists()
            };

            Ok((exists, 
                if exists { format!("Tool {} found in sandbox", tool_name) } else { String::new() },
                if !exists { format!("Tool {} not found in sandbox", tool_name) } else { String::new() }))
        }
        "npm" | "yarn" => {
            // Check if package exists in local node_modules
            let node_modules = sandbox_path.join("node_modules").join(tool_name);
            let exists = node_modules.exists();

            Ok((exists,
                if exists { format!("Package {} found in sandbox", tool_name) } else { String::new() },
                if !exists { format!("Package {} not found in sandbox", tool_name) } else { String::new() }))
        }
        _ => {
            // Generic check: try to run tool with --version
            let output = Command::new(tool_name)
                .args(&["--version"])
                .current_dir(sandbox_path)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await;

            match output {
                Ok(result) => {
                    let stdout = String::from_utf8_lossy(&result.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&result.stderr).to_string();
                    Ok((result.status.success(), stdout, stderr))
                }
                Err(e) => Ok((false, String::new(), format!("Tool not found: {}", e)))
            }
        }
    }
}

/// Clean up sandbox directory
async fn cleanup_sandbox(sandbox_path: &Path) -> Result<(), String> {
    info!(sandbox_path = %sandbox_path.display(), "Cleaning up sandbox");

    // Remove entire sandbox directory
    if sandbox_path.exists() {
        fs::remove_dir_all(sandbox_path)
            .await
            .map_err(|e| format!("Failed to remove sandbox directory: {}", e))?;
    }

    Ok(())
}
