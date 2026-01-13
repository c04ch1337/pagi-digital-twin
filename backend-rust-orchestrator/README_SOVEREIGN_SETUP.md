# Sovereign Permissions Setup Guide

This guide explains how to configure the bare-metal environment to allow the Orchestrator service to execute system commands without manual password entry.

## Overview

The Orchestrator's auto-repair workflow (P55) requires the ability to:
- Restart services using `systemctl`
- Inspect service logs using `journalctl`
- Access telemetry storage directories

To enable these capabilities securely, we create a dedicated system user with limited sudo permissions.

## Quick Setup (Linux)

### Automated Setup

Run the setup script as root:

```bash
sudo ./scripts/setup_sovereign_permissions.sh [TELEMETRY_STORAGE_DIR]
```

Example:
```bash
sudo ./scripts/setup_sovereign_permissions.sh /var/lib/telemetry/storage
```

### Manual Setup

If you prefer to set up manually:

#### 1. Create System User

```bash
sudo useradd -r -s /bin/bash -d /var/lib/agi-orchestrator -m agi-orchestrator
```

#### 2. Configure Sudoers

Copy the sudoers template:
```bash
sudo cp backend-rust-orchestrator/config/sudoers.agi-orchestrator /etc/sudoers.d/agi-orchestrator
sudo chmod 0440 /etc/sudoers.d/agi-orchestrator
```

Validate the sudoers file:
```bash
sudo visudo -c -f /etc/sudoers.d/agi-orchestrator
```

#### 3. Configure Storage Permissions

Set ownership and permissions on the telemetry storage directory:
```bash
# Replace with your actual storage directory path
TELEMETRY_STORAGE_DIR="/var/lib/telemetry/storage"

sudo mkdir -p "${TELEMETRY_STORAGE_DIR}/recordings"
sudo chown -R agi-orchestrator:agi-orchestrator "${TELEMETRY_STORAGE_DIR}"
sudo chmod -R 755 "${TELEMETRY_STORAGE_DIR}"
```

## Windows Setup

For Windows environments, run the PowerShell script as Administrator:

```powershell
.\scripts\setups_sovereign_permissions.ps1 [TELEMETRY_STORAGE_DIR]
```

**Note:** Windows uses a different permission model. The script will:
- Create the `agi-orchestrator` user account
- Configure directory permissions using `icacls`
- Set up the storage directory structure

Service management on Windows is handled through:
- Windows Services Manager (`services.msc`)
- `sc.exe` command-line tool
- PowerShell cmdlets (`Get-Service`, `Start-Service`, etc.)

## Verification

### Test Sudo Access (Linux)

Test that the orchestrator user can execute systemctl commands without a password:

```bash
sudo -u agi-orchestrator sudo systemctl status telemetry
sudo -u agi-orchestrator sudo journalctl -u telemetry -n 50
```

### Test Storage Access

Verify the orchestrator user can access the storage directory:

```bash
sudo -u agi-orchestrator ls -la /var/lib/telemetry/storage
sudo -u agi-orchestrator touch /var/lib/telemetry/storage/test.txt
sudo -u agi-orchestrator rm /var/lib/telemetry/storage/test.txt
```

## Service Configuration

After setting up permissions, configure your services to run as the `agi-orchestrator` user.

### systemd Service Example

Create a systemd service file at `/etc/systemd/system/backend-rust-orchestrator.service`:

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
RestartSec=10

# Environment variables
Environment="TELEMETRY_STORAGE_DIR=/var/lib/telemetry/storage"
Environment="ORCHESTRATOR_HTTP_PORT=8182"
Environment="LLM_PROVIDER=openrouter"

[Install]
WantedBy=multi-user.target
```

Then enable and start the service:
```bash
sudo systemctl daemon-reload
sudo systemctl enable backend-rust-orchestrator
sudo systemctl start backend-rust-orchestrator
```

## Security Considerations

### Principle of Least Privilege

The sudoers configuration grants **only** the specific commands needed for service management:
- `systemctl` commands for telemetry, gateway, and sys_control services
- `journalctl` commands for log inspection

The `agi-orchestrator` user does **not** have:
- General sudo access
- Ability to modify system files
- Access to other users' data

### User Account Security

The `agi-orchestrator` user is configured as:
- **System account** (`-r` flag): No login shell by default
- **Restricted shell**: `/bin/bash` but account should not be used for interactive login
- **Home directory**: `/var/lib/agi-orchestrator` (system directory)

### Sudoers File Security

- File permissions: `0440` (read-only for owner and group, no world access)
- Location: `/etc/sudoers.d/` (modular sudoers configuration)
- Validation: Always validate with `visudo -c` before deployment

## Troubleshooting

### Permission Denied Errors

If you see permission denied errors:

1. **Check user exists:**
   ```bash
   id agi-orchestrator
   ```

2. **Verify sudoers file:**
   ```bash
   sudo visudo -c -f /etc/sudoers.d/agi-orchestrator
   ```

3. **Test sudo access:**
   ```bash
   sudo -u agi-orchestrator sudo -n systemctl status telemetry
   ```
   (The `-n` flag tests non-interactive mode)

### Storage Access Issues

If the orchestrator cannot access storage:

1. **Check ownership:**
   ```bash
   ls -ld /var/lib/telemetry/storage
   ```

2. **Verify permissions:**
   ```bash
   sudo -u agi-orchestrator ls -la /var/lib/telemetry/storage
   ```

3. **Fix ownership if needed:**
   ```bash
   sudo chown -R agi-orchestrator:agi-orchestrator /var/lib/telemetry/storage
   ```

### Service Restart Failures

If service restarts fail:

1. **Check systemctl path:**
   ```bash
   which systemctl
   ```
   Update sudoers file if path differs from `/usr/bin/systemctl`

2. **Verify service name:**
   Ensure service names match exactly (case-sensitive)

3. **Check service status:**
   ```bash
   sudo systemctl list-units | grep -E "(telemetry|gateway|sys_control)"
   ```

## Environment Variables

Set the following environment variables for the orchestrator service:

```bash
# Storage directory (must match the directory configured above)
export TELEMETRY_STORAGE_DIR="/var/lib/telemetry/storage"

# Health check interval (optional, default: 30 seconds)
export HEALTH_CHECK_INTERVAL_SECS=30
```

## Related Documentation

- [P54: Sovereign OS Tools](../README.md#p54-sovereign-os-tools) - System tools implementation
- [P55: Auto-Repair Workflow](../README.md#p55-auto-repair-workflow) - Health check and repair logic
- [System Prompt Configuration](config/system_prompt.txt) - LLM instructions for service recovery

## Support

For issues or questions:
1. Check the troubleshooting section above
2. Review service logs: `sudo journalctl -u backend-rust-orchestrator -n 100`
3. Verify permissions: `sudo -u agi-orchestrator sudo -l`
