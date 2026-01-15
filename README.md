# PAGI Digital Twin Platform

A polyglot microservices architecture implementing a **Tri-Layer Phoenix Architecture** for production-grade agent orchestration, secure tool execution, and persistent memory management. The system features decentralized P2P networking (Blue Flame), Phoenix Consensus Sync for mesh-wide voting, Phoenix Memory Exchange for peer-to-peer knowledge transfer, and an Auto-Domain Ingestor that automatically classifies and routes knowledge into Mind/Body/Heart/Soul domains.

---

## Table of Contents

- [Overview](#overview)
- [Quick Start](#quick-start)
- [User Guide](#user-guide)
  - [What You Can Do](#what-you-can-do)
  - [Installation](#installation)
  - [First Run](#first-run)
  - [Using the App](#using-the-app)
  - [Configuration (User)](#configuration-user)
  - [Common Tasks](#common-tasks)
  - [Troubleshooting (User)](#troubleshooting-user)
  - [FAQ (User)](#faq-user)
- [Admin Guide](#admin-guide)
  - [Admin Responsibilities](#admin-responsibilities)
  - [System Requirements](#system-requirements)
  - [Deployment Options](#deployment-options)
  - [Configuration (Admin)](#configuration-admin)
  - [Operations](#operations)
  - [Monitoring & Logging](#monitoring--logging)
  - [Backup & Recovery](#backup--recovery)
  - [Security](#security)
  - [Upgrades & Versioning](#upgrades--versioning)
  - [Troubleshooting (Admin)](#troubleshooting-admin)
  - [Runbooks](#runbooks)
- [Architecture](#architecture)
- [API Reference](#api-reference)
- [Contributing](#contributing)
- [License](#license)

---

## Overview

The PAGI Digital Twin Platform is a distributed AI agent orchestration system designed for autonomous task execution, knowledge management, and secure tool execution. The platform consists of multiple microservices written in Rust, Go, and Python, organized into four architectural layers:

1. **Gateway Layer** - Edge protocol handling (WebSocket, HTTP, SSE)
2. **Orchestrator Layer** - Planning, policy mediation, and human-in-the-loop gating
3. **Infrastructure Layer** - Memory storage (Qdrant), tool execution (sandboxed), and data stores
4. **Observability Plane** - Telemetry, tracing (Jaeger), and metrics (Prometheus)

### Key Components

- **Rust Gateway** (port 8181) - Single entry point for frontend connections
- **Rust Orchestrator** (port 8182) - Core planning and decision engine
- **Rust Memory Service** (port 50052) - Vector storage via Qdrant
- **Rust Tools Service** (port 50054) - Secure tool execution with sandboxing
- **Rust Build Service** (port 50055) - Dynamic tool compilation
- **Rust Telemetry Service** (port 8183) - Real-time system metrics
- **Frontend Application** (port 3000) - React/TypeScript UI
- **Qdrant Database** (ports 6333/6334) - Vector database for semantic memory

### Who It's For

- **End Users**: Interact with AI agents through the web interface for task automation, knowledge queries, and system management
- **Administrators**: Deploy, configure, monitor, and maintain the multi-service platform
- **Developers**: Extend functionality, create new tools, and contribute to the codebase

---

## Quick Start

### Prerequisites

- **Docker & Docker Compose** (recommended) OR
- **Bare-metal**: Rust toolchain, Node.js 18+, Python 3.10+, Go 1.21+
- **OpenRouter API Key** (for LLM planning) - Get from https://openrouter.ai/

### Fastest Path (Docker Compose)

1. **Clone the repository:**
   ```bash
   git clone <repository-url>
   cd pagi-digital-twin
   ```

2. **Create `.env` file:**
   ```bash
   cp .env.example .env  # If .env.example exists, otherwise create manually
   ```

3. **Set required environment variables:**
   ```bash
   # Minimum required
   OPENROUTER_API_KEY=sk-your-api-key-here
   ```

4. **Start all services:**
   ```bash
   docker compose up --build
   ```

5. **Verify services are running:**
   - Frontend: http://localhost:3000
   - Gateway health: http://localhost:8181/api/health
   - Orchestrator health: http://localhost:8182/health

6. **Access the application:**
   Open http://localhost:3000 in your browser to start using the platform.

### Verification Checklist

- [ ] All Docker containers are running (`docker compose ps`)
- [ ] Frontend loads at http://localhost:3000
- [ ] Health endpoints return `{"status": "ok"}` or similar
- [ ] WebSocket connection establishes (check browser console)
- [ ] No critical errors in container logs

---

## User Guide

### What You Can Do

As an end user, you can:

- **Chat with AI Agents**: Interact with specialized agents through natural language
- **Query Knowledge Base**: Search semantic memory across Mind/Body/Heart/Soul domains
- **Execute Tools**: Request tool execution (requires approval in UI)
- **View System Status**: Monitor telemetry, agent health, and network topology
- **Explore Knowledge Atlas**: Visualize 3D semantic relationships in the memory network
- **Upload Media**: Record and analyze audio/video/screen recordings
- **Manage Agents**: View, spawn, and manage sub-agent instances
- **Auto-Ingest Files**: Drop files into `data/ingest/` for automatic classification and routing

### Installation

#### Option 1: Docker Compose (Recommended)

Follow the [Quick Start](#quick-start) section above.

#### Option 2: Bare-Metal Development

**Prerequisites:**
- Rust 1.70+ (`rustup install stable`)
- Node.js 18+ (`node --version`)
- Python 3.10+ (`python --version`)
- Go 1.21+ (`go version`)
- Qdrant running locally or remotely

**Installation Steps:**

1. **Install Rust services:**
   ```bash
   cd backend-rust-memory && cargo build --release
   cd ../backend-rust-tools && cargo build --release
   cd ../backend-rust-orchestrator && cargo build --release
   cd ../backend-rust-gateway && cargo build --release
   cd ../backend-rust-telemetry && cargo build --release
   ```

2. **Install frontend dependencies:**
   ```bash
   cd frontend-digital-twin
   npm install
   ```

3. **Install Python dependencies (if using Python services):**
   ```bash
   cd backend-python-memory
   pip install -r requirements.txt
   ```

4. **Install Go dependencies (if using Go services):**
   ```bash
   cd backend-go-model-gateway
   go mod download
   ```

5. **Start Qdrant** (if not using Docker):
   ```bash
   # Download Qdrant binary or use Docker
   docker run -p 6333:6333 -p 6334:6334 qdrant/qdrant:latest
   ```

### First Run

1. **Configure environment:**
   ```bash
   # Create .env file in project root
   OPENROUTER_API_KEY=sk-your-key-here
   QDRANT_URL=http://127.0.0.1:6334
   ORCHESTRATOR_HTTP_PORT=8182
   GATEWAY_PORT=8181
   MEMORY_GRPC_PORT=50052
   TOOLS_GRPC_PORT=50054
   ```

2. **Start services in order** (bare-metal):
   ```bash
   # Terminal 1: Memory Service
   cd backend-rust-memory && cargo run
   
   # Terminal 2: Tools Service
   cd backend-rust-tools && cargo run
   
   # Terminal 3: Orchestrator
   cd backend-rust-orchestrator && cargo run
   
   # Terminal 4: Gateway
   cd backend-rust-gateway && cargo run
   
   # Terminal 5: Telemetry
   cd backend-rust-telemetry && cargo run
   
   # Terminal 6: Frontend
   cd frontend-digital-twin && npm run dev
   ```

3. **Or use the development harness:**
   ```bash
   make run-dev  # Starts core services via Python script
   ```

4. **Access the UI:**
   - Open http://localhost:3000
   - Create a new session or connect to existing twin

### Using the App

#### Chat Interface

1. **Start a conversation:**
   - Enter your message in the chat input
   - The orchestrator will plan actions and request approvals if needed

2. **Approve/Deny Actions:**
   - When an action requires approval, a notification appears
   - Click "Approve" or "Deny" to proceed or cancel

3. **View Responses:**
   - Agent responses appear in the chat stream
   - Actions taken are listed in the response metadata

#### Intelligence Hub

Access via the "Intelligence Hub" view to:

- **Agent Vitality**: View agent stations and their health status
- **Neural Map**: Explore 3D knowledge graph visualization
- **Attribution Analytics**: See domain balance (Mind/Body/Heart/Soul)
- **Ingestion Progress Dashboard**: Real-time monitoring of knowledge ingestion with animated progress bars
- **Intelligence Stream**: Real-time activity feed

#### Knowledge Base Operations

- **Query Memory**: Use natural language to search across domains
- **View Knowledge Atlas**: Navigate semantic relationships
- **Trace Paths**: Find connections between knowledge nodes
- **Auto-Ingest**: Drop files into `data/ingest/` directory for automatic processing

#### Ingestion Progress Dashboard

The **Ingestion Progress Dashboard** provides real-time visualization of knowledge ingestion in the right sidebar:

**Features:**
- **Real-Time Progress Bars**: Animated progress indicators for each file being processed (using Framer Motion)
- **Domain Classification**: Files are automatically classified into Mind/Body/Heart/Soul domains with confidence percentages
- **Color-Coded Domains**:
  - 🔵 **Blue (Mind)**: Technical specifications, APIs, code patterns
  - 🟢 **Green (Body)**: System telemetry, logs, performance data
  - 🟠 **Orange (Heart)**: User preferences, personas, personalization
  - 🔴 **Red (Soul)**: Security audits, governance, ethical guidelines
- **Toast Notifications**: Real-time notifications when files complete: *"Security Audit classified as SOUL (98% confidence)"*
- **Summary Statistics**: View processed, failed, and active file counts
- **Error Display**: Failed ingestions show detailed error messages

**How to Use:**
1. Drop files (text, logs, JSON) into the `data/ingest/` directory
2. Watch the dashboard update in real-time as files are processed
3. Receive toast notifications when classification completes
4. Monitor the radar chart in Attribution Analytics as knowledge bases grow

**Location**: The dashboard appears in the right sidebar below the Domain Confidence Gauges section.

#### Media Recording

- **Start Recording**: Enable microphone/camera/screen share
- **Upload Media**: Recordings are automatically stored
- **View Gallery**: Access past recordings in Media Gallery
- **Transcript Analysis**: Recordings can be transcribed and analyzed

### Configuration (User)

User-configurable settings (via UI or environment):

| Setting | Description | Default | Location |
|---------|-------------|---------|----------|
| `VITE_WS_URL` | WebSocket URL for chat | `ws://localhost:8181/ws/chat` | Frontend `.env` |
| `VITE_SSE_URL` | SSE URL for telemetry | `http://localhost:8181/v1/telemetry/stream` | Frontend `.env` |
| `VITE_ORCHESTRATOR_URL` | Orchestrator API URL | `http://127.0.0.1:8182` | Frontend `.env` |

**Note**: Most configuration is handled by administrators. Users primarily interact through the UI.

### Common Tasks

#### Start/Stop Services

**Docker:**
```bash
docker compose up      # Start
docker compose down    # Stop
docker compose restart # Restart
```

**Bare-metal:**
```bash
make run-dev          # Start (via harness)
make stop-dev         # Stop
# Or manually: Ctrl+C in each terminal
```

#### Reset Session State

- Clear browser local storage
- Or use "Clear" button in Intelligence Stream (if available)

#### Export/Import Data

- **Knowledge Base**: TBD (verify in `backend-rust-memory/src/main.rs`)
- **Agent Configurations**: Stored in `config/agents/` directory
- **Playbooks**: Stored in `test-agent-repo/playbooks/`

### Troubleshooting (User)

#### Frontend Won't Load

**Symptom**: Browser shows connection error or blank page

**Likely Causes:**
1. Gateway service not running
2. Wrong port configuration
3. CORS issues

**Fix:**
1. Check Gateway health: `curl http://localhost:8181/api/health`
2. Verify `VITE_WS_URL` matches Gateway port
3. Check browser console for errors
4. Ensure services started in correct order

#### Chat Not Responding

**Symptom**: Messages sent but no response

**Likely Causes:**
1. Orchestrator not running
2. OpenRouter API key invalid/missing
3. WebSocket connection dropped

**Fix:**
1. Check Orchestrator health: `curl http://localhost:8182/health`
2. Verify `OPENROUTER_API_KEY` is set correctly
3. Check browser Network tab for WebSocket status
4. Review orchestrator logs for errors

#### Actions Stuck in "Pending"

**Symptom**: Actions require approval but UI doesn't show approval prompt

**Likely Causes:**
1. UI notification system not working
2. Action expired
3. Session state corrupted

**Fix:**
1. Refresh the page
2. Check browser console for JavaScript errors
3. Clear browser cache and reload
4. Check orchestrator logs for action status

#### Knowledge Queries Return Empty

**Symptom**: Memory searches return no results

**Likely Causes:**
1. Qdrant not running or unreachable
2. Memory service not connected to Qdrant
3. No data ingested yet

**Fix:**
1. Verify Qdrant: `curl http://localhost:6333/collections`
2. Check `QDRANT_URL` environment variable
3. Verify memory service logs for connection errors
4. Ingest some knowledge base files (see Admin Guide)

### FAQ (User)

**Q: Do I need an API key to use the system?**  
A: Yes, an OpenRouter API key is required for LLM planning. Get one from https://openrouter.ai/

**Q: Can I use the system offline?**  
A: Limited functionality is available with `LLM_PROVIDER=mock`, but full features require internet connectivity for OpenRouter API calls.

**Q: How do I add my own knowledge?**  
A: Drop files (PDFs, text, logs, JSON) into the `data/ingest/` directory. The Auto-Domain Ingestor will automatically classify and route them.

**Q: What file formats are supported for ingestion?**  
A: Currently supports text-based files (`.txt`, `.md`, `.log`, `.json`, etc.). PDF parsing TBD. See `backend-rust-orchestrator/src/knowledge/ingestor.rs` for details.

**Q: How do I monitor ingestion progress?**  
A: The Ingestion Progress Dashboard in the right sidebar shows real-time progress bars, domain classifications, and completion notifications. It polls the ingestion status every 2 seconds while files are processing.

**Q: How do I create custom agents?**  
A: See Admin Guide section on Agent Templates. Agent definitions are in `test-agent-repo/agent-templates/`.

**Q: Can I run this on Windows?**  
A: Yes, Docker Compose works on Windows. Bare-metal requires WSL2 or native Rust/Node/Python installations.

---

## Admin Guide

### Admin Responsibilities

Administrators are responsible for:

- **Deployment**: Setting up and maintaining the multi-service architecture
- **Configuration**: Managing environment variables, secrets, and service URLs
- **Security**: Implementing access controls, secret rotation, and network hardening
- **Monitoring**: Tracking service health, logs, and performance metrics
- **Backup**: Ensuring data persistence for Qdrant and other stateful services
- **Upgrades**: Safely updating services and migrating configurations
- **Troubleshooting**: Diagnosing and resolving operational issues

### System Requirements

#### Minimum Requirements (Development)

- **CPU**: 2 cores
- **RAM**: 4 GB
- **Disk**: 10 GB free
- **OS**: Linux, macOS, or Windows (with WSL2/Docker)

#### Recommended Requirements (Production)

- **CPU**: 4+ cores
- **RAM**: 8+ GB
- **Disk**: 50+ GB (for Qdrant vector storage and logs)
- **OS**: Linux (Ubuntu 22.04+ recommended) or containerized deployment
- **Network**: Stable internet connection for OpenRouter API

#### Software Dependencies

| Component | Version | Purpose |
|-----------|---------|---------|
| Docker | 20.10+ | Container orchestration (recommended) |
| Docker Compose | 2.0+ | Multi-container management |
| Rust | 1.70+ | Core services (if bare-metal) |
| Node.js | 18+ | Frontend build/runtime |
| Python | 3.10+ | Python services (if used) |
| Go | 1.21+ | Go services (if used) |
| Qdrant | Latest | Vector database (can run in Docker) |

### Deployment Options

#### Option 1: Docker Compose (Recommended for Production)

**Advantages:**
- Isolated service environments
- Easy scaling and updates
- Consistent across environments
- Built-in service discovery

**Setup:**
```bash
# 1. Configure environment
cp .env.example .env
# Edit .env with your settings

# 2. Start all services
docker compose up -d

# 3. View logs
docker compose logs -f

# 4. Stop services
docker compose down
```

**Service Dependencies:**
- Qdrant must start before Memory Service
- Memory/Tools Services must start before Orchestrator
- Orchestrator must start before Gateway
- Gateway must start before Frontend

**Networking:**
- Services communicate via Docker network `pagi-network`
- External ports exposed: 3000, 8181, 8182, 8183, 6333, 6334
- Internal service names: `rust-orchestrator`, `rust-memory-service`, etc.

#### Option 2: Bare-Metal Deployment

**Advantages:**
- Faster development iteration
- Direct access to logs and processes
- Lower resource overhead
- Easier debugging

**Setup:**
```bash
# 1. Install prerequisites (see System Requirements)

# 2. Build Rust services
cd backend-rust-memory && cargo build --release
cd ../backend-rust-tools && cargo build --release
cd ../backend-rust-orchestrator && cargo build --release
cd ../backend-rust-gateway && cargo build --release
cd ../backend-rust-telemetry && cargo build --release

# 3. Start Qdrant (if not using Docker)
# Option A: Docker
docker run -d -p 6333:6333 -p 6334:6334 qdrant/qdrant:latest

# Option B: Native binary
# Download from https://github.com/qdrant/qdrant/releases
./qdrant

# 4. Configure environment
export OPENROUTER_API_KEY=sk-your-key
export QDRANT_URL=http://127.0.0.1:6334
export MEMORY_GRPC_PORT=50052
export TOOLS_GRPC_PORT=50054
export ORCHESTRATOR_HTTP_PORT=8182
export GATEWAY_PORT=8181

# 5. Start services (use development harness)
make run-dev

# Or start manually in separate terminals (see First Run section)
```

**Service Management:**
- Use `systemd` units for production (TBD - verify in `scripts/` directory)
- Or use process managers like `supervisord` or `pm2`
- Logs go to stdout/stderr (redirect as needed)

#### Option 3: Hybrid Deployment

Run some services in Docker (Qdrant, observability) and others bare-metal (core services) for development flexibility.

### Configuration (Admin)

#### Environment Variables

Create a `.env` file in the project root. All services read from this file.

**Required Variables:**

| Variable | Description | Example |
|----------|-------------|---------|
| `OPENROUTER_API_KEY` | OpenRouter API key for LLM | `sk-or-...` |
| `QDRANT_URL` | Qdrant gRPC endpoint | `http://127.0.0.1:6334` |

**Service Ports:**

| Variable | Default | Description |
|----------|---------|-------------|
| `GATEWAY_PORT` | `8181` | Gateway HTTP/WebSocket port |
| `ORCHESTRATOR_HTTP_PORT` | `8182` | Orchestrator HTTP API port |
| `MEMORY_GRPC_PORT` | `50052` | Memory service gRPC port |
| `TOOLS_GRPC_PORT` | `50054` | Tools service gRPC port |
| `BUILD_SERVICE_PORT` | `50055` | Build service gRPC port |
| `TELEMETRY_PORT` | `8183` | Telemetry SSE port |
| `ORCHESTRATOR_GRPC_PORT` | `50057` | Orchestrator public gRPC port |
| `ORCHESTRATOR_ADMIN_GRPC_PORT` | `50056` | Orchestrator admin gRPC port |

**LLM Configuration:**

| Variable | Default | Description |
|----------|---------|-------------|
| `LLM_PROVIDER` | `openrouter` | LLM provider: `openrouter` or `mock` |
| `OPENROUTER_URL` | `https://openrouter.ai/api/v1/chat/completions` | OpenRouter API endpoint |
| `OPENROUTER_MODEL` | `google/gemini-2.0-flash-exp` | Model name for planning |

**Memory/Embedding Configuration:**

| Variable | Default | Description |
|----------|---------|-------------|
| `EMBEDDING_MODEL_DIM` | `384` | Vector dimension (all-MiniLM-L6-v2) |
| `EMBEDDING_MODEL_NAME` | `all-MiniLM-L6-v2` | Embedding model identifier |
| `QDRANT_API_KEY` | (optional) | Qdrant API key if required |

**Security Gates (Research Only - Use with Caution):**

See [`ENV_SETUP.md`](ENV_SETUP.md) for detailed security gate documentation. These bypass normal security restrictions:

- `ALLOW_PUBLIC_NETWORK_SCAN` - Enable public IP scanning
- `ALLOW_ARBITRARY_PORT_SCAN` - Allow custom port ranges
- `BYPASS_HITL_TOOL_EXEC` - Skip human approval for tools
- `BYPASS_HITL_MEMORY` - Skip human approval for memory ops
- `ALLOW_RESTRICTED_COMMANDS` - Allow destructive commands

**⚠️ WARNING**: Security gates are for research environments only. Do not enable in production.

**Ingestion Configuration:**

| Variable | Default | Description |
|----------|---------|-------------|
| `INGEST_DIR` | `data/ingest` | Directory watched for auto-ingestion |
| `INGEST_POLL_INTERVAL_MS` | `2000` | Frontend polling interval for ingestion status (milliseconds) |

**Logging:**

| Variable | Default | Description |
|----------|---------|-------------|
| `LOG_LEVEL` | `info` | Log verbosity: `trace`, `debug`, `info`, `warn`, `error` |

**Service URLs (for bare-metal):**

| Variable | Default | Description |
|----------|---------|-------------|
| `ORCHESTRATOR_URL` | `http://127.0.0.1:8182` | Orchestrator HTTP URL |
| `MEMORY_GRPC_ADDR` | `http://127.0.0.1:50052` | Memory service gRPC address |
| `TOOLS_GRPC_ADDR` | `http://127.0.0.1:50054` | Tools service gRPC address |

**Frontend Configuration:**

| Variable | Default | Description |
|----------|---------|-------------|
| `VITE_WS_URL` | `ws://localhost:8181/ws/chat` | WebSocket URL |
| `VITE_SSE_URL` | `http://localhost:8181/v1/telemetry/stream` | SSE telemetry URL |
| `VITE_ORCHESTRATOR_URL` | `http://127.0.0.1:8182` | Orchestrator API URL |
| `VITE_GATEWAY_URL` | `http://127.0.0.1:8181` | Gateway URL |

#### Secret Management

**Best Practices:**

1. **Never commit `.env` to version control** (already in `.gitignore`)
2. **Use `.env.example` as a template** (without actual secrets)
3. **Rotate API keys regularly** in production
4. **Use different keys** for development and production
5. **Store secrets securely** in production (use secret management systems)

**For Production:**

- Use environment variable injection from secret managers (HashiCorp Vault, AWS Secrets Manager, etc.)
- Or mount secrets as files in Docker: `docker run -v /secrets:/app/secrets:ro ...`
- TBD: Verify secret rotation procedures in `docs/SECURITY_GATES.md`

#### Port Configuration

**Default Port Matrix:**

| Service | Port | Protocol | External Access |
|---------|------|----------|-----------------|
| Frontend | 3000 | HTTP | Yes |
| Gateway | 8181 | HTTP/WS/SSE | Yes |
| Orchestrator | 8182 | HTTP | Yes |
| Telemetry | 8183 | HTTP (SSE) | Via Gateway proxy |
| Memory | 50052 | gRPC | Internal only |
| Tools | 50054 | gRPC | Internal only |
| Build | 50055 | gRPC | Internal only |
| Orchestrator gRPC | 50057 | gRPC | Yes |
| Qdrant REST | 6333 | HTTP | Yes (admin) |
| Qdrant gRPC | 6334 | gRPC | Internal only |

**Firewall Considerations:**

- Expose ports 3000, 8181, 8182 to users
- Restrict 6333 (Qdrant admin) to internal network
- Block direct access to gRPC ports (50052, 50054, 50055, 6334) from external networks
- Use reverse proxy (nginx/traefik) for production with TLS

### Operations

#### Starting Services

**Docker Compose:**
```bash
# Start all services
docker compose up -d

# Start specific service
docker compose up -d rust-orchestrator

# View logs
docker compose logs -f rust-orchestrator
```

**Bare-metal:**
```bash
# Using development harness
make run-dev

# Or manually (see First Run section)
```

#### Stopping Services

**Docker Compose:**
```bash
# Graceful shutdown
docker compose down

# Force stop
docker compose kill
docker compose rm -f
```

**Bare-metal:**
```bash
# Using harness
make stop-dev

# Or send SIGTERM to each process
# Services handle graceful shutdown
```

#### Restarting Services

**Docker Compose:**
```bash
# Restart all
docker compose restart

# Restart specific service
docker compose restart rust-orchestrator
```

**Bare-metal:**
```bash
# Stop and start again
make stop-dev && make run-dev
```

#### Health Checks

**HTTP Health Endpoints:**

```bash
# Gateway
curl http://localhost:8181/api/health

# Orchestrator
curl http://localhost:8182/health

# Qdrant
curl http://localhost:6333/health
```

**gRPC Health Checks:**

```bash
# Memory Service (requires grpcurl)
grpcurl -plaintext localhost:50052 grpc.health.v1.Health/Check

# Tools Service
grpcurl -plaintext localhost:50054 grpc.health.v1.Health/Check
```

**Expected Responses:**
- HTTP: `{"status": "ok"}` or `{"service": "...", "status": "healthy"}`
- gRPC: `{"status": "SERVING"}`

#### Graceful Shutdown Behavior

- Services listen for SIGTERM/SIGINT
- Active requests complete before shutdown
- WebSocket connections close gracefully
- Qdrant connections close cleanly
- TBD: Verify shutdown timeout behavior (check `backend-rust-*/src/main.rs`)

### Monitoring & Logging

#### Log Locations

**Docker:**
```bash
# View all logs
docker compose logs

# View specific service
docker compose logs rust-orchestrator

# Follow logs
docker compose logs -f rust-orchestrator

# Last 100 lines
docker compose logs --tail=100 rust-orchestrator
```

**Bare-metal:**
- Logs go to stdout/stderr
- Redirect to files: `cargo run > orchestrator.log 2>&1`
- Or use systemd journal: `journalctl -u pagi-orchestrator`

#### Log Formats

- **Rust services**: Structured JSON logging via `tracing` crate
- **Go services**: Structured JSON logging
- **Python services**: TBD (verify in `backend-python-*/main.py`)

**Log Levels:**
- `trace` - Very verbose (development only)
- `debug` - Debug information
- `info` - Normal operation (default)
- `warn` - Warnings
- `error` - Errors only

**Set log level:**
```bash
export LOG_LEVEL=debug
# Or in .env: LOG_LEVEL=debug
```

#### Observability Stack

**Jaeger (Tracing):**
- URL: http://localhost:16686
- Collects distributed traces from services
- TBD: Verify which services emit traces (check `backend-rust-orchestrator/src/main.rs`)

**Prometheus (Metrics):**
- URL: http://localhost:9090
- Collects metrics from services
- TBD: Verify metrics endpoints (check `observability/prometheus.yml`)

**Telemetry Service:**
- SSE stream: http://localhost:8181/v1/telemetry/stream
- Provides real-time CPU, memory, process counts
- Updates every 2 seconds (configurable via `TELEMETRY_INTERVAL_MS`)

#### Log Rotation

**Docker:**
- Configure in `docker-compose.yml`:
  ```yaml
  logging:
    driver: "json-file"
    options:
      max-size: "10m"
      max-file: "3"
  ```

**Bare-metal:**
- Use `logrotate` or systemd journal rotation
- TBD: Verify recommended rotation settings

#### Monitoring Checklist

- [ ] Services respond to health checks
- [ ] No error logs in last hour
- [ ] Qdrant collections accessible
- [ ] Memory service can connect to Qdrant
- [ ] WebSocket connections stable
- [ ] CPU/memory usage within limits
- [ ] Disk space adequate for Qdrant storage
- [ ] Ingestion dashboard shows real-time updates (if files are being processed)
- [ ] Toast notifications appear when files complete ingestion

### Backup & Recovery

#### What Needs Backup

1. **Qdrant Data**: Vector database storage (persistent volumes)
2. **Agent Configurations**: `config/agents/` directory
3. **Playbooks**: `test-agent-repo/playbooks/` directory
4. **Knowledge Bases**: `knowledge_bases/` directory
5. **Environment Configuration**: `.env` file (securely)
6. **Audit Logs**: TBD (verify location in `backend-go-agent-planner`)

#### Backup Procedures

**Qdrant Backup (Docker):**
```bash
# Qdrant data is in Docker volume
docker run --rm -v qdrant_data:/data -v $(pwd):/backup \
  alpine tar czf /backup/qdrant-backup-$(date +%Y%m%d).tar.gz /data

# Or use Qdrant snapshot API
curl -X POST http://localhost:6333/collections/{collection_name}/snapshots
```

**Qdrant Backup (Bare-metal):**
```bash
# Default storage location: ~/.qdrant/storage
tar czf qdrant-backup-$(date +%Y%m%d).tar.gz ~/.qdrant/storage
```

**Configuration Backup:**
```bash
# Backup config files
tar czf config-backup-$(date +%Y%m%d).tar.gz \
  config/ test-agent-repo/ knowledge_bases/ .env
```

**Automated Backups:**
- Set up cron job or systemd timer
- Store backups in secure location
- Test restore procedures regularly

#### Recovery Procedures

**Restore Qdrant:**
```bash
# Stop services
docker compose down

# Restore volume
docker run --rm -v qdrant_data:/data -v $(pwd):/backup \
  alpine tar xzf /backup/qdrant-backup-YYYYMMDD.tar.gz -C /

# Start services
docker compose up -d
```

**Restore Configuration:**
```bash
# Extract backup
tar xzf config-backup-YYYYMMDD.tar.gz

# Verify .env is correct
# Restart services
```

**Verification:**
- Health checks pass
- Knowledge queries return expected results
- Collections visible in Qdrant: `curl http://localhost:6333/collections`

### Security

#### Threat Model

**Assumptions:**
- Services run on trusted network (or behind firewall)
- Qdrant not exposed to public internet
- API keys stored securely
- Human-in-the-loop (HITL) provides safety checks

**Threats Addressed:**
- Unauthorized tool execution (HITL gating)
- Memory access control (HITL gating)
- Sandboxed tool execution (bubblewrap/isolated environments)
- Network scanning restrictions (RFC1918 only by default)

#### Hardening Checklist

- [ ] **Firewall Configuration**
  - Block external access to gRPC ports (50052, 50054, 50055, 6334)
  - Restrict Qdrant admin port (6333) to internal network
  - Use reverse proxy with TLS for production

- [ ] **Secret Management**
  - Rotate `OPENROUTER_API_KEY` regularly
  - Use different keys for dev/prod
  - Never commit secrets to version control

- [ ] **Service Isolation**
  - Run services with least privilege
  - Use non-root users in containers
  - Isolate tool execution in sandboxes

- [ ] **Network Security**
  - Disable public network scanning (`ALLOW_PUBLIC_NETWORK_SCAN=false`)
  - Restrict port scanning to required ranges
  - Use VPN or private networks for multi-node deployments

- [ ] **Access Control**
  - Keep HITL gates enabled (do not use `BYPASS_*` flags in production)
  - Monitor approval logs
  - Review tool execution history regularly

- [ ] **Update Cadence**
  - Update dependencies regularly
  - Monitor security advisories for Rust/Go/Python/Node
  - Update Qdrant to latest stable version

#### Authentication & Authorization

**Current State:**
- No built-in user authentication (TBD - verify in codebase)
- HITL provides action-level authorization
- API key authentication TBD for production (check `backend-go-agent-planner` for `PAGI_API_KEY`)

**Production Recommendations:**
- Implement API key authentication for all endpoints
- Add user roles and permissions
- Use OAuth2/OIDC for user authentication
- Implement rate limiting

#### Security Gates Documentation

See [`docs/SECURITY_GATES.md`](docs/SECURITY_GATES.md) and [`ENV_SETUP.md`](ENV_SETUP.md) for detailed security gate documentation.

**⚠️ IMPORTANT**: Security gates (`ALLOW_*`, `BYPASS_*`) are for research environments only. Never enable in production.

### Upgrades & Versioning

#### Upgrade Procedure

1. **Backup current state** (see Backup & Recovery)
2. **Review changelog** (TBD - verify if changelog exists)
3. **Update code:**
   ```bash
   git pull origin main
   # Or checkout specific version tag
   git checkout v1.0.0
   ```

4. **Rebuild services:**
   ```bash
   # Docker
   docker compose build
   docker compose up -d
   
   # Bare-metal
   cargo build --release  # For Rust services
   npm install && npm run build  # For frontend
   ```

5. **Verify health checks**
6. **Test critical functionality**
7. **Monitor logs for errors**

#### Configuration Migrations

- Check for new environment variables in updated `.env.example`
- Review service-specific migration notes (TBD - verify in `docs/`)
- Test configuration changes in development first

#### Rollback Procedure

1. **Stop services**
2. **Restore previous version:**
   ```bash
   git checkout <previous-version-tag>
   ```

3. **Restore backups if needed** (see Backup & Recovery)
4. **Rebuild and restart**
5. **Verify functionality**

#### Versioning Scheme

- TBD: Verify versioning scheme (check `Cargo.toml` and `package.json` files)
- Services may have independent versions
- Use git tags for release versions

### Troubleshooting (Admin)

#### Service Won't Start

**Symptom**: Container exits immediately or process crashes

**Diagnosis:**
```bash
# Docker
docker compose logs <service-name>

# Bare-metal
# Check stdout/stderr or systemd journal
journalctl -u <service-name> -n 50
```

**Common Causes:**
1. **Missing environment variables**
   - Fix: Verify `.env` file has all required variables
   - Check: Service logs for "missing" or "required" errors

2. **Port already in use**
   - Fix: `lsof -i :8182` (or `netstat -tulpn`) to find process
   - Kill conflicting process or change port in `.env`

3. **Qdrant not reachable**
   - Fix: Verify `QDRANT_URL` is correct
   - Check: `curl http://localhost:6333/health`
   - Ensure Qdrant container/service is running

4. **Invalid API key**
   - Fix: Verify `OPENROUTER_API_KEY` is valid
   - Test: `curl https://openrouter.ai/api/v1/models -H "Authorization: Bearer $OPENROUTER_API_KEY"`

5. **Permission errors**
   - Fix: Check file permissions for data directories
   - Ensure user has write access to `data/`, `tools_repo/`, etc.

#### Port Conflicts

**Symptom**: "Address already in use" error

**Fix:**
```bash
# Find process using port
lsof -i :8182  # Linux/macOS
netstat -ano | findstr :8182  # Windows

# Kill process (replace PID)
kill -9 <PID>  # Linux/macOS
taskkill /PID <PID> /F  # Windows

# Or change port in .env
ORCHESTRATOR_HTTP_PORT=8183
```

#### Configuration Errors

**Symptom**: Services start but fail to connect or behave incorrectly

**Diagnosis:**
- Check service logs for configuration-related errors
- Verify environment variables are loaded: `docker compose config`
- Test service URLs: `curl http://localhost:8182/health`

**Common Issues:**
1. **Wrong service URLs** (bare-metal)
   - Fix: Ensure `ORCHESTRATOR_URL`, `MEMORY_GRPC_ADDR` point to correct addresses
   - Use `http://127.0.0.1:PORT` for local, `http://service-name:PORT` for Docker

2. **Missing required variables**
   - Fix: Compare `.env` with `.env.example` (if exists)
   - Check service-specific requirements in code

3. **Type mismatches**
   - Fix: Ensure numeric values are numbers, not strings
   - Check boolean values: use `true`/`false` or `1`/`0`

#### High CPU/Memory Usage

**Symptom**: System becomes slow, services unresponsive

**Diagnosis:**
```bash
# Docker
docker stats

# Bare-metal
top
htop
```

**Common Causes:**
1. **Qdrant indexing large collections**
   - Fix: Normal during initial ingestion, wait for completion
   - Monitor: Qdrant logs for indexing progress

2. **Memory leak in service**
   - Fix: Restart affected service
   - Report: If persistent, check service logs and GitHub issues

3. **Too many concurrent requests**
   - Fix: Implement rate limiting (TBD - verify if exists)
   - Scale: Add more instances or increase resources

4. **Embedding model loading**
   - Fix: First request loads model (one-time cost)
   - Monitor: Subsequent requests should be faster

#### Qdrant Connection Issues

**Symptom**: Memory service can't connect to Qdrant

**Diagnosis:**
```bash
# Test Qdrant
curl http://localhost:6333/health
curl http://localhost:6333/collections

# Check Memory Service logs
docker compose logs rust-memory-service
```

**Common Causes:**
1. **Qdrant not running**
   - Fix: Start Qdrant: `docker compose up -d qdrant-db`

2. **Wrong URL**
   - Fix: Verify `QDRANT_URL=http://qdrant-db:6334` (Docker) or `http://127.0.0.1:6334` (bare-metal)

3. **Network issues**
   - Fix: Ensure services on same Docker network
   - Check: `docker network inspect pagi-network`

4. **API key required**
   - Fix: Set `QDRANT_API_KEY` if Qdrant requires authentication

#### Knowledge Base Not Updating

**Symptom**: Ingested files don't appear in queries

**Diagnosis:**
```bash
# Check ingestion status
curl http://localhost:8182/api/knowledge/ingest/status

# Check Qdrant collections
curl http://localhost:6333/collections

# Check orchestrator logs for ingestion errors
docker compose logs rust-orchestrator | grep -i ingest
```

**Common Causes:**
1. **Ingestor not running**
   - Fix: Verify ingestor is initialized (check orchestrator startup logs)
   - Ensure `data/ingest/` directory exists and is writable

2. **Classification failures**
   - Fix: Check LLM settings if using semantic classification
   - Verify fallback keyword classification works

3. **Qdrant write failures**
   - Fix: Check Qdrant logs and disk space
   - Verify collections exist: `curl http://localhost:6333/collections`

4. **Dashboard not showing progress**
   - Fix: Check browser console for JavaScript errors
   - Verify frontend can reach orchestrator: `curl http://localhost:8182/api/knowledge/ingest/status`
   - Check that `VITE_ORCHESTRATOR_URL` is correctly configured in frontend `.env`

### Runbooks

#### Runbook: Fresh Install

**Objective**: Deploy PAGI Digital Twin on a clean system

**Steps:**

1. **Prerequisites:**
   ```bash
   # Install Docker and Docker Compose
   # Verify: docker --version && docker compose version
   ```

2. **Clone repository:**
   ```bash
   git clone <repository-url>
   cd pagi-digital-twin
   ```

3. **Configure environment:**
   ```bash
   # Create .env file
   cat > .env << EOF
   OPENROUTER_API_KEY=sk-your-key-here
   QDRANT_URL=http://qdrant-db:6334
   LOG_LEVEL=info
   EOF
   ```

4. **Start services:**
   ```bash
   docker compose up -d
   ```

5. **Verify:**
   ```bash
   # Wait for services to start (30-60 seconds)
   sleep 60
   
   # Check health
   curl http://localhost:8181/api/health
   curl http://localhost:8182/health
   curl http://localhost:6333/health
   
   # Check all containers running
   docker compose ps
   ```

6. **Access UI:**
   - Open http://localhost:3000
   - Verify WebSocket connection establishes

**Success Criteria:**
- All health checks return `200 OK`
- Frontend loads without errors
- Can send chat message and receive response

#### Runbook: Upgrade

**Objective**: Safely upgrade to new version

**Steps:**

1. **Backup:**
   ```bash
   # Backup Qdrant
   docker run --rm -v qdrant_data:/data -v $(pwd):/backup \
     alpine tar czf /backup/qdrant-backup-$(date +%Y%m%d).tar.gz /data
   
   # Backup configuration
   tar czf config-backup-$(date +%Y%m%d).tar.gz \
     config/ test-agent-repo/ knowledge_bases/ .env
   ```

2. **Stop services:**
   ```bash
   docker compose down
   ```

3. **Update code:**
   ```bash
   git pull origin main
   # Or: git checkout <new-version-tag>
   ```

4. **Rebuild:**
   ```bash
   docker compose build --no-cache
   ```

5. **Start services:**
   ```bash
   docker compose up -d
   ```

6. **Verify:**
   ```bash
   # Wait for startup
   sleep 60
   
   # Health checks
   curl http://localhost:8181/api/health
   curl http://localhost:8182/health
   
   # Test functionality
   # Send test message, verify response
   ```

7. **Rollback if needed:**
   ```bash
   # If issues occur
   docker compose down
   git checkout <previous-version>
   docker compose up -d
   # Restore backups if data corrupted
   ```

**Success Criteria:**
- All services start successfully
- Health checks pass
- Critical functionality works
- No data loss

#### Runbook: Rotate Secrets

**Objective**: Update API keys and secrets securely

**Steps:**

1. **Prepare new secrets:**
   - Generate new OpenRouter API key
   - Update `.env` with new key (keep old for rollback)

2. **Update configuration:**
   ```bash
   # Edit .env
   OPENROUTER_API_KEY=sk-new-key-here
   ```

3. **Restart services:**
   ```bash
   # Restart orchestrator (uses API key)
   docker compose restart rust-orchestrator
   
   # Verify no errors in logs
   docker compose logs -f rust-orchestrator
   ```

4. **Test:**
   ```bash
   # Send test message
   curl -X POST http://localhost:8182/v1/chat \
     -H "Content-Type: application/json" \
     -d '{"message": "test", "twin_id": "test", "session_id": "test"}'
   ```

5. **Verify old key no longer works:**
   - TBD: Verify if old sessions are invalidated

6. **Update other services if needed:**
   - TBD: Check if other services use API keys

**Success Criteria:**
- Services restart without errors
- New API key works
- Old API key rejected (if applicable)

#### Runbook: Restore From Backup

**Objective**: Recover from data loss or corruption

**Steps:**

1. **Stop services:**
   ```bash
   docker compose down
   ```

2. **Identify backup:**
   ```bash
   ls -lh *backup*.tar.gz
   # Choose appropriate backup date
   ```

3. **Restore Qdrant:**
   ```bash
   # Remove existing volume (WARNING: destroys current data)
   docker volume rm qdrant_data
   
   # Create new volume
   docker volume create qdrant_data
   
   # Restore data
   docker run --rm -v qdrant_data:/data -v $(pwd):/backup \
     alpine tar xzf /backup/qdrant-backup-YYYYMMDD.tar.gz -C /
   ```

4. **Restore configuration (if needed):**
   ```bash
   tar xzf config-backup-YYYYMMDD.tar.gz
   ```

5. **Start services:**
   ```bash
   docker compose up -d
   ```

6. **Verify:**
   ```bash
   # Wait for startup
   sleep 60
   
   # Check collections
   curl http://localhost:6333/collections
   
   # Test queries
   # Send knowledge query, verify results match backup expectations
   ```

**Success Criteria:**
- Qdrant collections restored
- Knowledge queries return expected data
- Services operate normally

#### Runbook: Diagnose High CPU/Memory

**Objective**: Identify and resolve resource exhaustion

**Steps:**

1. **Identify resource usage:**
   ```bash
   # Docker
   docker stats --no-stream
   
   # Bare-metal
   top
   # Or: htop, btop
   ```

2. **Identify problematic service:**
   - Note which service uses most CPU/memory
   - Check service logs: `docker compose logs <service>`

3. **Common causes and fixes:**

   **Qdrant indexing:**
   - Symptom: High CPU during/after large ingestion
   - Fix: Wait for indexing to complete (normal behavior)
   - Monitor: Qdrant logs for "indexing" messages

   **Memory leak:**
   - Symptom: Memory usage grows over time
   - Fix: Restart service: `docker compose restart <service>`
   - If persistent: Check service code for leaks, file issue

   **Too many requests:**
   - Symptom: High CPU with many concurrent users
   - Fix: Implement rate limiting (TBD - verify if exists)
   - Scale: Add more service instances

   **Embedding model:**
   - Symptom: High memory on first request
   - Fix: Normal behavior, model loads into memory
   - Monitor: Subsequent requests should use less memory

4. **Apply fixes:**
   ```bash
   # Restart service
   docker compose restart <service-name>
   
   # Or scale down temporarily
   docker compose up -d --scale <service>=1
   ```

5. **Monitor:**
   ```bash
   # Watch resource usage
   watch -n 1 docker stats
   ```

**Success Criteria:**
- Resource usage returns to normal
- Services remain responsive
- No service crashes

---

## Architecture

### System Architecture

The platform uses a **Tri-Layer Phoenix Architecture** with four distinct layers:

```
┌─────────────────────────────────────────────────────────┐
│                    Client Layer                         │
│  Frontend (React/TypeScript) - Port 3000               │
└────────────────────┬──────────────────────────────────┘
                     │ HTTP/WebSocket/SSE
┌────────────────────▼──────────────────────────────────┐
│                 Gateway Layer                          │
│  Rust Gateway - Port 8181                             │
│  • WebSocket ingress                                   │
│  • HTTP proxy                                          │
│  • SSE proxy                                           │
└────────────────────┬──────────────────────────────────┘
                     │ HTTP
┌────────────────────▼──────────────────────────────────┐
│              Orchestrator Layer                        │
│  Rust Orchestrator - Port 8182                         │
│  • LLM planning (OpenRouter)                          │
│  • Human-in-the-loop gating                           │
│  • Action routing                                      │
└────┬──────────────────────┬──────────────────────────┘
     │ gRPC                  │ gRPC
┌────▼──────────┐   ┌────────▼──────────┐
│ Infrastructure│   │  Infrastructure  │
│   Layer       │   │     Layer        │
├───────────────┤   ├──────────────────┤
│ Memory Service│   │  Tools Service   │
│ Port 50052    │   │  Port 50054      │
│               │   │                  │
│ • Qdrant      │   │ • Sandboxed exec │
│ • Vector DB   │   │ • Build Service  │
└───────┬───────┘   └──────────────────┘
        │
        │ gRPC
┌───────▼───────┐
│   Qdrant DB   │
│ Ports 6333/34 │
│ Vector Store  │
└───────────────┘
```

### Data Flow

**Chat Request Flow:**
1. User sends message via Frontend
2. Frontend → Gateway (WebSocket)
3. Gateway → Orchestrator (HTTP POST /v1/chat)
4. Orchestrator plans action (LLM via OpenRouter)
5. If action requires approval → HITL gate → User approves/denies
6. Orchestrator → Memory Service (gRPC) for knowledge queries
7. Orchestrator → Tools Service (gRPC) for tool execution
8. Response flows back through Gateway → Frontend

**Knowledge Ingestion Flow:**
1. File dropped into `data/ingest/` directory
2. Auto-Domain Ingestor detects file (via `notify` crate)
3. Classifies domain (LLM or keyword-based)
4. Chunks content (domain-aware: 256-1024 tokens)
5. Generates embeddings (fastembed)
6. Upserts to Qdrant collection (domain-specific)
7. Updates knowledge base stats in UI
8. Frontend polls `/api/knowledge/ingest/status` every 2 seconds
9. Ingestion Progress Dashboard displays real-time progress with animated bars
10. Toast notification appears on completion with domain classification
11. Attribution Analytics radar chart updates to reflect knowledge base growth

### Key Directories

```
pagi-digital-twin/
├── backend-rust-orchestrator/    # Core orchestrator service
│   ├── src/
│   │   ├── main.rs               # Entry point
│   │   ├── api/                  # HTTP API routes
│   │   ├── knowledge/            # Knowledge base logic
│   │   │   ├── domain_router.rs  # Domain routing (Mind/Body/Heart/Soul)
│   │   │   └── ingestor.rs       # Auto-domain ingestion
│   │   ├── network/              # P2P networking (Blue Flame)
│   │   ├── agents/               # Agent factory and management
│   │   └── tools/                # Tool execution helpers
│   └── config/
│       └── system_prompt.txt      # System prompt template
├── backend-rust-memory/          # Memory service (Qdrant client)
├── backend-rust-tools/           # Tools service (sandboxed execution)
├── backend-rust-gateway/         # Gateway service (edge protocol)
├── backend-rust-telemetry/       # Telemetry service (SSE metrics)
├── backend-rust-build/           # Build service (tool compilation)
├── frontend-digital-twin/        # React frontend
│   ├── src/
│   │   ├── components/           # UI components
│   │   ├── services/             # API clients
│   │   └── context/              # React contexts
│   └── package.json
├── config/                       # Configuration files
│   └── agents/                   # Agent templates
├── test-agent-repo/              # Agent repository
│   ├── agent-templates/          # Agent definitions
│   └── playbooks/                # AI-generated playbooks
├── knowledge_bases/               # Knowledge base source files
├── data/                         # Runtime data
│   └── ingest/                   # Auto-ingestion directory
├── tools_repo/                   # Compiled tools (build service output)
├── scripts/                      # Utility scripts
│   └── run_all_dev.py            # Development harness
├── docs/                         # Documentation
├── docker-compose.yml            # Docker orchestration
├── Makefile                      # Convenience commands
└── .env                          # Environment configuration (not in git)
```

### Component Interactions

**Phoenix Consensus Sync:**
- Mesh-wide voting for agent updates
- mDNS discovery for peer nodes
- Handshake protocol for verification
- See: `backend-rust-orchestrator/src/network/consensus.rs`

**Phoenix Memory Exchange:**
- Peer-to-peer knowledge transfer
- Redaction for privacy
- Topic-based memory sharing
- See: `backend-rust-orchestrator/src/network/memory_exchange.rs`

**Phoenix Fleet Manager:**
- Distributed node registry
- Health monitoring
- Cross-node knowledge sharing
- See: `backend-rust-orchestrator/src/network/fleet.rs`

**Auto-Domain Ingestor:**
- File watching (notify crate)
- Semantic classification (LLM + keyword fallback)
- Domain-aware chunking
- Qdrant upsert
- See: `backend-rust-orchestrator/src/knowledge/ingestor.rs`

---

## API Reference

### Rust Gateway (Port 8181)

Base URL: `http://localhost:8181`

| Method | Endpoint | Description | Request | Response |
|--------|----------|-------------|---------|----------|
| `GET` | `/api/health` | Health check | - | `{"service": "gateway", "status": "ok"}` |
| `GET` | `/ws/chat/:user_id` | WebSocket chat connection | WebSocket upgrade | Real-time messages |
| `GET` | `/ws/signaling/:room_id` | WebSocket signaling for media | WebSocket upgrade | Signaling messages |
| `GET` | `/v1/telemetry/stream` | SSE telemetry proxy | - | Server-Sent Events |
| `POST` | `/api/media/upload` | Media upload proxy | Multipart form data | `{"success": true, "filename": "...", "path": "..."}` |

### Rust Orchestrator (Port 8182)

Base URL: `http://localhost:8182`

#### HTTP Endpoints

| Method | Endpoint | Description | Request Body | Response |
|--------|----------|-------------|--------------|----------|
| `GET` | `/health` | Health check | - | `{"service": "orchestrator", "status": "ok"}` |
| `POST` | `/v1/chat` | Chat request with planning | `{"message": string, "twin_id": string, "session_id": string, "namespace"?: string, "media_active"?: boolean}` | `{"response": string, "job_id": string, "actions_taken": string[], "status": string}` |
| `POST` | `/api/memory/query` | Query memory (semantic search) | `{"query": string, "namespace"?: string, "top_k"?: number, "domains"?: string[]}` | `{"results": MemoryResult[], "total": number, "domain_attribution"?: DomainAttribution}` |
| `POST` | `/api/knowledge/ingest` | Trigger file ingestion | `{"file_path"?: string}` | `{"success": boolean, "message": string}` |
| `GET` | `/api/knowledge/ingest/status` | Get ingestion status | - | `{"status": {"is_active": boolean, "files_processed": number, "files_failed": number, "current_file": string\|null, "last_error": string\|null}}` |

**Ingestion Status Response:**
- `is_active`: Whether a file is currently being processed
- `files_processed`: Total number of successfully ingested files
- `files_failed`: Total number of failed ingestions
- `current_file`: Path of file currently being processed (null if none)
- `last_error`: Error message from last failed ingestion (null if none)

**Note**: The frontend polls this endpoint every 2 seconds while `is_active` is true to provide real-time progress updates in the Ingestion Progress Dashboard.
| `GET` | `/api/knowledge/atlas` | Get knowledge graph data | Query: `?method=pca\|umap&max_nodes=500` | `{"nodes": AtlasNode[], "edges": AtlasEdge[], "total": number}` |
| `POST` | `/api/knowledge/path` | Find semantic path | `{"source_id": string, "target_id": string}` | `{"path": PathStep[], "total_strength": number, "found": boolean}` |

**Example Chat Request:**
```bash
curl -X POST http://localhost:8182/v1/chat \
  -H "Content-Type: application/json" \
  -d '{
    "message": "Search for security threats",
    "twin_id": "twin-1",
    "session_id": "session-123",
    "namespace": "default"
  }'
```

**Example Memory Query:**
```bash
curl -X POST http://localhost:8182/api/memory/query \
  -H "Content-Type: application/json" \
  -d '{
    "query": "security audit procedures",
    "top_k": 10,
    "domains": ["soul", "mind"]
  }'
```

#### gRPC Services

**Orchestrator Service (Port 50057):**
- `SummarizeTranscript` - Analyze transcript and extract decisions
  - Request: `{transcript_text: string}`
  - Response: `{summary: string, key_decisions: string[], follow_up_tasks: string[]}`

**Admin Service (Port 50056):**
- `HealthCheck` - Service health
- `GetPromptHistory` - Retrieve prompt change history
- `UpdateSystemPrompt` - Update system prompt
- TBD: Verify all admin endpoints (check `backend-rust-orchestrator/src/main.rs`)

### Rust Memory Service (Port 50052 - gRPC)

**Service:** `memory.MemoryService`

| Method | Description | Request | Response |
|--------|-------------|---------|----------|
| `CommitMemory` | Store memory fragment | `{content: string, namespace: string, twin_id: string, ...}` | `{memory_id: string, success: boolean}` |
| `QueryMemory` | Semantic search | `{query: string, namespace: string, top_k: number}` | `{results: MemoryResult[]}` |
| `ListMemories` | List memories | `{namespace: string, limit: number}` | `{memories: Memory[]}` |
| `DeleteMemory` | Delete memory | `{memory_id: string}` | `{success: boolean}` |
| `HealthCheck` | Health check | `{}` | `{status: string}` |

**Example (using grpcurl):**
```bash
grpcurl -plaintext -d '{
  "query": "security policies",
  "namespace": "corporate_context",
  "top_k": 5
}' localhost:50052 memory.MemoryService/QueryMemory
```

### Rust Tools Service (Port 50054 - gRPC)

**Service:** `tools.ToolExecutorService`

| Method | Description | Request | Response |
|--------|-------------|---------|----------|
| `ExecuteTool` | Execute tool | `{tool_name: string, args: string[]}` | `{output: string, exit_code: int32}` |
| `HealthCheck` | Health check | `{}` | `{status: string}` |

### Rust Build Service (Port 50055 - gRPC)

**Service:** `build.BuildService`

| Method | Description | Request | Response |
|--------|-------------|---------|----------|
| `CreateTool` | Compile Rust tool | `{tool_name: string, tool_code: string}` | `{stdout: string, stderr: string, exit_code: int32}` |
| `HealthCheck` | Health check | `{}` | `{status: string}` |

### Rust Telemetry Service (Port 8183)

Base URL: `http://localhost:8183`

| Method | Endpoint | Description | Response |
|--------|----------|-------------|----------|
| `GET` | `/v1/telemetry/stream` | SSE stream of metrics | Server-Sent Events with `{"ts_ms": number, "cpu_percent": number, "mem_total": number, "mem_used": number, "process_count": number}` |
| `POST` | `/v1/media/upload` | Upload media files | `{"success": boolean, "filename": string, "stored_path": string}` |

### Qdrant (Ports 6333/6334)

**REST API (6333):**
- Collections: `GET http://localhost:6333/collections`
- Health: `GET http://localhost:6333/health`
- TBD: Verify full API (see Qdrant documentation)

**gRPC API (6334):**
- Used internally by Memory Service
- TBD: Verify gRPC methods (check `backend-rust-memory/src/main.rs`)

### Additional Endpoints

TBD: Verify all endpoints in `backend-rust-orchestrator/src/api/phoenix_routes.rs`:
- Phoenix API endpoints (consensus, memory exchange, fleet)
- Agent management endpoints
- Playbook endpoints
- Scheduled tasks endpoints
- Tool proposal endpoints
- Peer review endpoints
- Retrospective endpoints

---

## Contributing

### Development Setup

1. **Fork and clone:**
   ```bash
   git clone <your-fork-url>
   cd pagi-digital-twin
   ```

2. **Install prerequisites:**
   - Rust toolchain: `rustup install stable`
   - Node.js 18+: `npm --version`
   - Python 3.10+: `python --version`
   - Go 1.21+: `go version`

3. **Set up development environment:**
   ```bash
   # Create .env file
   cp .env.example .env
   # Edit with your API keys
   ```

4. **Run development services:**
   ```bash
   make run-dev  # Starts core services
   ```

### Code Structure

- **Rust services**: Follow Rust conventions, use `cargo fmt` and `cargo clippy`
- **Frontend**: TypeScript/React, use `npm run` scripts
- **Go services**: Follow Go conventions, use `go fmt` and `go vet`
- **Python services**: Follow PEP 8, use `black` formatter

### Linting and Testing

**Rust:**
```bash
cd backend-rust-orchestrator
cargo fmt
cargo clippy
cargo test
```

**Frontend:**
```bash
cd frontend-digital-twin
npm run lint  # TBD: Verify if lint script exists
npm test      # TBD: Verify if tests exist
```

**Go:**
```bash
cd backend-go-model-gateway
go fmt ./...
go vet ./...
go test ./...
```

### Pull Request Process

1. Create feature branch from `main`
2. Make changes with clear commits
3. Ensure all linting/tests pass
4. Update documentation if needed
5. Submit PR with description of changes
6. Address review feedback

### Documentation

- Update README.md for user-facing changes
- Add code comments for complex logic
- Update API documentation if endpoints change
- Add examples for new features

---

## License

[Add your license information here]

For detailed architecture documentation, implementation choices, and production recommendations, see [`docs/PROJECT_DELIVERY_SUMMARY.md`](docs/PROJECT_DELIVERY_SUMMARY.md).
