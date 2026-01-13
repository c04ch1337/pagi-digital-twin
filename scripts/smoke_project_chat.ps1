param(
  [string]$ProjectName = 'Project Alpha',
  [string]$TwinId = 'twin-sentinel',
  [string]$OrchestratorUrl = 'http://127.0.0.1:8182/chat'
)

$ErrorActionPreference = 'Stop'

$sessionId = [guid]::NewGuid().ToString()

$body = @{
  message      = "Creating chats under $ProjectName"
  twin_id      = $TwinId
  session_id   = $sessionId
  namespace    = 'default'
  media_active = $false
} | ConvertTo-Json

$resp = Invoke-RestMethod -Method Post -Uri $OrchestratorUrl -ContentType 'application/json' -Body $body

if (-not $resp) {
  throw 'No response from orchestrator'
}

if ($resp.status -ne 'completed') {
  throw "Expected status=completed, got status=$($resp.status). Full response: $($resp | ConvertTo-Json -Depth 20)"
}

if (-not $resp.issued_command) {
  throw "Expected issued_command, got null. Full response: $($resp | ConvertTo-Json -Depth 20)"
}

if ($resp.issued_command.command -ne 'create_project_chat') {
  throw "Expected issued_command.command=create_project_chat, got $($resp.issued_command.command). Full response: $($resp | ConvertTo-Json -Depth 20)"
}

if ($resp.issued_command.project_name -ne $ProjectName) {
  throw "Expected issued_command.project_name='$ProjectName', got '$($resp.issued_command.project_name)'. Full response: $($resp | ConvertTo-Json -Depth 20)"
}

Write-Host "OK: create_project_chat issued for project '$ProjectName' (session_id=$sessionId)" -ForegroundColor Green

