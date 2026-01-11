# 1) Build + start the stack
docker compose build
docker compose up -d

# 2) Test request (requires tool use + RAG)
$AgentHost = "http://localhost:8585"
$SessionId = "test-session-$([DateTimeOffset]::UtcNow.ToUnixTimeSeconds())"
$TestPrompt = "Use the web_search tool once, then return the JSON field title from the tool result. Do not call any more tools."

Write-Host "--- Sending Test Prompt to Agent Planner ($AgentHost) ---"
Write-Host "Prompt: $TestPrompt"
Write-Host "Session ID: $SessionId"

$Body = @{ prompt = $TestPrompt; session_id = $SessionId } | ConvertTo-Json

Invoke-RestMethod -Method Post -Uri "$AgentHost/plan" -ContentType "application/json" -Body $Body |
  ConvertTo-Json -Depth 10

