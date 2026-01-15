# Phoenix Fleet Manager - Node Registration Script (PowerShell)
# 
# This script allows a new bare-metal machine to register itself with
# a primary Phoenix Orchestrator instance.
#
# Usage: .\phoenix-join.ps1 -Gateway <IP_ADDRESS> [-Port <PORT>] [-NodeId <NODE_ID>]

param(
    [Parameter(Mandatory=$true)]
    [string]$Gateway,
    
    [Parameter(Mandatory=$false)]
    [int]$Port = 3000,
    
    [Parameter(Mandatory=$false)]
    [string]$NodeId = ""
)

# Generate node ID if not provided
if ([string]::IsNullOrEmpty($NodeId)) {
    $NodeId = "node-$(New-Guid)"
}

# Get hostname
$Hostname = $env:COMPUTERNAME

# Get local IP address (prefer non-loopback interfaces)
$IPAddress = (Get-NetIPAddress -AddressFamily IPv4 -InterfaceAlias "Ethernet*","Wi-Fi*" -ErrorAction SilentlyContinue | 
    Where-Object { $_.IPAddress -notlike "169.254.*" -and $_.IPAddress -notlike "127.*" } | 
    Select-Object -First 1).IPAddress

if ([string]::IsNullOrEmpty($IPAddress)) {
    $IPAddress = "127.0.0.1"
}

Write-Host "Phoenix Fleet Manager - Node Registration" -ForegroundColor Cyan
Write-Host "==========================================" -ForegroundColor Cyan
Write-Host "Gateway: http://${Gateway}:${Port}"
Write-Host "Node ID: ${NodeId}"
Write-Host "Hostname: ${Hostname}"
Write-Host "IP Address: ${IPAddress}"
Write-Host ""

# Send heartbeat to gateway
$HeartbeatUrl = "http://${Gateway}:${Port}/api/fleet/heartbeat"

$Body = @{
    node_id = $NodeId
    hostname = $Hostname
    ip_address = $IPAddress
    software_version = "2.1.0"
} | ConvertTo-Json

Write-Host "Sending heartbeat to ${HeartbeatUrl}..." -ForegroundColor Yellow

try {
    $Response = Invoke-RestMethod -Uri $HeartbeatUrl -Method Post -Body $Body -ContentType "application/json" -ErrorAction Stop
    
    Write-Host "✓ Successfully registered with Phoenix Fleet Manager" -ForegroundColor Green
    Write-Host ""
    Write-Host "Response:" -ForegroundColor Cyan
    $Response | ConvertTo-Json -Depth 10
    Write-Host ""
    Write-Host "To keep this node registered, you should set up a periodic heartbeat:" -ForegroundColor Yellow
    Write-Host "  Create a scheduled task that runs every 30 minutes:" -ForegroundColor Yellow
    Write-Host "  `$Body = @{ node_id = '${NodeId}'; hostname = '${Hostname}'; ip_address = '${IPAddress}'; software_version = '2.1.0' } | ConvertTo-Json" -ForegroundColor Gray
    Write-Host "  Invoke-RestMethod -Uri '${HeartbeatUrl}' -Method Post -Body `$Body -ContentType 'application/json'" -ForegroundColor Gray
    Write-Host ""
    Write-Host "Or run this script periodically using Task Scheduler." -ForegroundColor Yellow
} catch {
    Write-Host "✗ Failed to register" -ForegroundColor Red
    Write-Host "Error: $($_.Exception.Message)" -ForegroundColor Red
    if ($_.Exception.Response) {
        $Reader = New-Object System.IO.StreamReader($_.Exception.Response.GetResponseStream())
        $ResponseBody = $Reader.ReadToEnd()
        Write-Host "Response: $ResponseBody" -ForegroundColor Red
    }
    exit 1
}
