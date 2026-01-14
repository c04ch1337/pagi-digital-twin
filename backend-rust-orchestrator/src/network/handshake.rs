//! Node Handshake Protocol - "Blue Flame" P2P Verification Layer
//!
//! This module implements the Zero Trust handshake protocol that ensures nodes
//! are aligned with the same values, guardrails, and system prompts before
//! allowing them to join the decentralized AGI network.
//!
//! Handshake Flow:
//! 1. Initiation: Node A sends HandshakeRequest (NodeID, SoftwareVersion, ManifestHash)
//! 2. Challenge: Node B responds with a nonce that must be signed
//! 3. Verification: Both nodes exchange Alignment Token (hash of system_prompt + KB)
//! 4. Validation: If validation fails, connection is dropped and event is broadcast

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use chrono::Utc;
use ed25519_dalek::{Signer, Verifier, Signature, SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use rand::RngCore;
use sha2::{Sha256, Digest};
use tokio::sync::RwLock;
use tonic::{Request, Response, Status};
use tracing::{info, warn, error};

use crate::bus::{GlobalMessageBus, PhoenixEvent};
use crate::memory_client::memory_service_client::MemoryServiceClient;
use crate::memory_client::CommitMemoryRequest;
use tonic::transport::Channel;

// Include generated proto code
use crate::handshake_proto::{
    node_handshake_service_server::{NodeHandshakeService, NodeHandshakeServiceServer},
    HandshakeRequest, HandshakeChallenge, HandshakeResponse, HandshakeResult,
    PropagateQuarantineRequest, PropagateQuarantineResponse,
};

/// Node identity and keys
#[derive(Clone)]
pub struct NodeIdentity {
    pub node_id: String,
    pub signing_key: SigningKey,
    pub verifying_key: VerifyingKey,
}

impl NodeIdentity {
    /// Generate a new node identity
    pub fn new(node_id: String) -> Self {
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let verifying_key = signing_key.verifying_key();
        
        Self {
            node_id,
            signing_key,
            verifying_key,
        }
    }

    /// Load or create node identity from disk
    pub async fn load_or_create(node_id: String, key_path: PathBuf) -> Result<Self, String> {
        if key_path.exists() {
            // Load existing key
            let key_bytes = tokio::fs::read(&key_path)
                .await
                .map_err(|e| format!("Failed to read key file: {}", e))?;
            
            if key_bytes.len() != 32 {
                return Err("Invalid key file size".to_string());
            }

            let mut key_array = [0u8; 32];
            key_array.copy_from_slice(&key_bytes);
            let signing_key = SigningKey::from_bytes(&key_array);
            let verifying_key = signing_key.verifying_key();

            Ok(Self {
                node_id,
                signing_key,
                verifying_key,
            })
        } else {
            // Create new identity
            let identity = Self::new(node_id.clone());
            
            // Save key to disk
            let key_dir = key_path.parent().ok_or("Invalid key path")?;
            tokio::fs::create_dir_all(key_dir)
                .await
                .map_err(|e| format!("Failed to create key directory: {}", e))?;
            
            tokio::fs::write(&key_path, identity.signing_key.to_bytes())
                .await
                .map_err(|e| format!("Failed to write key file: {}", e))?;

            info!(node_id = %node_id, "Generated new node identity");
            Ok(identity)
        }
    }
}

/// Handshake state for pending challenges
#[derive(Clone)]
struct PendingChallenge {
    nonce: Vec<u8>,
    timestamp: i64,
    node_id: String,
    software_version: String,
    manifest_hash: String,
}

/// Peer node information
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PeerNode {
    pub node_id: String,
    pub software_version: String,
    pub manifest_hash: String,
    pub remote_address: String,
    pub status: PeerStatus,
    pub last_seen: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum PeerStatus {
    Verified,
    Pending,
    Quarantined,
}

/// Node Handshake Service Implementation
pub struct NodeHandshakeServiceImpl {
    pub identity: Arc<NodeIdentity>,
    system_prompt_path: PathBuf,
    leadership_kb_path: Option<PathBuf>,
    manifest_path: Option<PathBuf>,
    software_version: String,
    guardrail_version: String,
    message_bus: Arc<GlobalMessageBus>,
    memory_client: Option<Arc<MemoryServiceClient<Channel>>>,
    qdrant_client: Option<Arc<qdrant_client::Qdrant>>,
    pending_challenges: Arc<RwLock<HashMap<String, PendingChallenge>>>,
    /// Map of verified peers: node_id -> PeerNode
    verified_peers: Arc<RwLock<HashMap<String, PeerNode>>>,
    /// Quarantine manager (optional, for checking quarantine status)
    quarantine_manager: Option<Arc<crate::network::quarantine::QuarantineManager>>,
    /// Immune response system for handling compliance alerts
    immune_response: Option<Arc<crate::network::immune_system::GlobalImmuneResponse>>,
}

impl NodeHandshakeServiceImpl {
    pub fn new(
        identity: Arc<NodeIdentity>,
        system_prompt_path: PathBuf,
        leadership_kb_path: Option<PathBuf>,
        manifest_path: Option<PathBuf>,
        software_version: String,
        guardrail_version: String,
        message_bus: Arc<GlobalMessageBus>,
        memory_client: Option<Arc<MemoryServiceClient<Channel>>>,
        qdrant_client: Option<Arc<qdrant_client::Qdrant>>,
        quarantine_manager: Option<Arc<crate::network::quarantine::QuarantineManager>>,
    ) -> Self {
        Self {
            identity,
            system_prompt_path,
            leadership_kb_path,
            manifest_path,
            software_version,
            guardrail_version,
            message_bus,
            memory_client,
            qdrant_client,
            pending_challenges: Arc::new(RwLock::new(HashMap::new())),
            verified_peers: Arc::new(RwLock::new(HashMap::new())),
            quarantine_manager,
            immune_response: None,
        }
    }

    /// Set the immune response system
    pub fn set_immune_response(&mut self, immune_response: Arc<crate::network::immune_system::GlobalImmuneResponse>) {
        self.immune_response = Some(immune_response);
    }

    /// Get all verified peers
    pub async fn get_verified_peers(&self) -> Vec<PeerNode> {
        let peers = self.verified_peers.read().await;
        peers.values().cloned().collect()
    }

    /// Get a specific peer by node_id
    pub async fn get_peer(&self, node_id: &str) -> Option<PeerNode> {
        let peers = self.verified_peers.read().await;
        peers.get(node_id).cloned()
    }

    /// Compute alignment token (hash of system_prompt + leadership KB)
    async fn compute_alignment_token(&self) -> Result<String, String> {
        let mut hasher = Sha256::new();

        // Hash system prompt
        let system_prompt = tokio::fs::read_to_string(&self.system_prompt_path)
            .await
            .map_err(|e| format!("Failed to read system prompt: {}", e))?;
        hasher.update(system_prompt.as_bytes());

        // Hash leadership KB if available
        if let Some(ref kb_path) = self.leadership_kb_path {
            if kb_path.exists() {
                let kb_content = tokio::fs::read_to_string(kb_path)
                    .await
                    .map_err(|e| format!("Failed to read leadership KB: {}", e))?;
                hasher.update(kb_content.as_bytes());
            }
        }

        let hash = hasher.finalize();
        Ok(hex::encode(hash))
    }

    /// Compute manifest hash
    async fn compute_manifest_hash(&self) -> Result<String, String> {
        if let Some(ref manifest_path) = self.manifest_path {
            if manifest_path.exists() {
                let manifest_content = tokio::fs::read_to_string(manifest_path)
                    .await
                    .map_err(|e| format!("Failed to read manifest: {}", e))?;
                let hash = Sha256::digest(manifest_content.as_bytes());
                return Ok(hex::encode(hash));
            }
        }
        // Return empty hash if manifest not found
        Ok(String::new())
    }

    /// Validate nonce timestamp (30-second TTL)
    fn validate_nonce_timestamp(timestamp: i64) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let age = now - timestamp;
        age >= 0 && age <= 30 // 30-second window
    }

    /// Log successful handshake to Qdrant audit_logs
    async fn log_handshake_success(
        &self,
        remote_node_id: &str,
        remote_address: &str,
    ) {
        if let Some(ref qdrant) = self.qdrant_client {
            // We'll use the memory client to log to Qdrant
            if let Some(ref mem_client) = self.memory_client {
                let content = format!(
                    "Successful handshake with node {} from {}",
                    remote_node_id, remote_address
                );
                let mut metadata = HashMap::new();
                metadata.insert("type".to_string(), "handshake_success".to_string());
                metadata.insert("remote_node_id".to_string(), remote_node_id.to_string());
                metadata.insert("remote_address".to_string(), remote_address.to_string());
                metadata.insert("local_node_id".to_string(), self.identity.node_id.clone());

                let _ = mem_client
                    .commit_memory(tonic::Request::new(CommitMemoryRequest {
                        content,
                        namespace: "audit_logs".to_string(),
                        twin_id: "orchestrator".to_string(),
                        memory_type: "Audit".to_string(),
                        risk_level: "Low".to_string(),
                        metadata,
                    }))
                    .await;
            }
        }
    }

    /// Broadcast unauthorized node event
    fn broadcast_unauthorized_node(&self, node_id: String, reason: String, remote_address: String) {
        let event = PhoenixEvent::UnauthorizedNodeDetected {
            node_id,
            reason,
            remote_address,
            timestamp: Utc::now().to_rfc3339(),
        };
        self.message_bus.publish(event);
    }
}

#[tonic::async_trait]
impl NodeHandshakeService for NodeHandshakeServiceImpl {
    async fn initiate_handshake(
        &self,
        request: Request<HandshakeRequest>,
    ) -> Result<Response<HandshakeChallenge>, Status> {
        let req = request.into_inner();
        let remote_address = request.remote_addr()
            .map(|addr| addr.to_string())
            .unwrap_or_else(|| "unknown".to_string());

        // Check if node is quarantined
        if let Some(ref qm) = self.quarantine_manager {
            if qm.is_quarantined(&req.node_id).await {
                warn!(
                    node_id = %req.node_id,
                    remote_address = %remote_address,
                    "Rejected handshake from quarantined node"
                );
                return Err(Status::permission_denied("Node is quarantined"));
            }

            // Also check by IP address
            if let Some(ip) = remote_address.split(':').next() {
                if qm.is_ip_quarantined(ip).await {
                    warn!(
                        node_id = %req.node_id,
                        remote_address = %remote_address,
                        "Rejected handshake from quarantined IP"
                    );
                    return Err(Status::permission_denied("IP address is quarantined"));
                }
            }
        }

        // Store software_version and manifest_hash for later use
        let software_version = req.software_version.clone();
        let manifest_hash = req.manifest_hash.clone();

        info!(
            node_id = %req.node_id,
            software_version = %req.software_version,
            remote_address = %remote_address,
            "Handshake initiation received"
        );

        // Validate manifest hash if provided
        let expected_manifest_hash = self.compute_manifest_hash().await
            .map_err(|e| Status::internal(format!("Failed to compute manifest hash: {}", e)))?;
        
        if !expected_manifest_hash.is_empty() && !req.manifest_hash.is_empty() {
            if req.manifest_hash != expected_manifest_hash {
                warn!(
                    node_id = %req.node_id,
                    expected = %expected_manifest_hash,
                    received = %req.manifest_hash,
                    "Manifest hash mismatch"
                );
                self.broadcast_unauthorized_node(
                    req.node_id,
                    "manifest_mismatch".to_string(),
                    remote_address,
                );
                return Err(Status::permission_denied("Manifest hash mismatch"));
            }
        }

        // Generate challenge nonce
        let mut csprng = OsRng;
        let nonce: [u8; 32] = {
            use rand::RngCore;
            let mut n = [0u8; 32];
            csprng.fill_bytes(&mut n);
            n
        };

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Compute expected alignment token
        let alignment_token = self.compute_alignment_token().await
            .map_err(|e| Status::internal(format!("Failed to compute alignment token: {}", e)))?;

        // Store pending challenge (with software_version and manifest_hash for later)
        {
            let mut challenges = self.pending_challenges.write().await;
            challenges.insert(req.node_id.clone(), PendingChallenge {
                nonce: nonce.to_vec(),
                timestamp,
                node_id: req.node_id.clone(),
                software_version: software_version.clone(),
                manifest_hash: manifest_hash.clone(),
            });
        }

        // Store initial peer info as Pending
        {
            let mut peers = self.verified_peers.write().await;
            peers.insert(
                req.node_id.clone(),
                PeerNode {
                    node_id: req.node_id.clone(),
                    software_version,
                    manifest_hash,
                    remote_address: remote_address.clone(),
                    status: PeerStatus::Pending,
                    last_seen: Utc::now().to_rfc3339(),
                },
            );
        }

        Ok(Response::new(HandshakeChallenge {
            nonce: nonce.to_vec(),
            timestamp,
            alignment_token,
        }))
    }

    async fn complete_handshake(
        &self,
        request: Request<HandshakeResponse>,
    ) -> Result<Response<HandshakeResult>, Status> {
        let req = request.into_inner();
        let remote_address = request.remote_addr()
            .map(|addr| addr.to_string())
            .unwrap_or_else(|| "unknown".to_string());

        // Check if node is quarantined
        if let Some(ref qm) = self.quarantine_manager {
            if qm.is_quarantined(&req.node_id).await {
                warn!(
                    node_id = %req.node_id,
                    remote_address = %remote_address,
                    "Rejected handshake completion from quarantined node"
                );
                return Err(Status::permission_denied("Node is quarantined"));
            }

            // Also check by IP address
            if let Some(ip) = remote_address.split(':').next() {
                if qm.is_ip_quarantined(ip).await {
                    warn!(
                        node_id = %req.node_id,
                        remote_address = %remote_address,
                        "Rejected handshake completion from quarantined IP"
                    );
                    return Err(Status::permission_denied("IP address is quarantined"));
                }
            }
        }

        info!(
            node_id = %req.node_id,
            remote_address = %remote_address,
            "Handshake completion received"
        );

        // Retrieve pending challenge
        let challenge = {
            let mut challenges = self.pending_challenges.write().await;
            challenges.remove(&req.node_id)
        };

        let challenge = challenge.ok_or_else(|| {
            warn!(node_id = %req.node_id, "No pending challenge found");
            Status::failed_precondition("No pending challenge found")
        })?;

        let software_version = challenge.software_version.clone();
        let manifest_hash = challenge.manifest_hash.clone();

        // Validate nonce timestamp
        if !Self::validate_nonce_timestamp(challenge.timestamp) {
            warn!(
                node_id = %req.node_id,
                timestamp = challenge.timestamp,
                "Nonce expired"
            );
            self.broadcast_unauthorized_node(
                req.node_id.clone(),
                "nonce_expired".to_string(),
                remote_address,
            );
            return Err(Status::deadline_exceeded("Nonce expired"));
        }

        // Verify signature
        let verifying_key = VerifyingKey::from_bytes(&req.public_key[..32])
            .map_err(|e| {
                warn!(error = %e, "Invalid public key format");
                Status::invalid_argument("Invalid public key format")
            })?;

        let signature = Signature::from_bytes(&req.signed_nonce[..64])
            .map_err(|e| {
                warn!(error = %e, "Invalid signature format");
                Status::invalid_argument("Invalid signature format")
            })?;

        if verifying_key.verify(&challenge.nonce, &signature).is_err() {
            warn!(node_id = %req.node_id, "Signature verification failed");
            self.broadcast_unauthorized_node(
                req.node_id.clone(),
                "signature_invalid".to_string(),
                remote_address,
            );
            return Err(Status::permission_denied("Signature verification failed"));
        }

        // Verify alignment token
        let expected_token = self.compute_alignment_token().await
            .map_err(|e| Status::internal(format!("Failed to compute alignment token: {}", e)))?;

        if req.alignment_token != expected_token {
            warn!(
                node_id = %req.node_id,
                expected = %expected_token,
                received = %req.alignment_token,
                "Alignment token mismatch"
            );
            self.broadcast_unauthorized_node(
                req.node_id.clone(),
                "alignment_token_mismatch".to_string(),
                remote_address,
            );
            return Err(Status::permission_denied("Alignment token mismatch"));
        }

        // Verify guardrail version (optional but recommended)
        if req.guardrail_version != self.guardrail_version {
            warn!(
                node_id = %req.node_id,
                expected = %self.guardrail_version,
                received = %req.guardrail_version,
                "Guardrail version mismatch"
            );
            // This is a warning, not a failure - different versions can coexist
        }

        // Handshake successful!
        info!(
            node_id = %req.node_id,
            remote_address = %remote_address,
            "Handshake completed successfully"
        );

        // Log to audit trail
        self.log_handshake_success(&req.node_id, &remote_address).await;

        // Update peer status to Verified
        {
            let mut peers = self.verified_peers.write().await;
            if let Some(peer) = peers.get_mut(&req.node_id) {
                peer.status = PeerStatus::Verified;
                peer.last_seen = Utc::now().to_rfc3339();
            } else {
                // Peer not found, create new entry
                peers.insert(
                    req.node_id.clone(),
                    PeerNode {
                        node_id: req.node_id.clone(),
                        software_version: software_version.clone(),
                        manifest_hash: manifest_hash.clone(),
                        remote_address: remote_address.clone(),
                        status: PeerStatus::Verified,
                        last_seen: Utc::now().to_rfc3339(),
                    },
                );
            }
        }

        // Broadcast peer verified event
        let event = PhoenixEvent::PeerVerified {
            node_id: req.node_id.clone(),
            software_version,
            manifest_hash,
            remote_address: remote_address.clone(),
            timestamp: Utc::now().to_rfc3339(),
        };
        self.message_bus.publish(event);

        Ok(Response::new(HandshakeResult {
            success: true,
            message: "Handshake successful".to_string(),
            node_id: self.identity.node_id.clone(),
        }))
    }

    async fn propagate_quarantine(
        &self,
        request: Request<PropagateQuarantineRequest>,
    ) -> Result<Response<PropagateQuarantineResponse>, Status> {
        let req = request.into_inner();
        
        info!(
            manifest_hash = %req.manifest_hash,
            agent_id = %req.agent_id,
            quarantined_by = %req.quarantined_by,
            "Received PropagateQuarantine request from peer"
        );

        // Handle the quarantine alert via immune response system
        if let Some(ref immune_response) = self.immune_response {
            immune_response.handle_peer_quarantine(
                req.manifest_hash.clone(),
                req.agent_id.clone(),
                req.quarantined_by.clone(),
                req.compliance_score,
            ).await;
        } else {
            // Fallback: broadcast via message bus
            let event = PhoenixEvent::ComplianceAlert {
                agent_id: req.agent_id.clone(),
                manifest_hash: req.manifest_hash.clone(),
                compliance_score: req.compliance_score,
                quarantined_by: req.quarantined_by.clone(),
                timestamp: Utc::now().to_rfc3339(),
            };
            self.message_bus.publish(event);
        }

        Ok(Response::new(PropagateQuarantineResponse {
            success: true,
            message: "Quarantine alert processed".to_string(),
        }))
    }
}

/// Create and return the handshake service server
pub fn create_handshake_server(
    service: NodeHandshakeServiceImpl,
) -> NodeHandshakeServiceServer<NodeHandshakeServiceImpl> {
    NodeHandshakeServiceServer::new(service)
}
