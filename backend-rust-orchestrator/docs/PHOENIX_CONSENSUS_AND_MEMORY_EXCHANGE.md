# Phoenix Consensus Sync & Memory Exchange

This document describes the two new Phoenix AGI features: **Phoenix Consensus Sync** and **Phoenix Memory Exchange**.

## 🏛️ Phoenix Consensus Sync

The Phoenix Consensus Sync implements a mesh-wide voting mechanism where nodes vote on whether to adopt updates from the `pagi-agent-repo`. Only commits that receive majority approval (based on compliance scores) are automatically adopted.

### Architecture

**Location:** `backend-rust-orchestrator/src/network/consensus.rs`

**Flow:**
1. **Commit Detection:** When a node detects a new commit in the `pagi-agent-repo`, it broadcasts a `PhoenixEvent::ConsensusRequest { commit_hash }`.
2. **Mesh Voting:** Connected peers respond with a `PhoenixEvent::ConsensusVote` containing their local **War Room** compliance score for that specific hash.
3. **Automatic Adoption:** If the average score >= 70% AND >= 50% of nodes approve, the local node automatically performs a `git pull` and triggers a `DiscoveryRefresh`.
4. **Quarantine Sync:** If the consensus is negative, the commit is added to a 'Mesh-Wide Quarantine' list, preventing any Phoenix Orchestrator from accidentally deploying it.

### Configuration

```rust
pub struct ConsensusConfig {
    pub min_average_score: f64,              // Default: 70.0
    pub min_approval_percentage: f64,       // Default: 50.0
    pub vote_timeout_seconds: u64,           // Default: 30
    pub agents_repo_path: PathBuf,
}
```

### Usage

```rust
use crate::network::consensus::PhoenixConsensus;

// Initialize consensus system
let consensus = PhoenixConsensus::new(
    message_bus.clone(),
    node_id.clone(),
    agents_repo_path.clone(),
);

// Set dependencies
consensus.set_handshake_service(handshake_service.clone());
consensus.set_compliance_monitor(compliance_monitor.clone());

// Start listening for consensus events
consensus.start_listener().await;

// Request consensus for a new commit
consensus.request_consensus("abc123def456".to_string()).await;
```

### Events

**ConsensusRequest:**
```rust
PhoenixEvent::ConsensusRequest {
    commit_hash: String,
    requesting_node: String,
    timestamp: String,
}
```

**ConsensusVote:**
```rust
PhoenixEvent::ConsensusVote {
    commit_hash: String,
    voting_node: String,
    compliance_score: f64,
    approved: bool,
    timestamp: String,
}
```

**ConsensusResult:**
```rust
PhoenixEvent::ConsensusResult {
    commit_hash: String,
    approved: bool,
    average_score: f64,
    approval_percentage: f64,
    total_votes: usize,
    timestamp: String,
}
```

## 🧠 Phoenix Memory Exchange

The Phoenix Memory Exchange facilitates direct, peer-to-peer knowledge transfer between bare-metal nodes, using the `PhoenixEvent` bus for coordination and gRPC streaming for data transfer.

### Architecture

**Location:** `backend-rust-orchestrator/src/network/memory_exchange.rs`

**Flow:**
1. **Knowledge Request:** Node A sends a request for vectors related to a specific topic via the `PhoenixEvent` bus.
2. **Sovereign Scrubbing:** Node B retrieves relevant Qdrant data, runs the **Phoenix-Redacted** filter to scrub Ferrellgas hostnames/IPs, and streams the 'Clean' embeddings to Node A.
3. **Identity Verification:** Ensure the exchange only occurs between nodes that have passed the `PhoenixHandshake` and possess a valid Alignment Token.

### Proto Definition

**Location:** `backend-rust-orchestrator/proto/memory_exchange.proto`

```protobuf
service PhoenixMemoryExchangeService {
  rpc ExchangeMemory(ExchangeMemoryRequest) returns (stream ExchangeMemoryResponse);
  rpc VerifyAlignment(AlignmentVerificationRequest) returns (AlignmentVerificationResponse);
}
```

### Usage

```rust
use crate::network::memory_exchange::PhoenixMemoryExchangeServiceImpl;

// Initialize memory exchange service
let memory_exchange = PhoenixMemoryExchangeServiceImpl::new(
    qdrant_client.clone(),
    message_bus.clone(),
    handshake_service.clone(),
    node_id.clone(),
);

// Start listening for memory exchange requests
memory_exchange.start_listener().await;

// Get gRPC server
let server = get_memory_exchange_server(memory_exchange);
```

### Events

**MemoryExchangeRequest:**
```rust
PhoenixEvent::MemoryExchangeRequest {
    requesting_node: String,
    topic: String,
    namespace: String,
    timestamp: String,
}
```

### Security

- **Alignment Token Verification:** Only verified peers (via `PhoenixHandshake`) can exchange memory
- **Phoenix-Redacted Filter:** All content is scrubbed using `PrivacyFilter` before transmission
- **gRPC Streaming:** Efficient transfer of large vector datasets

## 🔧 Integration

### Adding to main.rs

1. **Initialize Consensus:**
```rust
let consensus = Arc::new({
    let mut c = PhoenixConsensus::new(
        message_bus.clone(),
        node_id.clone(),
        agents_repo_path.clone(),
    );
    c.set_handshake_service(handshake_service.clone());
    c.set_compliance_monitor(compliance_monitor.clone());
    c
});

// Start consensus listener
consensus.start_listener().await;
```

2. **Initialize Memory Exchange:**
```rust
let memory_exchange = Arc::new(PhoenixMemoryExchangeServiceImpl::new(
    qdrant_client.clone(),
    message_bus.clone(),
    handshake_service.clone(),
    node_id.clone(),
));

// Start memory exchange listener
memory_exchange.start_listener().await;

// Add to gRPC server
let memory_exchange_server = get_memory_exchange_server((*memory_exchange).clone());
```

3. **Integrate with Git Pull:**
The `sync_library` function in `agents/loader.rs` now detects new commits. To trigger consensus:

```rust
// In sync_library or similar function
let new_commits = detect_new_commits(&repo_path).await?;
for commit_hash in new_commits {
    consensus.request_consensus(commit_hash).await;
}
```

## 📊 Monitoring

Both systems publish events to the `PhoenixEvent` bus, which can be monitored via:
- The frontend UI (Phoenix Monitor)
- Log aggregation systems
- The mesh health API

## 🚀 Next Steps

1. **Consensus Integration:** Wire consensus requests into the git pull workflow
2. **Memory Exchange Client:** Implement client-side code to request memory from peers
3. **Embedding Generation:** Add embedding model integration for topic-based vector search
4. **Performance Optimization:** Add caching and rate limiting for memory exchanges
