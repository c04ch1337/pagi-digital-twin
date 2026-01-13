use std::process::Stdio;
use tokio::process::Command;
use tracing::{info, warn};

/// Execute a shell command and return its output.
///
/// # Arguments
/// * `cmd` - The command string to execute (e.g., "ls -la" or "journalctl -u telemetry")
///
/// # Returns
/// * `Ok(String)` - The combined stdout and stderr output
/// * `Err(String)` - Error message if command execution fails
pub async fn run_command(cmd: String) -> Result<String, String> {
    info!(cmd = %cmd, "Executing system command");

    // Determine the shell based on the platform
    let (shell, shell_flag) = if cfg!(target_os = "windows") {
        ("cmd", "/C")
    } else {
        ("sh", "-c")
    };

    let output = Command::new(shell)
        .arg(shell_flag)
        .arg(&cmd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| format!("Failed to execute command '{}': {}", cmd, e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        let error_msg = if !stderr.is_empty() {
            format!("Command failed with exit code {}: {}", output.status.code().unwrap_or(-1), stderr)
        } else {
            format!("Command failed with exit code {}", output.status.code().unwrap_or(-1))
        };
        warn!(cmd = %cmd, error = %error_msg, "Command execution failed");
        return Err(error_msg);
    }

    // Combine stdout and stderr for complete output
    let combined = if stderr.is_empty() {
        stdout.to_string()
    } else {
        format!("{}\n{}", stdout, stderr)
    };

    info!(cmd = %cmd, output_len = combined.len(), "Command executed successfully");
    Ok(combined.trim().to_string())
}

/// Read a file from the filesystem.
///
/// # Arguments
/// * `path` - The file path to read (absolute or relative)
///
/// # Returns
/// * `Ok(String)` - The file contents
/// * `Err(String)` - Error message if file read fails
pub async fn read_file(path: String) -> Result<String, String> {
    info!(path = %path, "Reading file");

    tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| format!("Failed to read file '{}': {}", path, e))
}

/// Write content to a file, creating it if it doesn't exist or overwriting if it does.
///
/// # Arguments
/// * `path` - The file path to write (absolute or relative)
/// * `content` - The content to write to the file
///
/// # Returns
/// * `Ok(())` - Success
/// * `Err(String)` - Error message if file write fails
pub async fn write_file(path: String, content: String) -> Result<(), String> {
    info!(path = %path, content_len = content.len(), "Writing file");

    // Create parent directories if they don't exist
    if let Some(parent) = std::path::Path::new(&path).parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Failed to create parent directories for '{}': {}", path, e))?;
    }

    tokio::fs::write(&path, content)
        .await
        .map_err(|e| format!("Failed to write file '{}': {}", path, e))?;

    info!(path = %path, "File written successfully");
    Ok(())
}

/// Cross-platform service management abstraction.
/// Manages services using the appropriate tool for each operating system.
/// This ensures the AI doesn't get "stuck" when moving between research nodes.
///
/// # Arguments
/// * `name` - The service name (e.g., "telemetry", "gateway")
/// * `action` - The action to perform: "start", "stop", "restart", "status", "enable", "disable"
///
/// # Returns
/// * `Ok(String)` - The command output
/// * `Err(String)` - Error message if command execution fails
pub async fn manage_service(name: &str, action: &str) -> Result<String, String> {
    let valid_actions = ["start", "stop", "restart", "status", "enable", "disable"];
    if !valid_actions.contains(&action) {
        return Err(format!(
            "Invalid service action '{}'. Valid actions: {}",
            action,
            valid_actions.join(", ")
        ));
    }

    info!(os = %detect_os(), action = %action, service = %name, "Managing service");

    let cmd = if cfg!(target_os = "linux") {
        // Linux: Use systemctl with sudo (requires sudoers configuration)
        // For unlimited access: agi-orchestrator ALL=(ALL) NOPASSWD: /usr/bin/systemctl *, /usr/bin/journalctl *
        format!("sudo systemctl {} {}", action, name)
    } else if cfg!(target_os = "windows") {
        // Windows: Use sc.exe for service management (requires Administrator privileges)
        // Alternative: Can use service-manager crate for programmatic control
        match action {
            "start" => format!("sc start {}", name),
            "stop" => format!("sc stop {}", name),
            "restart" => {
                // Windows doesn't have a direct restart, so stop then start
                let stop_result = run_command(format!("sc stop {}", name)).await;
                if let Err(e) = stop_result {
                    return Err(format!("Failed to stop service: {}", e));
                }
                // Wait a moment for service to stop
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                format!("sc start {}", name)
            }
            "status" => format!("sc query {}", name),
            "enable" => format!("sc config {} start= auto", name),
            "disable" => format!("sc config {} start= disabled", name),
            _ => return Err(format!("Unsupported action for Windows: {}", action)),
        }
    } else if cfg!(target_os = "macos") {
        // macOS: Use launchctl (requires sudoers configuration)
        match action {
            "start" | "restart" => format!("sudo launchctl kickstart -k system/{}", name),
            "stop" => format!("sudo launchctl bootout system/{}", name),
            "status" => format!("sudo launchctl print system/{}", name),
            "enable" => format!("sudo launchctl bootstrap system /Library/LaunchDaemons/{}.plist", name),
            "disable" => format!("sudo launchctl bootout system/{}", name),
            _ => return Err(format!("Unsupported action for macOS: {}", action)),
        }
    } else {
        return Err(format!("Unsupported operating system: {}", detect_os()));
    };

    run_command(cmd).await
}

/// Cross-platform log retrieval abstraction.
/// Retrieves service logs using the appropriate tool for each operating system.
/// This ensures consistent log access across different research nodes.
///
/// # Arguments
/// * `name` - The service name (e.g., "telemetry", "gateway")
///
/// # Returns
/// * `Ok(String)` - The log output (last 50 entries on Linux, last 5 minutes on macOS, recent events on Windows)
/// * `Err(String)` - Error message if log retrieval fails
pub async fn get_logs(name: &str) -> Result<String, String> {
    info!(os = %detect_os(), service = %name, "Retrieving service logs");

    let cmd = if cfg!(target_os = "linux") {
        // Linux: Use journalctl with sudo (requires sudoers configuration)
        // For unlimited access: agi-orchestrator ALL=(ALL) NOPASSWD: /usr/bin/systemctl *, /usr/bin/journalctl *
        format!("sudo journalctl -u {} -n 50", name)
    } else if cfg!(target_os = "windows") {
        // Windows: Use PowerShell Get-EventLog (requires Administrator privileges)
        // Note: This requires the service to be registered as an event source
        // For services that don't log to Event Log, we might need to check log files directly
        format!(
            "powershell -Command \"Get-EventLog -LogName System -Source {} -Newest 50 -ErrorAction SilentlyContinue | Format-Table -AutoSize\"",
            name
        )
    } else if cfg!(target_os = "macos") {
        // macOS: Use log show with sudo (requires sudoers configuration)
        format!("sudo log show --predicate 'process == \"{}\"' --last 5m", name)
    } else {
        return Err(format!("Unsupported operating system: {}", detect_os()));
    };

    run_command(cmd).await
}

/// Detect the current operating system.
///
/// # Returns
/// * `String` - The OS name ("linux", "windows", "macos", or "unknown")
fn detect_os() -> String {
    if cfg!(target_os = "linux") {
        "linux".to_string()
    } else if cfg!(target_os = "windows") {
        "windows".to_string()
    } else if cfg!(target_os = "macos") {
        "macos".to_string()
    } else {
        "unknown".to_string()
    }
}

/// Control systemd services using systemctl with sudo.
/// This is a legacy function maintained for backward compatibility.
/// New code should use `manage_service` for cross-platform support.
///
/// # Arguments
/// * `action` - The action to perform: "start", "stop", "restart", "status", or "enable"
/// * `service_name` - The name of the systemd service (e.g., "telemetry", "sys_control")
///
/// # Returns
/// * `Ok(String)` - The command output
/// * `Err(String)` - Error message if command execution fails
pub async fn systemctl(action: String, service_name: String) -> Result<String, String> {
    // Delegate to the cross-platform manage_service function
    manage_service(&service_name, &action).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_read_file_nonexistent() {
        let result = read_file("/nonexistent/path/file.txt".to_string()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_write_and_read_file() {
        let test_path = if cfg!(target_os = "windows") {
            "C:\\temp\\test_system_tools.txt"
        } else {
            "/tmp/test_system_tools.txt"
        };
        let test_content = "Hello, World!";

        // Write file
        let write_result = write_file(test_path.to_string(), test_content.to_string()).await;
        assert!(write_result.is_ok());

        // Read file
        let read_result = read_file(test_path.to_string()).await;
        assert!(read_result.is_ok());
        assert_eq!(read_result.unwrap(), test_content);

        // Cleanup
        let _ = tokio::fs::remove_file(test_path).await;
    }

    #[tokio::test]
    async fn test_run_command_echo() {
        let cmd = if cfg!(target_os = "windows") {
            "echo test"
        } else {
            "echo test"
        };
        let result = run_command(cmd.to_string()).await;
        assert!(result.is_ok());
        assert!(result.unwrap().contains("test"));
    }

    #[test]
    fn test_detect_os() {
        let os = detect_os();
        assert!(!os.is_empty());
        assert!(os == "linux" || os == "windows" || os == "macos" || os == "unknown");
    }
}
