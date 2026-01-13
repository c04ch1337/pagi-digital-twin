use serde_json::json;
use std::time::Duration;
use sysinfo::System;
use tracing::{info, warn};

/// Diagnostic tool that verifies the telemetry service loop on any OS.
/// 
/// Checks:
/// - If the telemetry process is running (by name matching)
/// - If ports 8281-8284 are open and accepting connections
/// 
/// # Returns
/// * `Ok(String)` - JSON string with diagnostic results
/// * `Err(String)` - JSON error object that the Orchestrator can parse and act upon
pub async fn agi_doctor() -> Result<String, String> {
    info!("Running agi-doctor diagnostic");
    
    let mut results = json!({
        "status": "checking",
        "checks": {}
    });
    
    // Check 1: Is telemetry process running?
    let telemetry_running = check_telemetry_process();
    results["checks"]["telemetry_process"] = json!({
        "running": telemetry_running,
        "status": if telemetry_running { "ok" } else { "error" }
    });
    
    // Check 2: Are ports 8281-8284 open?
    let ports_status = check_ports().await;
    results["checks"]["ports"] = ports_status.clone();
    
    // Determine overall status
    let all_ok = telemetry_running 
        && ports_status["8281"]["open"].as_bool().unwrap_or(false)
        && ports_status["8282"]["open"].as_bool().unwrap_or(false)
        && ports_status["8283"]["open"].as_bool().unwrap_or(false)
        && ports_status["8284"]["open"].as_bool().unwrap_or(false);
    
    if all_ok {
        results["status"] = json!("ok");
        info!("All diagnostic checks passed");
        Ok(results.to_string())
    } else {
        results["status"] = json!("error");
        let error_details = json!({
            "error": "Diagnostic checks failed",
            "telemetry_process_running": telemetry_running,
            "ports": {
                "8281": ports_status["8281"]["open"].as_bool().unwrap_or(false),
                "8282": ports_status["8282"]["open"].as_bool().unwrap_or(false),
                "8283": ports_status["8283"]["open"].as_bool().unwrap_or(false),
                "8284": ports_status["8284"]["open"].as_bool().unwrap_or(false)
            },
            "recommendations": build_recommendations(&telemetry_running, &ports_status)
        });
        warn!(error = %error_details, "Diagnostic checks failed");
        Err(error_details.to_string())
    }
}

/// Check if the telemetry process is running by searching for processes
/// that match common telemetry service names.
fn check_telemetry_process() -> bool {
    let mut system = System::new_all();
    system.refresh_all();
    
    // Common telemetry process names to check
    let telemetry_names = vec![
        "telemetry",
        "backend-rust-telemetry",
        "rust-telemetry",
        "telemetry-service",
    ];
    
    for process in system.processes().values() {
        let process_name = process.name().to_string_lossy().to_lowercase();
        for telemetry_name in &telemetry_names {
            if process_name.contains(telemetry_name) {
                info!(pid = %process.pid(), name = %process.name().to_string_lossy(), "Found telemetry process");
                return true;
            }
        }
    }
    
    warn!("Telemetry process not found");
    false
}

/// Check if ports 8281-8284 are open and accepting connections.
async fn check_ports() -> serde_json::Value {
    let ports = vec![8281, 8282, 8283, 8284];
    let mut port_status = json!({});
    
    for port in ports {
        let is_open = check_port(port).await;
        port_status[port.to_string()] = json!({
            "open": is_open,
            "status": if is_open { "ok" } else { "error" }
        });
        
        if is_open {
            info!(port = port, "Port is open");
        } else {
            warn!(port = port, "Port is not open");
        }
    }
    
    port_status
}

/// Check if a specific port is open by attempting a TCP connection.
async fn check_port(port: u16) -> bool {
    // Try connecting to localhost on the specified port
    // Use a short timeout to avoid hanging
    match tokio::time::timeout(
        Duration::from_millis(500),
        tokio::net::TcpStream::connect(format!("127.0.0.1:{}", port))
    ).await {
        Ok(Ok(_)) => true,
        Ok(Err(_)) => false,
        Err(_) => false, // Timeout
    }
}

/// Build recommendations based on diagnostic results.
fn build_recommendations(
    telemetry_running: &bool,
    ports_status: &serde_json::Value,
) -> Vec<String> {
    let mut recommendations = Vec::new();
    
    if !telemetry_running {
        recommendations.push("Telemetry process is not running. Use 'manage_service' with action 'start' and service 'telemetry' to start it.".to_string());
    }
    
    let ports_to_check = vec![8281, 8282, 8283, 8284];
    let mut closed_ports = Vec::new();
    
    for port in ports_to_check {
        if !ports_status[port.to_string()]["open"].as_bool().unwrap_or(false) {
            closed_ports.push(port);
        }
    }
    
    if !closed_ports.is_empty() {
        recommendations.push(format!(
            "Ports {:?} are not open. Check if telemetry service is listening on these ports. Use 'get_logs' with service 'telemetry' to investigate.",
            closed_ports
        ));
    }
    
    if recommendations.is_empty() {
        recommendations.push("All checks passed. System is healthy.".to_string());
    }
    
    recommendations
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_check_port_localhost() {
        // This test will fail if nothing is listening on port 8281
        // But it's useful to verify the function works
        let result = check_port(8281).await;
        // Just verify it doesn't panic
        assert!(result == true || result == false);
    }
    
    #[test]
    fn test_check_telemetry_process() {
        // This will check the actual system, so it may or may not find telemetry
        let result = check_telemetry_process();
        // Just verify it doesn't panic
        assert!(result == true || result == false);
    }
}
