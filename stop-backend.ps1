# Stop Backend Services
# Run from root folder: .\stop-backend.ps1

Write-Host "Stopping PAGI Digital Twin Backend Services..." -ForegroundColor Cyan
Write-Host ""

# Find and stop cargo processes
$cargoProcesses = Get-Process | Where-Object { 
    $_.ProcessName -eq "cargo" -or 
    ($_.CommandLine -like "*backend-rust*" -and $_.ProcessName -eq "rustc")
}

if ($cargoProcesses) {
    Write-Host "Found $($cargoProcesses.Count) Rust service process(es)" -ForegroundColor Yellow
    
    foreach ($proc in $cargoProcesses) {
        try {
            Write-Host "Stopping process: $($proc.ProcessName) (PID: $($proc.Id))" -ForegroundColor Yellow
            Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
        } catch {
            Write-Host "Could not stop process $($proc.Id): $_" -ForegroundColor Red
        }
    }
    
    # Also stop any child processes (the actual service binaries)
    Start-Sleep -Seconds 1
    
    $serviceProcesses = Get-Process | Where-Object {
        $_.Path -like "*backend-rust-*" -or
        $_.CommandLine -like "*backend-rust-*"
    }
    
    if ($serviceProcesses) {
        foreach ($proc in $serviceProcesses) {
            try {
                Write-Host "Stopping service: $($proc.ProcessName) (PID: $($proc.Id))" -ForegroundColor Yellow
                Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
            } catch {
                # Ignore errors
            }
        }
    }
    
    Write-Host ""
    Write-Host "Services stopped." -ForegroundColor Green
} else {
    Write-Host "No running backend services found." -ForegroundColor Yellow
}

# Also try to close PowerShell windows that might be running services
Write-Host ""
Write-Host "Note: You may need to manually close PowerShell windows that were opened for services." -ForegroundColor Cyan
