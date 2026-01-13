# Start Backend Services (Bare Metal)
# Run from root folder: .\start-backend.ps1

Write-Host "Starting PAGI Digital Twin Backend Services..." -ForegroundColor Cyan
Write-Host ""

# Load .env if it exists
if (Test-Path .env) {
    Get-Content .env | ForEach-Object {
        if ($_ -match '^([^#][^=]+)=(.*)$') {
            $key = $matches[1].Trim()
            $value = $matches[2].Trim()
            [Environment]::SetEnvironmentVariable($key, $value, "Process")
        }
    }
    Write-Host "Loaded .env file" -ForegroundColor Green
} else {
    Write-Host "No .env file found, using defaults" -ForegroundColor Yellow
    # Set defaults
    $env:LLM_PROVIDER = "mock"
    $env:GATEWAY_PORT = "8181"
    $env:ORCHESTRATOR_HTTP_PORT = "8182"
    $env:TELEMETRY_PORT = "8183"
    $env:MEMORY_GRPC_PORT = "50052"
    $env:TOOLS_GRPC_PORT = "50054"
    $env:BUILD_SERVICE_PORT = "50055"
    $env:MEMORY_GRPC_ADDR = "http://127.0.0.1:50052"
    $env:TOOLS_GRPC_ADDR = "http://127.0.0.1:50054"
    $env:ORCHESTRATOR_URL = "http://127.0.0.1:8182"
    $env:TELEMETRY_URL = "http://127.0.0.1:8183"
    $env:LOG_LEVEL = "info"
}

# Service startup order (with dependencies)
$services = @(
    @{
        Name = "Memory Service"
        Dir = "backend-rust-memory"
        Port = $env:MEMORY_GRPC_PORT
        Delay = 0
    },
    @{
        Name = "Tools Service"
        Dir = "backend-rust-tools"
        Port = $env:TOOLS_GRPC_PORT
        Delay = 2
    },
    @{
        Name = "Telemetry Service"
        Dir = "backend-rust-telemetry"
        Port = $env:TELEMETRY_PORT
        Delay = 2
    },
    @{
        Name = "Orchestrator"
        Dir = "backend-rust-orchestrator"
        Port = $env:ORCHESTRATOR_HTTP_PORT
        Delay = 3
    },
    @{
        Name = "Gateway"
        Dir = "backend-rust-gateway"
        Port = $env:GATEWAY_PORT
        Delay = 3
    }
)

Write-Host "Starting services in order..." -ForegroundColor Cyan
Write-Host ""

foreach ($service in $services) {
    if ($service.Delay -gt 0) {
        Write-Host "Waiting $($service.Delay) seconds before starting $($service.Name)..." -ForegroundColor Yellow
        Start-Sleep -Seconds $service.Delay
    }
    
    Write-Host "[$($service.Name)] Starting on port $($service.Port)..." -ForegroundColor Green
    
    $serviceDir = Join-Path $PSScriptRoot $service.Dir
    if (-not (Test-Path $serviceDir)) {
        Write-Host "[$($service.Name)] ERROR: Directory not found: $serviceDir" -ForegroundColor Red
        continue
    }
    
    # Start service in new window
    Start-Process powershell -ArgumentList @(
        "-NoExit",
        "-Command",
        "cd '$serviceDir'; Write-Host '[$($service.Name)] Starting...' -ForegroundColor Cyan; cargo run"
    ) -WindowStyle Normal
    
    Write-Host "[$($service.Name)] Started in new window (PID: $((Get-Process powershell | Sort-Object StartTime -Descending | Select-Object -First 1).Id))" -ForegroundColor Green
}

Write-Host ""
Write-Host "All services started!" -ForegroundColor Green
Write-Host ""
Write-Host "Services:" -ForegroundColor Cyan
Write-Host "  - Memory Service:    http://localhost:$($env:MEMORY_GRPC_PORT)" -ForegroundColor White
Write-Host "  - Tools Service:      http://localhost:$($env:TOOLS_GRPC_PORT)" -ForegroundColor White
Write-Host "  - Telemetry Service:  http://localhost:$($env:TELEMETRY_PORT)" -ForegroundColor White
Write-Host "  - Orchestrator:       http://localhost:$($env:ORCHESTRATOR_HTTP_PORT)" -ForegroundColor White
Write-Host "  - Gateway:            http://localhost:$($env:GATEWAY_PORT)" -ForegroundColor White
Write-Host ""
Write-Host "Note: Services are compiling. This may take 1-2 minutes on first run." -ForegroundColor Yellow
Write-Host "Check the PowerShell windows for compilation progress." -ForegroundColor Yellow
Write-Host ""
Write-Host "To stop all services, close the PowerShell windows or run: .\stop-backend.ps1" -ForegroundColor Cyan
