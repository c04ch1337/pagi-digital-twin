# Sovereign Permissions Configuration

This document describes the bare-metal environment configuration required for the Orchestrator's auto-repair workflow (P55) to function properly.

## Overview

The Orchestrator needs to execute system commands (via System Tools from P54) to:
- Restart services when they go offline
- Inspect service logs for diagnostics
- Access telemetry storage directories

To enable this securely, we configure:
1. **Dedicated System User**: `agi-orchestrator` - runs the orchestrator service
2. **Sudoers Configuration**: Passwordless sudo for specific systemctl/journalctl commands
3. **Storage Permissions**: Proper ownership and permissions for telemetry storage

## Quick Start

### Linux

```bash
# Run the automated setup script
sudo ./scripts/setup_sovereign_permissions.sh /var/lib/telemetry/storage
```

### Windows

```powershell
# Run the PowerShell setup script (as Administrator)
.\scripts\setup_sovereign_permissions.ps1 C:\telemetry\storage
```

## Configuration Details

### 1. System User: `agi-orchestrator`

**Purpose**: Dedicated service account for running the orchestrator

**Creation**:
```bash
sudo useradd -r -s /bin/bash -d /var/lib/agi-orchestrator -m agi-orchestrator
```

**Properties**:
- System account (`-r` flag)
- Home directory: `/var/lib/agi-orchestrator`
- Shell: `/bin/bash` (but not for interactive login)

### 2. Sudoers Configuration

**File**: `/etc/sudoers.d/agi-orchestrator`

**Permissions Granted**:
- `systemctl restart/start/stop/status/enable/disable` for:
  - `telemetry`
  - `gateway`
  - `sys_control`
- `journalctl -u <service> *` for log inspection

**Security**:
- Only specific commands allowed (principle of least privilege)
- No password required (NOPASSWD)
- File permissions: `0440` (read-only, owner and group only)

**Template**: See `config/sudoers.agi-orchestrator`

### 3. Storage Directory Permissions

**Default Location**: `./storage` (or `TELEMETRY_STORAGE_DIR` environment variable)

**Required Permissions**:
- Owner: `agi-orchestrator:agi-orchestrator`
- Permissions: `755` (rwxr-xr-x)
- Subdirectories: `recordings/` for media files

**Setup**:
```bash
TELEMETRY_STORAGE_DIR="/var/lib/telemetry/storage"
sudo mkdir -p "${TELEMETRY_STORAGE_DIR}/recordings"
sudo chown -R agi-orchestrator:agi-orchestrator "${TELEMETRY_STORAGE_DIR}"
sudo chmod -R 755 "${TELEMETRY_STORAGE_DIR}"
```

## Integration with System Tools

The System Tools (P54) use these permissions:

### `run_command`
- Executes: `sudo journalctl -u telemetry -n 50`
- Requires: Sudoers permission for `journalctl`

### `systemctl`
- Executes: `sudo systemctl restart telemetry`
- Requires: Sudoers permission for `systemctl restart`

### `read_file` / `write_file`
- Accesses: Files in `TELEMETRY_STORAGE_DIR`
- Requires: File system permissions (ownership)

## Service Configuration

After setting up permissions, configure the orchestrator service to run as `agi-orchestrator`:

### systemd Service File

```ini
[Unit]
Description=Ferrellgas AGI Orchestrator Service
After=network.target

[Service]
Type=simple
User=agi-orchestrator
Group=agi-orchestrator
WorkingDirectory=/opt/agi/backend-rust-orchestrator
ExecStart=/opt/agi/backend-rust-orchestrator/target/release/backend-rust-orchestrator
Restart=always

Environment="TELEMETRY_STORAGE_DIR=/var/lib/telemetry/storage"
Environment="ORCHESTRATOR_HTTP_PORT=8182"
Environment="LLM_PROVIDER=openrouter"
Environment="HEALTH_CHECK_INTERVAL_SECS=30"

[Install]
WantedBy=multi-user.target
```

## Verification

### Test Sudo Access

```bash
# Test systemctl access
sudo -u agi-orchestrator sudo -n systemctl status telemetry

# Test journalctl access
sudo -u agi-orchestrator sudo -n journalctl -u telemetry -n 10
```

### Test Storage Access

```bash
# Test read access
sudo -u agi-orchestrator ls -la /var/lib/telemetry/storage

# Test write access
sudo -u agi-orchestrator touch /var/lib/telemetry/storage/test.txt
sudo -u agi-orchestrator rm /var/lib/telemetry/storage/test.txt
```

## Troubleshooting

### Permission Denied

**Symptom**: `sudo: a password is required`

**Solution**:
1. Verify sudoers file exists: `ls -l /etc/sudoers.d/agi-orchestrator`
2. Check file permissions: `chmod 0440 /etc/sudoers.d/agi-orchestrator`
3. Validate syntax: `sudo visudo -c -f /etc/sudoers.d/agi-orchestrator`
4. Test access: `sudo -u agi-orchestrator sudo -l`

### Storage Access Denied

**Symptom**: Cannot read/write files in storage directory

**Solution**:
1. Check ownership: `ls -ld /var/lib/telemetry/storage`
2. Fix ownership: `sudo chown -R agi-orchestrator:agi-orchestrator /var/lib/telemetry/storage`
3. Fix permissions: `sudo chmod -R 755 /var/lib/telemetry/storage`

### Service Restart Fails

**Symptom**: `systemctl restart` returns error

**Solution**:
1. Verify service exists: `systemctl list-units | grep telemetry`
2. Check service name (case-sensitive)
3. Verify systemctl path: `which systemctl` (should be `/usr/bin/systemctl`)
4. Check service logs: `sudo journalctl -u backend-rust-orchestrator -n 50`

## Security Best Practices

1. **Principle of Least Privilege**: Only grant the minimum permissions needed
2. **Regular Audits**: Review sudoers file periodically
3. **Service Isolation**: Run each service as its own user when possible
4. **Log Monitoring**: Monitor sudo access logs: `sudo grep agi-orchestrator /var/log/auth.log`
5. **File Permissions**: Keep storage directories with restrictive permissions (755)

## Related Documentation

- [P54: Sovereign OS Tools](../README.md#p54-sovereign-os-tools) - System tools implementation
- [P55: Auto-Repair Workflow](../README.md#p55-auto-repair-workflow) - Health check and repair logic
- [Setup Guide](README_SOVEREIGN_SETUP.md) - Detailed setup instructions
