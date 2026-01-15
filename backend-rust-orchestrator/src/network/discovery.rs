//! Automatic Node Discovery via mDNS/Zeroconf
//!
//! This module implements automatic discovery of other Blue Flame nodes
//! on the local network using mDNS service discovery. When a new node is
//! discovered, it automatically triggers a handshake.

use std::sync::Arc;
use std::time::Duration;
use chrono::Utc;
use tokio::sync::RwLock;
use tokio::time::sleep;
use tracing::{info, warn, error};

use crate::bus::{GlobalMessageBus, PhoenixEvent};
// Note: mDNS discovery implementation requires mdns-sd or zeroconf crate
// This is a placeholder structure that can be extended when the crate is added

use crate::handshake_proto::{
    node_handshake_service_client::NodeHandshakeServiceClient,
    HandshakeRequest,
};
use crate::network::handshake::NodeIdentity;
use tonic::transport::Channel;

/// Service name for Blue Flame discovery
const SERVICE_NAME: &str = "_blueflame._tcp.local";
const SERVICE_PORT: u16 = 8285;

/// Discovered service information
#[derive(Debug, Clone)]
struct DiscoveredService {
    hostname: String,
    ip_address: String,
    port: u16,
    discovered_at: String,
}

/// mDNS Discovery Manager
pub struct DiscoveryManager {
    /// Map of discovered services: ip:port -> DiscoveredService
    discovered_services: Arc<RwLock<std::collections::HashMap<String, DiscoveredService>>>,
    message_bus: Arc<GlobalMessageBus>,
    local_identity: Arc<NodeIdentity>,
    software_version: String,
    manifest_hash: String,
    running: Arc<RwLock<bool>>,
}

impl DiscoveryManager {
    pub fn new(
        message_bus: Arc<GlobalMessageBus>,
        local_identity: Arc<NodeIdentity>,
        software_version: String,
        manifest_hash: String,
    ) -> Self {
        Self {
            discovered_services: Arc::new(RwLock::new(std::collections::HashMap::new())),
            message_bus,
            local_identity,
            software_version,
            manifest_hash,
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Start the discovery service (register and browse)
    pub async fn start(&self) -> Result<(), String> {
        let mut running = self.running.write().await;
        if *running {
            return Err("Discovery already running".to_string());
        }
        *running = true;
        drop(running);

        info!("Starting mDNS discovery service");

        // Register our own service
        let register_handle = {
            let identity = self.local_identity.clone();
            let software_version = self.software_version.clone();
            let manifest_hash = self.manifest_hash.clone();
            tokio::spawn(async move {
                Self::register_service(identity, software_version, manifest_hash).await
            })
        };

        // Browse for other services
        let browse_handle = {
            let services = self.discovered_services.clone();
            let message_bus = self.message_bus.clone();
            let identity = self.local_identity.clone();
            let software_version = self.software_version.clone();
            let manifest_hash = self.manifest_hash.clone();
            tokio::spawn(async move {
                Self::browse_services(
                    services,
                    message_bus,
                    identity,
                    software_version,
                    manifest_hash,
                )
                .await
            })
        };

        // Monitor for service timeouts
        let monitor_handle = {
            let services = self.discovered_services.clone();
            let running = self.running.clone();
            tokio::spawn(async move {
                Self::monitor_services(services, running).await
            })
        };

        // Wait for all tasks (they run indefinitely)
        tokio::select! {
            _ = register_handle => {
                warn!("Service registration task ended");
            }
            _ = browse_handle => {
                warn!("Service browsing task ended");
            }
            _ = monitor_handle => {
                warn!("Service monitoring task ended");
            }
        }

        Ok(())
    }

    /// Stop the discovery service
    pub async fn stop(&self) {
        let mut running = self.running.write().await;
        *running = false;
        info!("Stopped mDNS discovery service");
    }

    /// Register this node's service
    async fn register_service(
        identity: Arc<NodeIdentity>,
        software_version: String,
        manifest_hash: String,
    ) -> Result<(), String> {
        // Note: This is a simplified implementation
        // In production, you would use mdns-sd or zeroconf crate
        // For now, we'll log that registration would happen
        info!(
            node_id = %identity.node_id,
            service = %SERVICE_NAME,
            port = SERVICE_PORT,
            "Would register mDNS service (implementation pending mdns-sd crate)"
        );

        // TODO: Implement actual mDNS registration using mdns-sd crate
        // Example:
        // let service = ServiceInfo::new(
        //     SERVICE_NAME,
        //     &identity.node_id,
        //     "local.",
        //     SERVICE_PORT,
        // )?;
        // let mdns = ServiceDaemon::new()?;
        // mdns.register(service)?;

        Ok(())
    }

    /// Browse for other services
    async fn browse_services(
        services: Arc<RwLock<std::collections::HashMap<String, DiscoveredService>>>,
        message_bus: Arc<GlobalMessageBus>,
        identity: Arc<NodeIdentity>,
        software_version: String,
        manifest_hash: String,
    ) -> Result<(), String> {
        info!("Starting service browser");

        // TODO: Implement actual mDNS browsing using mdns-sd crate
        // For now, this is a placeholder that would:
        // 1. Listen for service announcements
        // 2. When a new service is found, add it to discovered_services
        // 3. Trigger handshake

        loop {
            sleep(Duration::from_secs(5)).await;

            // Placeholder: In real implementation, this would be driven by mDNS events
            // For now, we'll just log that browsing is active
            let count = {
                let s = services.read().await;
                s.len()
            };
            if count > 0 {
                info!(count = count, "Active discovered services");
            }
        }
    }

    /// Monitor services and remove stale entries
    async fn monitor_services(
        services: Arc<RwLock<std::collections::HashMap<String, DiscoveredService>>>,
        running: Arc<RwLock<bool>>,
    ) {
        loop {
            sleep(Duration::from_secs(30)).await;

            let is_running = {
                let r = running.read().await;
                *r
            };

            if !is_running {
                break;
            }

            // Remove services that haven't been seen in 5 minutes
            let now = Utc::now();
            let mut to_remove = Vec::new();

            {
                let s = services.read().await;
                for (key, service) in s.iter() {
                    if let Ok(discovered_at) = chrono::DateTime::parse_from_rfc3339(&service.discovered_at) {
                        let age = now.signed_duration_since(discovered_at);
                        if age.num_seconds() > 300 {
                            to_remove.push(key.clone());
                        }
                    }
                }
            }

            if !to_remove.is_empty() {
                let mut s = services.write().await;
                for key in to_remove {
                    s.remove(&key);
                    info!(service = %key, "Removed stale service");
                }
            }
        }
    }

    /// Manually trigger handshake with a discovered service
    pub async fn initiate_handshake_to(
        &self,
        ip_address: &str,
        port: u16,
    ) -> Result<(), String> {
        info!(ip = %ip_address, port = port, "Initiating handshake to discovered service");

        let endpoint = format!("http://{}:{}", ip_address, port);
        let channel = Channel::from_shared(endpoint)
            .map_err(|e| format!("Invalid endpoint: {}", e))?
            .connect()
            .await
            .map_err(|e| format!("Failed to connect: {}", e))?;

        let mut client = NodeHandshakeServiceClient::new(channel);

        // Create handshake request
        let request = HandshakeRequest {
            node_id: self.local_identity.node_id.clone(),
            software_version: self.software_version.clone(),
            manifest_hash: self.manifest_hash.clone(),
        };

        // Initiate handshake
        match client.initiate_handshake(request).await {
            Ok(response) => {
                info!(ip = %ip_address, "Handshake initiated successfully");
                // TODO: Complete the handshake flow
                Ok(())
            }
            Err(e) => {
                error!(ip = %ip_address, error = %e, "Handshake initiation failed");
                Err(format!("Handshake failed: {}", e))
            }
        }
    }

    /// Get list of discovered services
    pub async fn get_discovered_services(&self) -> Vec<DiscoveredService> {
        let services = self.discovered_services.read().await;
        services.values().cloned().collect()
    }
}
