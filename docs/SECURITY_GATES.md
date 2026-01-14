# Security Gates Reference

This document provides a comprehensive reference for all security gates available in the Ferrellgas AGI Multi Digital Twin Platform. These gates allow controlled access to restricted capabilities for research purposes.

## Overview

Security gates are environment variables that enable features normally restricted for safety. They are designed for **research projects only** and should be used with extreme caution in isolated environments.

## Network Scanning Security Gates

### `ALLOW_PUBLIC_NETWORK_SCAN`

**Default**: `false` (disabled)

**Purpose**: Enables network scanning of public IP addresses (not just RFC1918 private subnets).

**Usage**:
```bash
ALLOW_PUBLIC_NETWORK_SCAN=1
NETWORK_SCAN_HITL_TOKEN=your-secure-token-here
```

**Requirements**:
- Must be set to `1`, `true`, `yes`, or `on` to enable
- Requires `NETWORK_SCAN_HITL_TOKEN` to be set
- Public scan requests must include the matching HITL token

**Security Impact**: ⚠️ **HIGH** - Allows scanning external networks, which may violate terms of service or be illegal in some jurisdictions.

---

### `NETWORK_SCAN_HITL_TOKEN`

**Default**: Unset

**Purpose**: HITL token required for public network scans when `ALLOW_PUBLIC_NETWORK_SCAN=1`.

**Usage**:
```bash
NETWORK_SCAN_HITL_TOKEN=your-secure-random-token-here
```

**Requirements**:
- Must be set when `ALLOW_PUBLIC_NETWORK_SCAN=1`
- Use a strong, random token (e.g., `openssl rand -hex 32`)
- Must be provided in scan requests for public IP targets

**Security Impact**: ⚠️ **MEDIUM** - Acts as an additional authentication layer for public scans.

---

### `ALLOW_IPV6_NETWORK_SCAN`

**Default**: `false` (disabled)

**Purpose**: Enables IPv6 network scanning support.

**Usage**:
```bash
ALLOW_IPV6_NETWORK_SCAN=1
```

**Current Status**: 
- Gate is implemented and checked
- Full IPv6 parsing support is planned but not yet fully implemented
- Currently returns an informative error message when IPv6 targets are detected

**Security Impact**: ⚠️ **LOW** - Currently informational only, full implementation pending.

---

### `ALLOW_ARBITRARY_PORT_SCAN`

**Default**: `false` (disabled)

**Purpose**: Allows custom port ranges for network scanning (not just ports 8281-8284).

**Usage**:
```bash
ALLOW_ARBITRARY_PORT_SCAN=1
```

**Port Format**:
- Comma-separated: `"22,80,443,8080"`
- Range: `"1-65535"`
- Default (when disabled): `"8281-8284"` (AGI core ports only)

**Example Request**:
```json
{
  "target": "192.168.1.0/24",
  "ports": "22,80,443,8080",
  "twin_id": "twin-aegis"
}
```

**Security Impact**: ⚠️ **MEDIUM** - Allows comprehensive port scanning which may be detected by network security systems.

---

## HITL (Human-In-The-Loop) Bypass Gates

### `BYPASS_HITL_TOOL_EXEC`

**Default**: `false` (disabled)

**Purpose**: Bypasses human approval requirement for `tool_exec` actions.

**Usage**:
```bash
BYPASS_HITL_TOOL_EXEC=1
```

**What It Bypasses**:
- Normal HITL flow requires human operator to approve tool execution plans
- When enabled, `tool_exec` actions proceed automatically without approval

**Security Impact**: ⚠️ **CRITICAL** - Removes a critical safety check. Tools can execute commands automatically without oversight.

**Implementation Note**: This gate is available as a helper function. Full integration requires updating the frontend/LLM prompt handling to check this gate.

---

### `BYPASS_HITL_MEMORY`

**Default**: `false` (disabled)

**Purpose**: Bypasses human approval requirement for memory operations (`memory_query`, `memory_commit`).

**Usage**:
```bash
BYPASS_HITL_MEMORY=1
```

**What It Bypasses**:
- Normal HITL flow requires human operator to approve memory queries and commits
- When enabled, memory operations proceed automatically without approval

**Security Impact**: ⚠️ **HIGH** - Allows unrestricted memory access without oversight. Could lead to data leakage or unauthorized data modification.

**Implementation Note**: This gate is available as a helper function. Full integration requires updating the frontend/LLM prompt handling to check this gate.

---

### `BYPASS_HITL_KILL_PROCESS`

**Default**: `false` (disabled)

**Purpose**: Bypasses human approval requirement for `kill_process` actions.

**Usage**:
```bash
BYPASS_HITL_KILL_PROCESS=1
```

**What It Bypasses**:
- Normal HITL flow requires human operator to approve process termination
- When enabled, `kill_process` actions proceed automatically without approval

**Security Impact**: ⚠️ **CRITICAL** - Allows terminating any process without oversight. Could cause system instability or service disruption.

**Implementation Note**: This gate is available as a helper function. Full integration requires updating the frontend/LLM prompt handling to check this gate.

---

### `BYPASS_EMAIL_TEAMS_APPROVAL`

**Default**: `false` (disabled)

**Purpose**: Bypasses user approval requirement for email and Teams message sending.

**Usage**:
```bash
BYPASS_EMAIL_TEAMS_APPROVAL=1
```

**What It Bypasses**:
- Normal flow requires user approval before sending emails or Teams messages
- When enabled, `send_email` and `send_teams_message` actions proceed automatically

**Security Impact**: ⚠️ **CRITICAL** - Allows sending messages on your behalf automatically. Could lead to unauthorized communication or information disclosure.

**Implementation Note**: This gate is available as a helper function. Full integration requires updating the email/Teams service handlers to check this gate.

---

## Restricted Commands Security Gate

### `ALLOW_RESTRICTED_COMMANDS`

**Default**: `false` (disabled)

**Purpose**: Allows execution of normally restricted commands.

**Usage**:
```bash
ALLOW_RESTRICTED_COMMANDS=1
```

**Restricted Commands**:
- `rm` - Remove/delete files
- `delete` - Delete operations
- `format` - Disk formatting
- `shutdown` - System shutdown
- `reboot` - System reboot

**Security Impact**: ⚠️ **CRITICAL** - These commands can cause:
- Data loss (rm, delete, format)
- System unavailability (shutdown, reboot)
- Permanent damage to filesystems (format)

**Additional Restrictions**:
- Even with this enabled, `SAFE_MODE=true` will still restrict these commands
- The Tools Service enforces this restriction at the service level

**Implementation**: ✅ **FULLY IMPLEMENTED** - The Tools Service checks this gate when authorizing command execution.

---

## Security Gate Status Summary

| Security Gate | Status | Implementation Level |
|--------------|--------|---------------------|
| `ALLOW_PUBLIC_NETWORK_SCAN` | ✅ Active | Fully implemented |
| `NETWORK_SCAN_HITL_TOKEN` | ✅ Active | Fully implemented |
| `ALLOW_IPV6_NETWORK_SCAN` | ⚠️ Partial | Gate exists, full IPv6 support pending |
| `ALLOW_ARBITRARY_PORT_SCAN` | ✅ Active | Fully implemented |
| `BYPASS_HITL_TOOL_EXEC` | 🔧 Helper | Function available, integration pending |
| `BYPASS_HITL_MEMORY` | 🔧 Helper | Function available, integration pending |
| `BYPASS_HITL_KILL_PROCESS` | 🔧 Helper | Function available, integration pending |
| `BYPASS_EMAIL_TEAMS_APPROVAL` | 🔧 Helper | Function available, integration pending |
| `ALLOW_RESTRICTED_COMMANDS` | ✅ Active | Fully implemented |

**Legend**:
- ✅ **Active**: Fully implemented and functional
- ⚠️ **Partial**: Partially implemented or pending full support
- 🔧 **Helper**: Helper function available, requires integration into handlers

---

## Best Practices

1. **Use in Isolated Environments**: Only enable security gates in isolated research environments, never in production.

2. **Document Your Usage**: Keep a record of which gates you've enabled and why.

3. **Use Strong Tokens**: When using `NETWORK_SCAN_HITL_TOKEN`, generate a strong, random token:
   ```bash
   openssl rand -hex 32
   ```

4. **Monitor Activity**: When gates are enabled, monitor system activity closely for unexpected behavior.

5. **Disable When Not Needed**: Disable security gates when not actively using them.

6. **Review Regularly**: Regularly review which gates are enabled and whether they're still needed.

---

## Integration Notes

Some security gates (HITL bypass gates) are implemented as helper functions but require integration into the actual request handlers. To fully enable these:

1. **For HITL Bypass Gates**: Update the frontend/LLM prompt handling to check these gates before requiring approval.

2. **For Email/Teams Bypass**: Update the email and Teams service handlers to check `BYPASS_EMAIL_TEAMS_APPROVAL` before requiring user approval.

3. **Testing**: Test all security gates in isolated environments before use.

---

## Questions or Issues

If you encounter issues with security gates or need clarification on their usage, please:
1. Review this document
2. Check `ENV_SETUP.md` for general environment configuration
3. Review the source code comments in `backend-rust-orchestrator/src/main.rs` and `backend-rust-tools/src/main.rs`
