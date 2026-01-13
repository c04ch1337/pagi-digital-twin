# Ferrellgas AGI - Windows Unlimited Access Setup Script
# This script configures Windows for Unlimited Access (P54.V2) to enable cross-platform
# service management across research nodes.
#
# Usage: .\setup_windows_unlimited_access.ps1 [ORCHESTRATOR_BINARY_PATH] [TELEMETRY_BINARY_PATH]
#
# Prerequisites:
# - Must be run as Administrator
# - Windows 10/11 or Windows Server
# - Orchestrator and Telemetry binaries must be built (target/release or target/debug)

param(
    [string]$OrchestratorBinaryPath = "",
    [string]$TelemetryBinaryPath = "",
    [string]$TelemetryStorageDir = ".\storage"
)

# Check if running as Administrator
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)

if (-not $isAdmin) {
    Write-Host "❌ Error: This script must be run as Administrator" -ForegroundColor Red
    Write-Host "   Right-click PowerShell and select 'Run as Administrator'" -ForegroundColor Yellow
    exit 1
}

Write-Host "🚀 Setting up Windows Unlimited Access for AGI Orchestrator" -ForegroundColor Green
Write-Host "===========================================================" -ForegroundColor Green
Write-Host ""
Write-Host "This script will configure:" -ForegroundColor Cyan
Write-Host "  ✓ PowerShell Execution Policy (RemoteSigned)" -ForegroundColor White
Write-Host "  ✓ Windows Services for Orchestrator and Telemetry" -ForegroundColor White
Write-Host "  ✓ Service management capabilities" -ForegroundColor White
Write-Host "  ✓ Log access permissions" -ForegroundColor White
Write-Host ""

# Step 1: Set PowerShell Execution Policy
Write-Host "📝 Step 1: Setting PowerShell Execution Policy" -ForegroundColor Yellow
try {
    $currentPolicy = Get-ExecutionPolicy -Scope CurrentUser
    Write-Host "   Current execution policy: $currentPolicy" -ForegroundColor Gray
    
    if ($currentPolicy -ne "RemoteSigned" -and $currentPolicy -ne "Unrestricted" -and $currentPolicy -ne "Bypass") {
        Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser -Force
        Write-Host "   ✅ Set execution policy to RemoteSigned for CurrentUser" -ForegroundColor Green
    } else {
        Write-Host "   ✅ Execution policy already allows script execution ($currentPolicy)" -ForegroundColor Green
    }
} catch {
    Write-Host "   ⚠️  Warning: Could not set execution policy: $_" -ForegroundColor Yellow
    Write-Host "   You may need to set it manually: Set-ExecutionPolicy RemoteSigned -Scope CurrentUser" -ForegroundColor Yellow
}

Write-Host ""

# Step 2: Find Orchestrator Binary
Write-Host "📝 Step 2: Locating Orchestrator Binary" -ForegroundColor Yellow

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$projectRoot = Split-Path -Parent $scriptRoot

if ([string]::IsNullOrEmpty($OrchestratorBinaryPath)) {
    # Try to find the binary in common locations
    $possiblePaths = @(
        "$projectRoot\backend-rust-orchestrator\target\release\backend-rust-orchestrator.exe",
        "$projectRoot\backend-rust-orchestrator\target\debug\backend-rust-orchestrator.exe",
        "$projectRoot\backend-rust-orchestrator\target\release\backend_rust_orchestrator.exe",
        "$projectRoot\backend-rust-orchestrator\target\debug\backend_rust_orchestrator.exe"
    )
    
    $OrchestratorBinaryPath = $null
    foreach ($path in $possiblePaths) {
        if (Test-Path $path) {
            $OrchestratorBinaryPath = $path
            break
        }
    }
}

if ([string]::IsNullOrEmpty($OrchestratorBinaryPath) -or -not (Test-Path $OrchestratorBinaryPath)) {
    Write-Host "   ⚠️  Warning: Orchestrator binary not found" -ForegroundColor Yellow
    Write-Host "   Please build the orchestrator first: cd backend-rust-orchestrator; cargo build --release" -ForegroundColor Yellow
    Write-Host "   Or provide the path manually: -OrchestratorBinaryPath 'C:\path\to\backend-rust-orchestrator.exe'" -ForegroundColor Yellow
    $installOrchestrator = $false
} else {
    $OrchestratorBinaryPath = Resolve-Path $OrchestratorBinaryPath
    Write-Host "   ✅ Found Orchestrator binary: $OrchestratorBinaryPath" -ForegroundColor Green
    $installOrchestrator = $true
}

Write-Host ""

# Step 3: Find Telemetry Binary
Write-Host "📝 Step 3: Locating Telemetry Binary" -ForegroundColor Yellow

if ([string]::IsNullOrEmpty($TelemetryBinaryPath)) {
    # Try to find the binary in common locations
    $possiblePaths = @(
        "$projectRoot\backend-rust-telemetry\target\release\backend-rust-telemetry.exe",
        "$projectRoot\backend-rust-telemetry\target\debug\backend-rust-telemetry.exe",
        "$projectRoot\backend-rust-telemetry\target\release\backend_rust_telemetry.exe",
        "$projectRoot\backend-rust-telemetry\target\debug\backend_rust_telemetry.exe"
    )
    
    $TelemetryBinaryPath = $null
    foreach ($path in $possiblePaths) {
        if (Test-Path $path) {
            $TelemetryBinaryPath = $path
            break
        }
    }
}

if ([string]::IsNullOrEmpty($TelemetryBinaryPath) -or -not (Test-Path $TelemetryBinaryPath)) {
    Write-Host "   ⚠️  Warning: Telemetry binary not found" -ForegroundColor Yellow
    Write-Host "   Please build the telemetry service first: cd backend-rust-telemetry; cargo build --release" -ForegroundColor Yellow
    Write-Host "   Or provide the path manually: -TelemetryBinaryPath 'C:\path\to\backend-rust-telemetry.exe'" -ForegroundColor Yellow
    $installTelemetry = $false
} else {
    $TelemetryBinaryPath = Resolve-Path $TelemetryBinaryPath
    Write-Host "   ✅ Found Telemetry binary: $TelemetryBinaryPath" -ForegroundColor Green
    $installTelemetry = $true
}

Write-Host ""

# Step 4: Install Orchestrator as Windows Service
if ($installOrchestrator) {
    Write-Host "📝 Step 4: Installing Orchestrator as Windows Service" -ForegroundColor Yellow
    
    $serviceName = "AGIOrchestrator"
    $serviceDisplayName = "Ferrellgas AGI Orchestrator Service"
    $serviceDescription = "Ferrellgas AGI Orchestrator - Cross-platform service management and orchestration"
    
    # Check if service already exists
    $existingService = Get-Service -Name $serviceName -ErrorAction SilentlyContinue
    
    if ($existingService) {
        Write-Host "   Service '$serviceName' already exists" -ForegroundColor Gray
        
        # Stop and remove existing service
        if ($existingService.Status -eq "Running") {
            Write-Host "   Stopping existing service..." -ForegroundColor Gray
            Stop-Service -Name $serviceName -Force -ErrorAction SilentlyContinue
            Start-Sleep -Seconds 2
        }
        
        Write-Host "   Removing existing service..." -ForegroundColor Gray
        sc.exe delete $serviceName | Out-Null
        Start-Sleep -Seconds 2
    }
    
    # Install the service
    try {
        $binPath = "`"$OrchestratorBinaryPath`""
        $result = sc.exe create $serviceName binPath= $binPath start= demand DisplayName= $serviceDisplayName
        
        if ($LASTEXITCODE -eq 0) {
            Write-Host "   ✅ Service '$serviceName' created successfully" -ForegroundColor Green
            
            # Set service description
            sc.exe description $serviceName $serviceDescription | Out-Null
            
            # Configure service to run with highest privileges (LocalSystem account)
            # Note: For production, consider using a dedicated service account
            sc.exe config $serviceName obj= "LocalSystem" | Out-Null
            
            Write-Host "   ✅ Service configured to run with LocalSystem privileges" -ForegroundColor Green
        } else {
            Write-Host "   ❌ Failed to create service. Exit code: $LASTEXITCODE" -ForegroundColor Red
            Write-Host "   Output: $result" -ForegroundColor Red
        }
    } catch {
        Write-Host "   ❌ Error installing service: $_" -ForegroundColor Red
    }
} else {
    Write-Host "📝 Step 4: Skipping Orchestrator service installation (binary not found)" -ForegroundColor Yellow
}

Write-Host ""

# Step 5: Install Telemetry as Windows Service
if ($installTelemetry) {
    Write-Host "📝 Step 5: Installing Telemetry as Windows Service" -ForegroundColor Yellow
    
    $serviceName = "AGITelemetry"
    $serviceDisplayName = "Ferrellgas AGI Telemetry Service"
    $serviceDescription = "Ferrellgas AGI Telemetry Service - Multi-modal telemetry and recording"
    
    # Check if service already exists
    $existingService = Get-Service -Name $serviceName -ErrorAction SilentlyContinue
    
    if ($existingService) {
        Write-Host "   Service '$serviceName' already exists" -ForegroundColor Gray
        
        # Stop and remove existing service
        if ($existingService.Status -eq "Running") {
            Write-Host "   Stopping existing service..." -ForegroundColor Gray
            Stop-Service -Name $serviceName -Force -ErrorAction SilentlyContinue
            Start-Sleep -Seconds 2
        }
        
        Write-Host "   Removing existing service..." -ForegroundColor Gray
        sc.exe delete $serviceName | Out-Null
        Start-Sleep -Seconds 2
    }
    
    # Install the service
    try {
        $binPath = "`"$TelemetryBinaryPath`""
        $result = sc.exe create $serviceName binPath= $binPath start= demand DisplayName= $serviceDisplayName
        
        if ($LASTEXITCODE -eq 0) {
            Write-Host "   ✅ Service '$serviceName' created successfully" -ForegroundColor Green
            
            # Set service description
            sc.exe description $serviceName $serviceDescription | Out-Null
            
            # Configure service to run with highest privileges (LocalSystem account)
            sc.exe config $serviceName obj= "LocalSystem" | Out-Null
            
            Write-Host "   ✅ Service configured to run with LocalSystem privileges" -ForegroundColor Green
        } else {
            Write-Host "   ❌ Failed to create service. Exit code: $LASTEXITCODE" -ForegroundColor Red
            Write-Host "   Output: $result" -ForegroundColor Red
        }
    } catch {
        Write-Host "   ❌ Error installing service: $_" -ForegroundColor Red
    }
} else {
    Write-Host "📝 Step 5: Skipping Telemetry service installation (binary not found)" -ForegroundColor Yellow
}

Write-Host ""

# Step 6: Verify Service Management
Write-Host "📝 Step 6: Verifying Service Management Capabilities" -ForegroundColor Yellow

$servicesToTest = @()
if ($installOrchestrator) { $servicesToTest += "AGIOrchestrator" }
if ($installTelemetry) { $servicesToTest += "AGITelemetry" }

foreach ($serviceName in $servicesToTest) {
    $service = Get-Service -Name $serviceName -ErrorAction SilentlyContinue
    if ($service) {
        Write-Host "   ✅ Service '$serviceName' is installed" -ForegroundColor Green
        
        # Test service query
        $queryResult = sc.exe query $serviceName 2>&1
        if ($LASTEXITCODE -eq 0) {
            Write-Host "   ✅ Service query successful" -ForegroundColor Green
        } else {
            Write-Host "   ⚠️  Service query returned exit code: $LASTEXITCODE" -ForegroundColor Yellow
        }
    } else {
        Write-Host "   ❌ Service '$serviceName' not found" -ForegroundColor Red
    }
}

Write-Host ""

# Step 7: Verify Log Access
Write-Host "📝 Step 7: Verifying Log Access" -ForegroundColor Yellow

try {
    # Test PowerShell Get-EventLog (this is what the Orchestrator uses)
    $testLog = Get-EventLog -LogName System -Newest 1 -ErrorAction SilentlyContinue
    if ($testLog) {
        Write-Host "   ✅ PowerShell Get-EventLog access verified" -ForegroundColor Green
    } else {
        Write-Host "   ⚠️  Could not retrieve event log entries (this may be normal if no events exist)" -ForegroundColor Yellow
    }
} catch {
    Write-Host "   ⚠️  Warning: Event log access test failed: $_" -ForegroundColor Yellow
    Write-Host "   This may require additional permissions" -ForegroundColor Yellow
}

Write-Host ""

# Step 8: Summary and Next Steps
Write-Host "✅ Windows Unlimited Access Setup Complete!" -ForegroundColor Green
Write-Host ""
Write-Host "Summary:" -ForegroundColor Cyan
Write-Host "  - PowerShell Execution Policy: Configured" -ForegroundColor White
if ($installOrchestrator) {
    Write-Host "  - Orchestrator Service: Installed as 'AGIOrchestrator'" -ForegroundColor White
}
if ($installTelemetry) {
    Write-Host "  - Telemetry Service: Installed as 'AGITelemetry'" -ForegroundColor White
}
Write-Host ""

Write-Host "Service Management Commands:" -ForegroundColor Cyan
if ($installOrchestrator) {
    Write-Host "  # Start Orchestrator: sc.exe start AGIOrchestrator" -ForegroundColor White
    Write-Host "  # Stop Orchestrator:  sc.exe stop AGIOrchestrator" -ForegroundColor White
    Write-Host "  # Status:             sc.exe query AGIOrchestrator" -ForegroundColor White
}
if ($installTelemetry) {
    Write-Host "  # Start Telemetry:   sc.exe start AGITelemetry" -ForegroundColor White
    Write-Host "  # Stop Telemetry:    sc.exe stop AGITelemetry" -ForegroundColor White
    Write-Host "  # Status:            sc.exe query AGITelemetry" -ForegroundColor White
}
Write-Host ""

Write-Host "Important Notes:" -ForegroundColor Cyan
Write-Host "  1. Services are installed but NOT started automatically" -ForegroundColor Yellow
Write-Host "  2. Services run with LocalSystem privileges (full Administrator access)" -ForegroundColor Yellow
Write-Host "  3. For production, consider using a dedicated service account" -ForegroundColor Yellow
Write-Host "  4. The Orchestrator can now manage services using manage_service() function" -ForegroundColor Green
Write-Host "  5. Log access is available via get_logs() function" -ForegroundColor Green
Write-Host ""

Write-Host "To start services manually:" -ForegroundColor Cyan
if ($installOrchestrator) {
    Write-Host "  sc.exe start AGIOrchestrator" -ForegroundColor White
}
if ($installTelemetry) {
    Write-Host "  sc.exe start AGITelemetry" -ForegroundColor White
}
Write-Host ""

Write-Host "To verify setup, test service management:" -ForegroundColor Cyan
Write-Host "  sc.exe query AGIOrchestrator" -ForegroundColor White
Write-Host "  Get-EventLog -LogName System -Source AGIOrchestrator -Newest 10" -ForegroundColor White
Write-Host ""
