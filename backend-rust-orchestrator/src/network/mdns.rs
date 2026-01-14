//! mDNS Service Discovery - "Blue Flame" Network Discovery Layer
//!
//! This module implements mDNS (multicast DNS) service registration and discovery
//! to enable automatic peer discovery on the local network. Nodes announce their
//! presence and discover other "Blue Flame" nodes automatically.

use std::sync::Arc;
use std::time::Duration;
use chrono::Utc;
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use tokio::sync::broadcast;
use tracing::{info, warn, error};

use crate::bus::{GlobalMessageBus, PhoenixEvent};

/// mDNS service manager for Blue Flame network discovery
pub struct MdnsService {
    daemon: ServiceDaemon,
    message_bus: Arc<GlobalMessageBus>,
    node_id: String,
    software_version: String,
    guardrail_version: String,
    handshake_port: u16,
}

impl MdnsService {
    /// Create a new mDNS service manager
    pub fn new(
        message_bus: Arc<GlobalMessageBus>,
        node_id: String,
        software_version: String,
        guardrail_version: String,
        handshake_port: u16,
    ) -> Result<Self, String> {
        let daemon = ServiceDaemon::new()
            .map_err(|e| format!("Failed to create mDNS daemon: {}", e))?;

        Ok(Self {
            daemon,
            message_bus,
            node_id,
            software_version,
            guardrail_version,
            handshake_port,
        })
    }

    /// Start mDNS service registration
    /// Registers this node as a Blue Flame orchestrator on the local network
    pub async fn start_registration(&self) -> Result<(), String> {
        info!(
            node_id = %self.node_id,
            port = self.handshake_port,
            "Starting mDNS service registration"
        );

        // Create short node ID for service name (first 6 chars)
        let node_id_short = if self.node_id.len() >= 6 {
            &self.node_id[..6]
        } else {
            &self.node_id
        };

        let service_type = "_blueflame._tcp.local.";
        let service_name = format!("orchestrator-{}.{}", node_id_short, service_type);

        // Create TXT records with node metadata
        let mut txt_properties = std::collections::HashMap::new();
        txt_properties.insert("node_id".to_string(), self.node_id.clone());
        txt_properties.insert("software_version".to_string(), self.software_version.clone());
        txt_properties.insert("guardrail_version".to_string(), self.guardrail_version.clone());

        // Build service info
        let service_info = ServiceInfo::new(
            service_type,
            &service_name,
            &format!("{}.local.", service_name),
            None, // Host name (auto-detect)
            self.handshake_port,
            Some(&txt_properties),
        )
        .map_err(|e| format!("Failed to create service info: {}", e))?;

        // Register the service
        self.daemon
            .register(service_info)
            .map_err(|e| format!("Failed to register mDNS service: {}", e))?;

        info!(
            service_name = %service_name,
            "mDNS service registered successfully"
        );

        Ok(())
    }

    /// Start mDNS service discovery
    /// Browses for other Blue Flame nodes on the local network
    pub async fn start_discovery(&self) -> Result<(), String> {
        info!("Starting mDNS service discovery");

        let service_type = "_blueflame._tcp.local.";
        let receiver = self
            .daemon
            .browse(service_type)
            .map_err(|e| format!("Failed to start mDNS browsing: {}", e))?;

        // Spawn a task to handle discovery events
        let message_bus = self.message_bus.clone();
        tokio::spawn(async move {
            loop {
                match receiver.recv_async().await {
                    Ok(event) => {
                        match event {
                            ServiceEvent::ServiceResolved(info) => {
                                info!(
                                    service_name = %info.get_fullname(),
                                    "mDNS service resolved"
                                );

                                // Extract IP address
                                let ip_addresses = info.get_addresses();
                                if ip_addresses.is_empty() {
                                    warn!("Service resolved but no IP address found");
                                    continue;
                                }

                                // Use the first IPv4 address
                                let ip = ip_addresses
                                    .iter()
                                    .find(|addr| addr.is_ipv4())
                                    .or_else(|| ip_addresses.first())
                                    .map(|addr| addr.to_string())
                                    .unwrap_or_else(|| "unknown".to_string());

                                // Extract node_id from TXT records
                                let txt_props = info.get_properties();
                                let node_id = txt_props
                                    .get("node_id")
                                    .map(|v| v.to_string())
                                    .unwrap_or_else(|| "unknown".to_string());

                                if node_id != "unknown" {
                                    info!(
                                        node_id = %node_id,
                                        ip = %ip,
                                        "Discovered Blue Flame node"
                                    );

                                    // Publish NodeDiscovered event
                                    let event = PhoenixEvent::NodeDiscovered {
                                        ip: ip.clone(),
                                        node_id: node_id.clone(),
                                        timestamp: Utc::now().to_rfc3339(),
                                    };
                                    message_bus.publish(event);
                                }
                            }
                            ServiceEvent::ServiceFound(service, _fullname) => {
                                info!(
                                    service = %service,
                                    "mDNS service found, resolving..."
                                );
                                // The daemon will automatically resolve it
                            }
                            ServiceEvent::ServiceRemoved(service, _fullname) => {
                                info!(
                                    service = %service,
                                    "mDNS service removed"
                                );
                            }
                            _ => {
                                // Ignore other events
                            }
                        }
                    }
                    Err(e) => {
                        error!(error = %e, "Error receiving mDNS discovery event");
                        // Continue listening despite errors
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                }
            }
        });

        info!("mDNS discovery started successfully");
        Ok(())
    }
}

/// Start mDNS registration and discovery as a long-lived task
pub async fn start_mdns_service(
    message_bus: Arc<GlobalMessageBus>,
    node_id: String,
    software_version: String,
    guardrail_version: String,
    handshake_port: u16,
) -> Result<(), String> {
    let mdns_service = MdnsService::new(
        message_bus,
        node_id,
        software_version,
        guardrail_version,
        handshake_port,
    )?;

    // Start both registration and discovery
    mdns_service.start_registration().await?;
    mdns_service.start_discovery().await?;

    // Keep the service alive
    info!("mDNS service started and running");
    Ok(())
}
