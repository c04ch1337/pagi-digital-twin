#!/usr/bin/env bash
set -euo pipefail

# 1) Build + start the stack
docker compose build
docker compose up -d

# 2) Test request (requires tool use + RAG)
AGENT_HOST="http://localhost:8585"
SESSION_ID="test-session-$(date +%s)"
TEST_PROMPT="Use the web_search tool once, then return the JSON field title from the tool result. Do not call any more tools."

echo "--- Sending Test Prompt to Agent Planner ($AGENT_HOST) ---"
echo "Prompt: $TEST_PROMPT"
echo "Session ID: $SESSION_ID"

curl -sS -X POST "$AGENT_HOST/plan" \
  -H "Content-Type: application/json" \
  -d '{
    "prompt": '"'"$TEST_PROMPT"'"',
    "session_id": '"'"$SESSION_ID"'"'
  }' \
  | python -m json.tool
