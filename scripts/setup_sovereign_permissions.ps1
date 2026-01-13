# Ferrellgas AGI - Sovereign Permissions Setup Script (PowerShell)
# This script configures the Windows environment for the Orchestrator service.
# Note: Windows uses a different permission model than Linux.
#
# Usage: .\setup_sovereign_permissions.ps1 [TELEMETRY_STORAGE_DIR]
#
# Prerequisites:
# - Must be run as Administrator
# - Windows 10/11 or Windows Server

param(
    [string]$TelemetryStorageDir = ".\storage"
)

# Check if running as Administrator
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)

if (-not $isAdmin) {
    Write-Host "❌ Error: This script must be run as Administrator" -ForegroundColor Red
    Write-Host "   Right-click PowerShell and select 'Run as Administrator'" -ForegroundColor Yellow
    exit 1
}

Write-Host "🚀 Setting up Sovereign Permissions for AGI Orchestrator" -ForegroundColor Green
Write-Host "==================================================" -ForegroundColor Green
Write-Host ""

$OrchestratorUser = "agi-orchestrator"

# Step 1: Create dedicated user (if it doesn't exist)
Write-Host "📝 Step 1: Creating system user '$OrchestratorUser'" -ForegroundColor Yellow

try {
    $user = Get-LocalUser -Name $OrchestratorUser -ErrorAction SilentlyContinue
    if ($user) {
        Write-Host "   User '$OrchestratorUser' already exists, skipping creation" -ForegroundColor Gray
    } else {
        $password = Read-Host "Enter password for $OrchestratorUser" -AsSecureString
        New-LocalUser -Name $OrchestratorUser -Password $password -Description "AGI Orchestrator Service Account" -UserMayNotChangePassword
        Write-Host "   ✅ User '$OrchestratorUser' created successfully" -ForegroundColor Green
    }
} catch {
    Write-Host "   ❌ Error creating user: $_" -ForegroundColor Red
    exit 1
}

# Step 2: Configure storage directory permissions
Write-Host "📝 Step 2: Configuring storage directory permissions" -ForegroundColor Yellow

$absStorageDir = Resolve-Path -Path $TelemetryStorageDir -ErrorAction SilentlyContinue
if (-not $absStorageDir) {
    # Create directory if it doesn't exist
    New-Item -ItemType Directory -Path $TelemetryStorageDir -Force | Out-Null
    $absStorageDir = Resolve-Path -Path $TelemetryStorageDir
    Write-Host "   ✅ Created storage directory at $absStorageDir" -ForegroundColor Green
} else {
    Write-Host "   Storage directory exists at $absStorageDir" -ForegroundColor Gray
}

# Set permissions using icacls
try {
    # Grant full control to the orchestrator user
    icacls $absStorageDir /grant "${OrchestratorUser}:(OI)(CI)F" /T | Out-Null
    Write-Host "   ✅ Set permissions on $absStorageDir for $OrchestratorUser" -ForegroundColor Green
} catch {
    Write-Host "   ⚠️  Warning: Could not set permissions using icacls: $_" -ForegroundColor Yellow
    Write-Host "   You may need to set permissions manually through File Explorer" -ForegroundColor Yellow
}

# Create recordings subdirectory
$recordingsDir = Join-Path $absStorageDir "recordings"
if (-not (Test-Path $recordingsDir)) {
    New-Item -ItemType Directory -Path $recordingsDir -Force | Out-Null
    icacls $recordingsDir /grant "${OrchestratorUser}:(OI)(CI)F" /T | Out-Null
    Write-Host "   ✅ Created recordings directory at $recordingsDir" -ForegroundColor Green
}

Write-Host ""
Write-Host "✅ Sovereign Permissions Setup Complete!" -ForegroundColor Green
Write-Host ""
Write-Host "Summary:"
Write-Host "  - User created: $OrchestratorUser"
Write-Host "  - Storage directory: $absStorageDir"
Write-Host ""
Write-Host "Note: Windows does not use sudo/sudoers. Service management is handled through:"
Write-Host "  - Windows Services (services.msc)"
Write-Host "  - sc.exe command-line tool"
Write-Host "  - PowerShell cmdlets (Get-Service, Start-Service, etc.)"
Write-Host ""
Write-Host "Next steps:"
Write-Host "  1. Configure services to run as $OrchestratorUser"
Write-Host "  2. Set TELEMETRY_STORAGE_DIR=$absStorageDir in your environment"
Write-Host ""
