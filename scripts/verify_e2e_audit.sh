#!/usr/bin/env bash
set -euo pipefail

AGENT_URL="${AGENT_URL:-http://localhost:8585}"
SESSION_ID="${SESSION_ID:-e2e-session-$(date +%s)}"
PROMPT="${PROMPT:-Return a short JSON object with keys hello and trace_check.}"

HEADERS_FILE=""
BODY_FILE=""

cleanup() {
  rm -f "${HEADERS_FILE}" "${BODY_FILE}" 2>/dev/null || true
  docker compose down -v >/dev/null 2>&1 || true
}
trap cleanup EXIT

echo "[1/6] Building and starting docker compose stack..."
docker compose build
docker compose up -d

echo "[2/6] Waiting for Agent Planner to become healthy..."
for i in {1..60}; do
  if curl -fsS "${AGENT_URL}/health" >/dev/null; then
    break
  fi
  sleep 2
  if [[ $i -eq 60 ]]; then
    echo "ERROR: Agent Planner did not become healthy at ${AGENT_URL}/health" >&2
    exit 1
  fi
done

echo "[3/6] Executing a single /plan transaction and capturing X-Trace-ID..."
HEADERS_FILE="$(mktemp)"
BODY_FILE="$(mktemp)"

cat >"${BODY_FILE}" <<EOF
{"prompt":"${PROMPT}","session_id":"${SESSION_ID}"}
EOF

curl -sS -D "${HEADERS_FILE}" -o /tmp/pagi_e2e_response.json \
  -X POST "${AGENT_URL}/plan" \
  -H "Content-Type: application/json" \
  --data-binary @"${BODY_FILE}"

TRACE_ID="$(grep -i '^X-Trace-ID:' "${HEADERS_FILE}" | head -n1 | awk '{print $2}' | tr -d '\r')"
if [[ -z "${TRACE_ID}" ]]; then
  echo "ERROR: Missing X-Trace-ID header in response" >&2
  echo "Response headers:" >&2
  cat "${HEADERS_FILE}" >&2
  exit 1
fi
echo "Captured TRACE_ID=${TRACE_ID}"

echo "[4/6] Verifying notification-service received TRACE_ID..."
FOUND_NOTIFICATION=0
for i in {1..30}; do
  if docker compose logs --no-color notification-service 2>/dev/null | grep -q "${TRACE_ID}"; then
    FOUND_NOTIFICATION=1
    break
  fi
  sleep 1
done
if [[ "${FOUND_NOTIFICATION}" -ne 1 ]]; then
  echo "ERROR: notification-service logs did not contain TRACE_ID=${TRACE_ID}" >&2
  docker compose logs --no-color notification-service || true
  exit 1
fi
echo "Notification verification passed."

echo "[5/6] Verifying SQLite audit log contains TRACE_ID..."
COUNT="$(docker compose exec -T audit-inspector sqlite3 /audit/pagi_audit.db "SELECT COUNT(*) FROM audit_log WHERE trace_id='${TRACE_ID}';" | tr -d '\r' | tr -d ' ')"
if [[ -z "${COUNT}" ]] || [[ "${COUNT}" -lt 1 ]]; then
  echo "ERROR: No audit_log rows found for TRACE_ID=${TRACE_ID}" >&2
  echo "Dumping recent audit_log rows:" >&2
  docker compose exec -T audit-inspector sqlite3 /audit/pagi_audit.db "SELECT id, trace_id, session_id, timestamp, event_type FROM audit_log ORDER BY id DESC LIMIT 20;" || true
  exit 1
fi

echo "Audit verification passed (rows=${COUNT})."
echo "Sample rows for this trace:" 
docker compose exec -T audit-inspector sqlite3 /audit/pagi_audit.db "SELECT id, trace_id, session_id, timestamp, event_type FROM audit_log WHERE trace_id='${TRACE_ID}' ORDER BY id;"

echo "[6/6] SUCCESS: E2E test, async notification, and audit log verification completed."

