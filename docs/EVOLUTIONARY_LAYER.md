# 🚀 Evolutionary Layer: The "Blue Flame" Intelligence Factory

The Evolutionary Layer transforms your Phoenix Foundry from a "Builder" into an "Enterprise Intelligence Factory" by adding version control, testing, and compatibility management for agents and tools.

## 🏛️ Architecture Overview

The Evolutionary Layer consists of three major components:

### 1. **AgentForge** - IT-Strategic Layer
Build and evolve agents with git-based version control and rollback capabilities.

### 2. **ToolForge** - Technical Excellence Layer  
Manage tools with semantic versioning and breaking change detection.

### 3. **AgentWarRoom** - Testing & Compliance Suite
Test agents in sandbox environments with compliance grading.

---

## 📦 Components

### AgentForge

**Location:** [`frontend-digital-twin/components/AgentForge.tsx`](../frontend-digital-twin/components/AgentForge.tsx)

**Features:**
- **Agent Editor**: Edit agent manifests, prompts, and tool assignments
- **Version History**: View git commit history for each agent
- **Rollback**: One-click revert to previous versions
- **Discovery Refresh**: Automatically trigger agent mesh updates

**Usage:**
```typescript
import AgentForge from '@/components/AgentForge';

<AgentForge className="h-full" />
```

---

### ForgeHistory

**Location:** [`frontend-digital-twin/components/ForgeHistory.tsx`](../frontend-digital-twin/components/ForgeHistory.tsx)

**Features:**
- **Git Integration**: Uses `git2` on the backend to fetch commit history
- **Diff Viewer**: Shows exactly what changed between versions
- **Audit Trail**: Tracks who (human or AI) made changes and why
- **Revert Logic**: Performs `git checkout [COMMIT_HASH] -- [FILE_PATH]` and creates a new commit

**API Endpoints:**
- `GET /api/agents/:id/history` - Fetch commit history
- `GET /api/agents/:id/diff/:commit` - Get diff for a specific commit
- `POST /api/agents/:id/revert` - Revert to a specific version

**Example Response:**
```json
{
  "hash": "a1b2c3d",
  "author": "admin",
  "timestamp": "2026-01-14T00:00:00Z",
  "message": "Updated network scanner prompt",
  "files": ["agents/network-scanner/prompt.txt"],
  "is_active": true
}
```

---

### AgentWarRoom

**Location:** [`frontend-digital-twin/components/AgentWarRoom.tsx`](../frontend-digital-twin/components/AgentWarRoom.tsx)

**Features:**
- **Scenario Runner**: Define test missions for agents
- **Trace Visualization**: Display agent's "Thought → Action → Observation" loop
- **Compliance Grading**: Validate against Ferrellgas standards
  - **Privacy**: Did it access redacted IPs?
  - **Efficiency**: Did it use the correct tools in the right order?
  - **Tone**: Did it maintain the "Visionary Architect" persona?

**API Endpoints:**
- `POST /api/agents/:id/test` - Run a test mission (returns SSE stream)

**Example Test Mission:**
```json
{
  "mission": "Scan the network and summarize findings",
  "enable_compliance": true
}
```

**Example Trace Step:**
```json
{
  "type": "thought",
  "content": "Analyzing mission: Scan the network...",
  "timestamp": "2026-01-14T00:00:00Z",
  "tool": null
}
```

**Example Compliance Result:**
```json
{
  "privacy": {
    "passed": true,
    "details": "No sensitive data accessed"
  },
  "efficiency": {
    "passed": true,
    "details": "Used optimal tool sequence"
  },
  "tone": {
    "passed": true,
    "details": "Maintained professional tone"
  }
}
```

---

### ToolForge

**Location:** [`frontend-digital-twin/components/ToolForge.tsx`](../frontend-digital-twin/components/ToolForge.tsx)

**Features:**
- **Semantic Versioning**: Auto-increment tool versions (major.minor.patch)
- **Breaking Change Detector**: Flags agents when tool input schemas change
- **Deprecation Path**: Mark tools as "Legacy" with upgrade prompts
- **Dependency Tracking**: Shows which agents use each tool

**API Endpoints:**
- `GET /api/tools` - List all tools
- `PUT /api/tools/:id` - Update a tool (auto-increments version)
- `POST /api/tools/:id/mark-legacy` - Mark tool as legacy

**Example Tool:**
```json
{
  "id": "nmap",
  "name": "Nmap Scanner",
  "description": "Network mapping and port scanning tool",
  "version": "1.0.0",
  "input_schema": {
    "type": "object",
    "properties": {
      "target": { "type": "string" },
      "ports": { "type": "string" }
    }
  },
  "script": "#!/bin/bash\nnmap $1",
  "status": "active",
  "dependent_agents": ["network-scanner"],
  "breaking_changes": false
}
```

**Breaking Change Detection:**
When a tool's `input_schema` changes, the backend flags all dependent agents:
```json
{
  "success": true,
  "breakingChanges": true,
  "affectedAgents": ["network-scanner", "vulnerability-scanner"]
}
```

---

## 🔧 Backend Implementation

### Rust Endpoints

**Location:** [`backend-rust-orchestrator/src/foundry/forge_api.rs`](../backend-rust-orchestrator/src/foundry/forge_api.rs)

**Key Functions:**
- `list_agents()` - List all agents
- `get_agent_history()` - Fetch git commit history for an agent
- `get_agent_diff()` - Get diff between commits
- `revert_agent()` - Revert agent to a specific commit
- `test_agent()` - Run agent test mission (SSE stream)
- `list_tools()` - List all tools
- `update_tool()` - Update tool and detect breaking changes
- `mark_tool_legacy()` - Mark tool as legacy

**Git2 Integration:**
```rust
use git2::{Commit, DiffOptions, Repository};

let repo = Repository::open(&state.agents_repo_path)?;
let mut revwalk = repo.revwalk()?;
revwalk.push_head()?;

for oid in revwalk {
    let commit = repo.find_commit(oid?)?;
    // Process commit...
}
```

**Revert Logic:**
```rust
// Checkout specific files from commit
let tree = commit.tree()?;
let mut checkout_builder = git2::build::CheckoutBuilder::new();
checkout_builder.path(format!("agents/{}/", agent_id));
checkout_builder.force();

repo.checkout_tree(tree.as_object(), Some(&mut checkout_builder))?;

// Create new commit
repo.commit(
    Some("HEAD"),
    &signature,
    &signature,
    &format!("Revert agent {} to commit {}", agent_id, commit_hash),
    &new_tree,
    &[&parent_commit],
)?;
```

---

## 🎯 Use Cases

### 1. **Rollback a Hallucinating Agent**
If a new prompt causes an agent to drift from Ferrellgas values:
1. Open **AgentForge**
2. Select the agent
3. Switch to **History** tab
4. Click "Revert to This Version" on a previous commit
5. The agent is instantly restored

### 2. **Test Before Production**
Before deploying a new agent:
1. Open **AgentForge**
2. Select the agent
3. Switch to **War Room** tab
4. Enter a test mission
5. Enable **Compliance Check**
6. Review the trace and compliance results
7. Fix any issues before deploying

### 3. **Prevent Zombie Agents**
When updating a tool:
1. Open **ToolForge**
2. Select the tool
3. Modify the `input_schema`
4. Click "Save & Version"
5. If breaking changes are detected, a warning shows affected agents
6. Update those agents in **AgentForge** before deploying

---

## 🔐 Security & Compliance

### Privacy Protection
- **Redacted IP Check**: War Room validates agents don't access sensitive IPs
- **Data Access Audit**: All agent actions are logged with timestamps

### Ferrellgas Compliance
- **Tone Validation**: Ensures agents maintain "Visionary Architect" persona
- **Tool Usage**: Validates agents use approved tools only
- **Efficiency**: Checks for optimal tool sequences

### Audit Trail
Every agent change is tracked:
- **Who**: Human or AI user
- **When**: Timestamp
- **What**: Commit message and diff
- **Why**: Change rationale

---

## 📊 Benefits

| Feature | Phoenix Benefit |
| --- | --- |
| **Rollback UI** | Protects against "Model Drift" and accidental prompt injection |
| **War Room** | Ensures high-stakes tasks are "Safe-to-Fail" in a sandbox first |
| **Compatibility Guard** | Maintains mesh stability as your tool library grows to hundreds of scripts |
| **Version Control** | Full audit trail for compliance and debugging |
| **Breaking Change Detection** | Prevents "Zombie Agents" from breaking in production |

---

## 🚀 Getting Started

### Prerequisites
- Git repository for agents (default: `./agents`)
- Git repository for tools (default: `./tools`)
- Rust backend with `git2` dependency

### Installation

1. **Add git2 to Cargo.toml:**
```toml
[dependencies]
git2 = "0.18"
chrono = "0.4"
```

2. **Initialize Git Repositories:**
```bash
cd agents && git init
cd ../tools && git init
```

3. **Start the Backend:**
```bash
cd backend-rust-orchestrator && cargo run
```

4. **Access the UI:**
Navigate to `http://localhost:3000` and open **AgentForge** or **ToolForge**.

---

## 🎓 Best Practices

### Agent Development
1. **Test First**: Always test in War Room before deploying
2. **Small Changes**: Make incremental changes with clear commit messages
3. **Compliance Check**: Enable compliance grading for all tests
4. **Version Tags**: Use semantic versioning for major agent updates

### Tool Development
1. **Schema Stability**: Avoid breaking changes to `input_schema`
2. **Deprecation Path**: Mark old tools as legacy before removing
3. **Documentation**: Update tool descriptions with each version
4. **Dependency Tracking**: Review dependent agents before updates

### Rollback Strategy
1. **Immediate Rollback**: If an agent misbehaves, revert immediately
2. **Root Cause Analysis**: Review the diff to understand what changed
3. **Fix Forward**: After rollback, fix the issue and redeploy
4. **Document**: Add commit message explaining the rollback reason

---

## 🔮 Future Enhancements

- **Multi-Agent Testing**: Test interactions between multiple agents
- **Performance Benchmarks**: Track agent response times and resource usage
- **A/B Testing**: Compare two agent versions side-by-side
- **Auto-Rollback**: Automatically revert agents that fail compliance checks
- **Tool Marketplace**: Share and discover tools across teams

---

## 📚 Related Documentation

- [Phoenix Foundry Setup](../backend-rust-orchestrator/README_SOVEREIGN_SETUP.md)
- [Agent Discovery](../backend-rust-orchestrator/src/agents/README.md)
- [Tool Development](../backend-rust-tools/README.md)
- [Project Delivery Summary](./PROJECT_DELIVERY_SUMMARY.md)

---

## 🤝 Contributing

To add new features to the Evolutionary Layer:

1. **Frontend**: Add components to `frontend-digital-twin/components/`
2. **Backend**: Add endpoints to `backend-rust-orchestrator/src/foundry/forge_api.rs`
3. **Documentation**: Update this file with new features
4. **Testing**: Add test missions to War Room

---

## 📞 Support

For questions or issues:
- **GitHub Issues**: [pagi-digital-twin/issues](https://github.com/your-org/pagi-digital-twin/issues)
- **Slack**: #phoenix-foundry
- **Email**: support@ferrellgas.com

---

**Built with 💙 by the Ferrellgas Phoenix AI Team**
