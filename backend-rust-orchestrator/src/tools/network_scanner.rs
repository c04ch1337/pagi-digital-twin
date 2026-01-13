use quick_xml::de::from_str;
use tokio::process::Command as TokioCommand;

use crate::tools::nmap_models::{Address, NmapRun};
use crate::{NetworkScanHost, NetworkScanPort};

fn best_ipv4_address(addrs: &[Address]) -> Option<String> {
    addrs
        .iter()
        .find(|a| a.addr_type == "ipv4")
        .map(|a| a.addr.clone())
}

pub fn parse_nmap_xml_to_hosts(xml: &str) -> Result<Vec<NetworkScanHost>, String> {
    let run: NmapRun = from_str(xml).map_err(|e| format!("XML Parse Error: {e}"))?;

    let mut out = Vec::new();
    for h in run.hosts {
        // Skip hosts that are clearly down.
        if h.status.state != "up" {
            continue;
        }

        let ipv4 = best_ipv4_address(&h.addresses);
        let hostnames = h
            .hostnames
            .map(|hn| {
                hn.hostnames
                    .into_iter()
                    .map(|x| x.name)
                    .filter(|s| !s.trim().is_empty())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let mut ports: Vec<NetworkScanPort> = Vec::new();
        if let Some(p) = h.ports {
            for prt in p.ports {
                let state = prt.state.state;
                if state != "open" {
                    continue;
                }
                ports.push(NetworkScanPort {
                    port: prt.portid,
                    protocol: prt.protocol,
                    state,
                    service: prt.service.map(|s| s.name),
                });
            }
        }

        let is_agi_core_node = ports
            .iter()
            .any(|p| p.protocol == "tcp" && (8281..=8284).contains(&p.port));

        out.push(NetworkScanHost {
            ipv4,
            hostnames,
            ports,
            is_agi_core_node,
        });
    }

    Ok(out)
}

pub async fn run_xml_scan(target: &str, ports: &str) -> Result<Vec<NetworkScanHost>, String> {
    // NOTE:
    // - On Unix, `-sS` requires root; we attempt `sudo -n` (no password prompt).
    // - On Windows, the process must be launched with Administrator privileges.
    let mut cmd = if cfg!(target_os = "windows") {
        let mut c = TokioCommand::new("nmap");
        c.arg("-sS")
            .arg("-T4")
            .arg("-Pn")
            .arg("--max-retries")
            .arg("2")
            .arg("--host-timeout")
            .arg("15s")
            .arg("-p")
            .arg(ports)
            .arg("-oX")
            .arg("-")
            .arg(target);
        c
    } else {
        let mut c = TokioCommand::new("sudo");
        c.arg("-n")
            .arg("nmap")
            .arg("-sS")
            .arg("-T4")
            .arg("-Pn")
            .arg("--max-retries")
            .arg("2")
            .arg("--host-timeout")
            .arg("15s")
            .arg("-p")
            .arg(ports)
            .arg("-oX")
            .arg("-")
            .arg(target);
        c
    };

    cmd.kill_on_drop(true);
    let output = cmd
        .output()
        .await
        .map_err(|e| format!("failed to launch nmap: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        if !cfg!(target_os = "windows") && stderr.to_lowercase().contains("password") {
            return Err(
                "nmap requires elevated privileges. Configure sudoers NOPASSWD for agi-orchestrator (see backend-rust-orchestrator/config/sudoers.*)".to_string(),
            );
        }
        if stderr.to_lowercase().contains("not found") || stderr.to_lowercase().contains("is not recognized") {
            return Err(
                "nmap not found. Install it on the host (Linux: sudo apt install nmap; macOS: brew install nmap; Windows: install Nmap + Npcap).".to_string(),
            );
        }
        return Err(format!("nmap scan failed (status={:?}). stderr: {}", output.status.code(), stderr));
    }

    if stdout.trim().is_empty() {
        return Err(format!("nmap returned empty output. stderr: {}", stderr));
    }

    parse_nmap_xml_to_hosts(&stdout)
}

#[cfg(test)]
mod tests {
    use super::parse_nmap_xml_to_hosts;

    #[test]
    fn parses_minimal_nmap_xml() {
        // NOTE: this is a *raw string literal*, so quotes must NOT be escaped.
        let xml = r#"<?xml version="1.0"?>
<nmaprun>
  <host>
    <status state="up" reason="syn-ack" reason_ttl="64" />
    <address addr="192.168.1.10" addrtype="ipv4" />
    <hostnames>
      <hostname name="lab-node" type="PTR" />
    </hostnames>
    <ports>
      <port protocol="tcp" portid="8282">
        <state state="open" />
        <service name="unknown" />
      </port>
      <port protocol="tcp" portid="22">
        <state state="closed" />
      </port>
    </ports>
  </host>
</nmaprun>"#;

        let hosts = parse_nmap_xml_to_hosts(xml).expect("parse");
        assert_eq!(hosts.len(), 1);
        let h = &hosts[0];
        assert_eq!(h.ipv4.as_deref(), Some("192.168.1.10"));
        assert!(h.is_agi_core_node);
        assert_eq!(h.ports.len(), 1);
        assert_eq!(h.ports[0].port, 8282);
    }
}

