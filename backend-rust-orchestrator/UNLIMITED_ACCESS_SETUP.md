# OS-Specific "Unlimited Access" Setup Guide

This guide explains how to configure the host OS to grant the Orchestrator "Unlimited Access" for cross-platform service management. This is the "Bare Metal" permission layer that allows the AI to manage services across different research nodes without getting "stuck" when moving between environments.

## Overview

The Orchestrator's cross-platform SystemManager (`manage_service` and `get_logs` functions) requires OS-specific permissions to:

- **Linux**: Execute `systemctl` and `journalctl` commands without password prompts
- **Windows**: Execute `sc.exe` and PowerShell commands with Administrator privileges
- **macOS**: Execute `launchctl` and `log show` commands with sudo access

## 🐧 Linux (Systemd) Setup

### Step 1: Create Sudoers Configuration

Create `/etc/sudoers.d/agi-orchestrator` with unlimited access:

```bash
sudo tee /etc/sudoers.d/agi-orchestrator > /dev/null <<EOF
# Ferrellgas AGI Orchestrator - Unlimited Access Configuration
# This grants passwordless sudo access to systemctl and journalctl commands
# for cross-platform service management (P54.V2)

agi-orchestrator ALL=(ALL) NOPASSWD: /usr/bin/systemctl *, /usr/bin/journalctl *
EOF
```

### Step 2: Set Proper Permissions

```bash
sudo chmod 0440 /etc/sudoers.d/agi-orchestrator
```

### Step 3: Validate Configuration

```bash
sudo visudo -c -f /etc/sudoers.d/agi-orchestrator
```

Expected output: `/etc/sudoers.d/agi-orchestrator: parsed OK`

### Step 4: Verify Access

Test that the orchestrator user can execute systemctl commands without a password:

```bash
sudo -u agi-orchestrator sudo -n systemctl status telemetry
sudo -u agi-orchestrator sudo -n journalctl -u telemetry -n 50
```

### Alternative: Restricted Access (Production)

For production environments, you may prefer restricted access. See `config/sudoers.agi-orchestrator` for a more restrictive configuration that only allows specific service management commands.

## 🪟 Windows (Services) Setup

### Step 1: Run Orchestrator with Administrative Privileges

The Orchestrator binary **must** be run with Administrative Privileges on Windows. This can be done by:

**Option A: Run as Administrator**
- Right-click the `backend-rust-orchestrator.exe` binary
- Select "Run as administrator"

**Option B: Install as Windows Service**
- Install the Orchestrator as a Windows Service that runs with Administrator privileges
- Use `sc.exe` or the `service-manager` crate to install the service

**Option C: Use Task Scheduler**
- Create a scheduled task that runs with highest privileges
- Configure it to run the Orchestrator binary

### Step 2: Set PowerShell Execution Policy

Run PowerShell as Administrator and execute:

```powershell
Set-ExecutionPolicy RemoteSigned -Scope CurrentUser
```

This allows the AI to execute diagnostic PowerShell scripts for log retrieval.

### Step 3: Install Services as Windows Services

Ensure the Telemetry and Orchestrator services are installed as Windows Services so they can be managed programmatically:

```powershell
# Install Telemetry Service
sc.exe create TelemetryService binPath="C:\path\to\telemetry.exe" start=auto
sc.exe description TelemetryService "Ferrellgas AGI Telemetry Service"

# Install Orchestrator Service
sc.exe create OrchestratorService binPath="C:\path\to\backend-rust-orchestrator.exe" start=auto
sc.exe description OrchestratorService "Ferrellgas AGI Orchestrator Service"
```

### Step 4: Verify Service Management

Test that services can be managed:

```powershell
# Check service status
sc.exe query TelemetryService

# Start service
sc.exe start TelemetryService

# Stop service
sc.exe stop TelemetryService
```

### Step 5: Verify Log Access

Test PowerShell log retrieval:

```powershell
Get-EventLog -LogName System -Source TelemetryService -Newest 50
```

**Note**: Services must be registered as event sources to appear in Event Log. For services that don't log to Event Log, the Orchestrator may need to read log files directly from the service's log directory.

## 🍎 macOS (Launchd) Setup

### Step 1: Create Sudoers Configuration

Create `/etc/sudoers.d/agi-orchestrator` with launchctl access:

```bash
sudo tee /etc/sudoers.d/agi-orchestrator > /dev/null <<EOF
# Ferrellgas AGI Orchestrator - macOS Unlimited Access Configuration
# This grants passwordless sudo access to launchctl and log commands
# for cross-platform service management (P54.V2)

agi-orchestrator ALL=(ALL) NOPASSWD: /bin/launchctl *
agi-orchestrator ALL=(ALL) NOPASSWD: /usr/bin/log *
EOF
```

### Step 2: Set Proper Permissions

```bash
sudo chmod 0440 /etc/sudoers.d/agi-orchestrator
```

### Step 3: Validate Configuration

```bash
sudo visudo -c -f /etc/sudoers.d/agi-orchestrator
```

### Step 4: Grant Full Disk Access

The Orchestrator needs Full Disk Access to read logs and storage files:

1. Open **System Settings** (or **System Preferences** on older macOS)
2. Navigate to **Privacy & Security** > **Full Disk Access**
3. Click the **+** button to add an application
4. Navigate to and select the `backend-rust-orchestrator` binary
5. Ensure the checkbox is enabled for the Orchestrator

**Alternative via Terminal** (requires admin):

```bash
# Add the Orchestrator binary to Full Disk Access
sudo tccutil reset SystemPolicyAllFiles
# Then manually add via System Settings as described above
```

### Step 5: Verify Access

Test that the orchestrator user can execute launchctl commands:

```bash
sudo -u agi-orchestrator sudo -n launchctl print system/com.ferrellgas.telemetry
sudo -u agi-orchestrator sudo -n log show --predicate 'process == "telemetry"' --last 5m
```

## Cross-Platform Service Management

Once configured, the Orchestrator's `manage_service` function will automatically use the correct commands for each OS:

### Linux
- **Start/Stop/Restart**: `sudo systemctl <action> <service>`
- **Status**: `sudo systemctl status <service>`
- **Enable/Disable**: `sudo systemctl enable/disable <service>`

### Windows
- **Start**: `sc.exe start <service>`
- **Stop**: `sc.exe stop <service>`
- **Restart**: `sc.exe stop <service>` followed by `sc.exe start <service>`
- **Status**: `sc.exe query <service>`
- **Enable/Disable**: `sc.exe config <service> start= auto/disabled`

### macOS
- **Start/Restart**: `sudo launchctl kickstart -k system/<service>`
- **Stop**: `sudo launchctl bootout system/<service>`
- **Status**: `sudo launchctl print system/<service>`
- **Enable**: `sudo launchctl bootstrap system /Library/LaunchDaemons/<service>.plist`
- **Disable**: `sudo launchctl bootout system/<service>`

## Cross-Platform Log Retrieval

The `get_logs` function uses platform-specific commands:

### Linux
```bash
sudo journalctl -u <service> -n 50
```

### Windows
```powershell
Get-EventLog -LogName System -Source <service> -Newest 50
```

### macOS
```bash
sudo log show --predicate 'process == "<service>"' --last 5m
```

## Security Considerations

### Principle of Least Privilege

While "Unlimited Access" is convenient for development and research nodes, consider using restricted access in production:

- **Linux**: Use specific service names instead of wildcards
- **Windows**: Run services with minimal required privileges
- **macOS**: Limit launchctl access to specific service domains

### Audit and Monitoring

Monitor service management activities:

- **Linux**: Check `/var/log/auth.log` for sudo usage
- **Windows**: Review Event Viewer > Windows Logs > Security
- **macOS**: Review Console.app or `log show --predicate 'eventMessage contains "sudo"'`

## Troubleshooting

### Linux: Permission Denied

**Symptom**: `sudo: a password is required`

**Solutions**:
1. Verify sudoers file exists: `ls -l /etc/sudoers.d/agi-orchestrator`
2. Check file permissions: `sudo chmod 0440 /etc/sudoers.d/agi-orchestrator`
3. Validate syntax: `sudo visudo -c -f /etc/sudoers.d/agi-orchestrator`
4. Test access: `sudo -u agi-orchestrator sudo -l`

### Windows: Access Denied

**Symptom**: `Access is denied` when running `sc.exe` commands

**Solutions**:
1. Ensure PowerShell/Command Prompt is running as Administrator
2. Verify the Orchestrator process has Administrator privileges
3. Check service exists: `sc.exe query <service>`
4. Verify service name is correct (case-sensitive)

### macOS: Operation Not Permitted

**Symptom**: `Operation not permitted` when accessing logs

**Solutions**:
1. Verify Full Disk Access is granted in System Settings
2. Check sudoers configuration: `sudo visudo -c -f /etc/sudoers.d/agi-orchestrator`
3. Ensure the orchestrator binary is in the Full Disk Access list
4. Restart the Orchestrator service after granting permissions

## Related Documentation

- [SOVEREIGN_PERMISSIONS.md](./SOVEREIGN_PERMISSIONS.md) - Detailed permission configuration
- [README_SOVEREIGN_SETUP.md](./README_SOVEREIGN_SETUP.md) - Setup guide with scripts
- [System Tools Implementation](./src/tools/system.rs) - Cross-platform service management code

## Support

For issues or questions:
1. Check the troubleshooting section above
2. Review service logs for the Orchestrator
3. Verify permissions using the verification commands in each OS section
4. Check the system tools implementation in `src/tools/system.rs`
