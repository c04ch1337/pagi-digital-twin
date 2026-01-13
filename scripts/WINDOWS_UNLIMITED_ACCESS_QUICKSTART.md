# Windows Unlimited Access - Quick Start Guide

This guide will help you set up Windows Unlimited Access for the AGI Orchestrator (P54.V2).

## Prerequisites

1. **Administrator Privileges**: You must run PowerShell as Administrator
2. **Built Binaries**: Ensure the orchestrator and telemetry services are built:
   ```powershell
   cd backend-rust-orchestrator
   cargo build --release
   
   cd ..\backend-rust-telemetry
   cargo build --release
   ```

## Quick Setup

### Step 1: Run the Setup Script

1. **Right-click** on PowerShell and select **"Run as Administrator"**
2. Navigate to the project root:
   ```powershell
   cd C:\Users\JAMEYMILNER\AppData\Local\pagi-digital-twin
   ```
3. Run the setup script:
   ```powershell
   .\scripts\setup_windows_unlimited_access.ps1
   ```

The script will:
- ✅ Set PowerShell execution policy to `RemoteSigned`
- ✅ Locate orchestrator and telemetry binaries
- ✅ Install services as Windows Services (if binaries found)
- ✅ Configure services to run with LocalSystem privileges
- ✅ Verify service management capabilities

### Step 2: Verify Setup

After running the script, verify the setup:

```powershell
# Check if services are installed
Get-Service -Name AGIOrchestrator -ErrorAction SilentlyContinue
Get-Service -Name AGITelemetry -ErrorAction SilentlyContinue

# Test service management
sc.exe query AGIOrchestrator
sc.exe query AGITelemetry

# Test log access
Get-EventLog -LogName System -Newest 10
```

## Manual Setup (If Script Fails)

If the automated script doesn't work, you can set up manually:

### 1. Set PowerShell Execution Policy

```powershell
Set-ExecutionPolicy RemoteSigned -Scope CurrentUser
```

### 2. Install Orchestrator Service

```powershell
$orchestratorPath = "C:\Users\JAMEYMILNER\AppData\Local\pagi-digital-twin\backend-rust-orchestrator\target\release\backend-rust-orchestrator.exe"
sc.exe create AGIOrchestrator binPath= "`"$orchestratorPath`"" start= demand DisplayName= "Ferrellgas AGI Orchestrator Service"
sc.exe description AGIOrchestrator "Ferrellgas AGI Orchestrator - Cross-platform service management and orchestration"
sc.exe config AGIOrchestrator obj= "LocalSystem"
```

### 3. Install Telemetry Service

```powershell
$telemetryPath = "C:\Users\JAMEYMILNER\AppData\Local\pagi-digital-twin\backend-rust-telemetry\target\release\backend-rust-telemetry.exe"
sc.exe create AGITelemetry binPath= "`"$telemetryPath`"" start= demand DisplayName= "Ferrellgas AGI Telemetry Service"
sc.exe description AGITelemetry "Ferrellgas AGI Telemetry Service - Multi-modal telemetry and recording"
sc.exe config AGITelemetry obj= "LocalSystem"
```

## Service Management

Once services are installed, you can manage them:

### Start Services
```powershell
sc.exe start AGIOrchestrator
sc.exe start AGITelemetry
```

### Stop Services
```powershell
sc.exe stop AGIOrchestrator
sc.exe stop AGITelemetry
```

### Check Status
```powershell
sc.exe query AGIOrchestrator
sc.exe query AGITelemetry
```

### Restart Services
```powershell
sc.exe stop AGIOrchestrator
Start-Sleep -Seconds 2
sc.exe start AGIOrchestrator
```

## Log Access

The Orchestrator can now retrieve logs using PowerShell:

```powershell
# Get recent system events
Get-EventLog -LogName System -Newest 50

# Get events for a specific source (if registered)
Get-EventLog -LogName System -Source AGIOrchestrator -Newest 50
```

**Note**: Services must be registered as event sources to appear in Event Log. For services that don't log to Event Log, the Orchestrator may need to read log files directly.

## What This Enables

With Unlimited Access configured, the Orchestrator can now:

1. **Manage Services**: Use `manage_service()` function to start, stop, restart, enable, or disable services
2. **Retrieve Logs**: Use `get_logs()` function to access service logs via PowerShell
3. **Cross-Platform**: The same code works across Linux, Windows, and macOS research nodes

## Troubleshooting

### "Access is denied" when running sc.exe

**Solution**: Ensure PowerShell is running as Administrator

### "Service not found" when querying services

**Solution**: Verify services are installed:
```powershell
Get-Service | Where-Object { $_.Name -like "*AGI*" }
```

### "Execution policy" errors

**Solution**: Set execution policy:
```powershell
Set-ExecutionPolicy RemoteSigned -Scope CurrentUser
```

### Services won't start

**Solution**: Check service logs:
```powershell
Get-EventLog -LogName System -Source AGIOrchestrator -Newest 20
```

## Next Steps

1. Start the services (if not already running)
2. Test the Orchestrator's `manage_service()` function
3. Test the Orchestrator's `get_logs()` function
4. Verify cross-platform compatibility when moving between research nodes

## Related Documentation

- [UNLIMITED_ACCESS_SETUP.md](../backend-rust-orchestrator/UNLIMITED_ACCESS_SETUP.md) - Full documentation
- [System Tools Implementation](../backend-rust-orchestrator/src/tools/system.rs) - Code reference
