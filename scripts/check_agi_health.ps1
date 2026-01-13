# Ferrellgas AGI Triple-Service Diagnostic Script (PowerShell)
# This script verifies the networking and gRPC reachability between
# Telemetry, Orchestrator, and Memory services on Windows.

param(
    [string]$OrchestratorGrpcAddr = "127.0.0.1:50057",
    [string]$OrchestratorHttpAddr = "127.0.0.1:8182",
    [string]$TelemetryHttpAddr = "127.0.0.1:8183",
    [string]$MemoryGrpcAddr = "127.0.0.1:50052",
    [string]$GatewayHttpAddr = "127.0.0.1:8181"
)

Write-Host "🚀 Starting Ferrellgas AGI Triple-Service Diagnostic..." -ForegroundColor Cyan
Write-Host "==================================================" -ForegroundColor Cyan
Write-Host ""

# Check if grpcurl is installed
if (-not (Get-Command grpcurl -ErrorAction SilentlyContinue)) {
    Write-Host "❌ ERROR: grpcurl is not installed." -ForegroundColor Red
    Write-Host "   Download from: https://github.com/fullstorydev/grpcurl/releases" -ForegroundColor Yellow
    Write-Host "   Or install via: scoop install grpcurl" -ForegroundColor Yellow
    exit 1
}

# 1. Check Service Liveness (HTTP Health Checks)
Write-Host "📡 Step 1: Checking Service Liveness (HTTP)" -ForegroundColor Yellow
Write-Host "--------------------------------------------" -ForegroundColor Yellow

Write-Host -NoNewline "  Checking Orchestrator HTTP ($OrchestratorHttpAddr)... "
try {
    $response = Invoke-WebRequest -Uri "http://$OrchestratorHttpAddr/health" -TimeoutSec 2 -ErrorAction Stop
    Write-Host "✅ ONLINE" -ForegroundColor Green
} catch {
    Write-Host "❌ OFFLINE" -ForegroundColor Red
    Write-Host "    → Check if Orchestrator is running on port 8182" -ForegroundColor Yellow
}

Write-Host -NoNewline "  Checking Telemetry HTTP ($TelemetryHttpAddr)... "
try {
    $response = Invoke-WebRequest -Uri "http://$TelemetryHttpAddr/v1/telemetry/stream" -TimeoutSec 2 -ErrorAction Stop
    Write-Host "✅ ONLINE" -ForegroundColor Green
} catch {
    Write-Host "❌ OFFLINE" -ForegroundColor Red
    Write-Host "    → Check if Telemetry is running on port 8183" -ForegroundColor Yellow
}

Write-Host -NoNewline "  Checking Gateway HTTP ($GatewayHttpAddr)... "
try {
    $response = Invoke-WebRequest -Uri "http://$GatewayHttpAddr/api/health" -TimeoutSec 2 -ErrorAction Stop
    Write-Host "✅ ONLINE" -ForegroundColor Green
} catch {
    Write-Host "❌ OFFLINE" -ForegroundColor Red
    Write-Host "    → Check if Gateway is running on port 8181" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "📡 Step 2: Checking gRPC Service Liveness" -ForegroundColor Yellow
Write-Host "--------------------------------------------" -ForegroundColor Yellow

Write-Host -NoNewline "  Checking Orchestrator gRPC ($OrchestratorGrpcAddr)... "
$grpcTest = & grpcurl -plaintext "$OrchestratorGrpcAddr" list 2>&1
if ($LASTEXITCODE -eq 0) {
    Write-Host "✅ ONLINE" -ForegroundColor Green
    Write-Host "    Available services:" -ForegroundColor Gray
    $grpcTest | ForEach-Object { Write-Host "      - $_" -ForegroundColor Gray }
} else {
    Write-Host "❌ OFFLINE" -ForegroundColor Red
    Write-Host "    → Check if Orchestrator gRPC is running on port 50057" -ForegroundColor Yellow
    Write-Host "    → Verify ORCHESTRATOR_GRPC_PORT environment variable" -ForegroundColor Yellow
}

Write-Host -NoNewline "  Checking Memory gRPC ($MemoryGrpcAddr)... "
$grpcTest = & grpcurl -plaintext "$MemoryGrpcAddr" list 2>&1
if ($LASTEXITCODE -eq 0) {
    Write-Host "✅ ONLINE" -ForegroundColor Green
    Write-Host "    Available services:" -ForegroundColor Gray
    $grpcTest | ForEach-Object { Write-Host "      - $_" -ForegroundColor Gray }
} else {
    Write-Host "❌ OFFLINE" -ForegroundColor Red
    Write-Host "    → Check if Memory service is running on port 50052" -ForegroundColor Yellow
    Write-Host "    → Verify MEMORY_GRPC_PORT environment variable" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "🔗 Step 3: Testing Telemetry -> Orchestrator Handshake (Summarization)" -ForegroundColor Yellow
Write-Host "------------------------------------------------------------------------" -ForegroundColor Yellow

# Test transcript summarization
$testTranscript = @{
    transcript_text = "This is a test of the automated insight system. We discussed several important topics and made key decisions."
} | ConvertTo-Json -Compress

Write-Host "  Sending test transcript to Orchestrator..." -ForegroundColor Gray
$summaryResponse = & grpcurl -plaintext -d $testTranscript "$OrchestratorGrpcAddr" orchestrator.OrchestratorService/SummarizeTranscript 2>&1

if ($summaryResponse -match "summary") {
    Write-Host "  ✅ SUCCESS: Orchestrator processed the transcript." -ForegroundColor Green
    Write-Host "  Response preview:" -ForegroundColor Gray
    $summaryResponse | Select-Object -First 5 | ForEach-Object { Write-Host "    $_" -ForegroundColor Gray }
} else {
    Write-Host "  ❌ FAILED: Telemetry cannot get a summary from Orchestrator." -ForegroundColor Red
    Write-Host "  Error details:" -ForegroundColor Yellow
    $summaryResponse | ForEach-Object { Write-Host "    $_" -ForegroundColor Red }
    Write-Host ""
    Write-Host "  Troubleshooting:" -ForegroundColor Yellow
    Write-Host "    → Check ORCHESTRATOR_GRPC_ADDR in Telemetry service" -ForegroundColor Yellow
    Write-Host "    → Verify LLM_PROVIDER=openrouter and OPENROUTER_API_KEY is set" -ForegroundColor Yellow
    Write-Host "    → Check Orchestrator logs for errors" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "🔗 Step 4: Testing Orchestrator -> Memory Handshake (Neural Archive)" -ForegroundColor Yellow
Write-Host "------------------------------------------------------------------------" -ForegroundColor Yellow

# Test memory commit
$testMemory = @{
    content = "Deployment test successful. This is a diagnostic entry."
    namespace = "system_logs"
    twin_id = "diagnostic-test"
    memory_type = "RAGSource"
    risk_level = "Low"
} | ConvertTo-Json -Compress

Write-Host "  Attempting to commit test entry to Memory service..." -ForegroundColor Gray
$memoryResponse = & grpcurl -plaintext -d $testMemory "$MemoryGrpcAddr" memory.MemoryService/CommitMemory 2>&1

if ($memoryResponse -match "success") {
    Write-Host "  ✅ SUCCESS: Orchestrator successfully committed to Neural Archive." -ForegroundColor Green
    if ($memoryResponse -match '"memory_id":"([^"]*)"') {
        $memoryId = $matches[1]
        Write-Host "  Memory ID: $memoryId" -ForegroundColor Gray
    }
} else {
    Write-Host "  ❌ FAILED: Orchestrator cannot write to Memory service." -ForegroundColor Red
    Write-Host "  Error details:" -ForegroundColor Yellow
    $memoryResponse | ForEach-Object { Write-Host "    $_" -ForegroundColor Red }
    Write-Host ""
    Write-Host "  Troubleshooting:" -ForegroundColor Yellow
    Write-Host "    → Check MEMORY_GRPC_ADDR in Orchestrator service" -ForegroundColor Yellow
    Write-Host "    → Verify Memory service is running and Qdrant is accessible" -ForegroundColor Yellow
    Write-Host "    → Check Memory service logs for errors" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "==================================================" -ForegroundColor Cyan
Write-Host "✅ Diagnostic Complete!" -ForegroundColor Green
Write-Host ""
Write-Host "If all tests passed, your AGI Closed-Loop Intelligence System is active." -ForegroundColor Cyan
Write-Host ""
Write-Host "Next Steps:" -ForegroundColor Yellow
Write-Host "  - Monitor service logs for any warnings" -ForegroundColor Gray
Write-Host "  - Check that transcript files are being processed" -ForegroundColor Gray
Write-Host "  - Verify insights are appearing in the Memory service" -ForegroundColor Gray
Write-Host ""
