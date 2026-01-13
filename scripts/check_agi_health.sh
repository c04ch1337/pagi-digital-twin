#!/bin/bash

# Ferrellgas AGI Triple-Service Diagnostic Script
# This script verifies the networking and gRPC reachability between
# Telemetry, Orchestrator, and Memory services.

set -e

# Configuration - Update these with your actual bare-metal IPs/Ports
ORCHESTRATOR_GRPC_ADDR="${ORCHESTRATOR_GRPC_ADDR:-127.0.0.1:50057}"
ORCHESTRATOR_HTTP_ADDR="${ORCHESTRATOR_HTTP_ADDR:-127.0.0.1:8182}"
TELEMETRY_HTTP_ADDR="${TELEMETRY_HTTP_ADDR:-127.0.0.1:8183}"
MEMORY_GRPC_ADDR="${MEMORY_GRPC_ADDR:-127.0.0.1:50052}"
GATEWAY_HTTP_ADDR="${GATEWAY_HTTP_ADDR:-127.0.0.1:8181}"

echo "🚀 Starting Ferrellgas AGI Triple-Service Diagnostic..."
echo "=================================================="
echo ""

# Check if grpcurl is installed
if ! command -v grpcurl &> /dev/null; then
    echo "❌ ERROR: grpcurl is not installed."
    echo "   Install it with: brew install grpcurl (macOS) or download from https://github.com/fullstorydev/grpcurl"
    exit 1
fi

# Check if curl is installed
if ! command -v curl &> /dev/null; then
    echo "❌ ERROR: curl is not installed."
    exit 1
fi

# 1. Check Service Liveness (HTTP Health Checks)
echo "📡 Step 1: Checking Service Liveness (HTTP)"
echo "--------------------------------------------"

echo -n "  Checking Orchestrator HTTP (${ORCHESTRATOR_HTTP_ADDR})... "
if curl -s -f "http://${ORCHESTRATOR_HTTP_ADDR}/health" > /dev/null 2>&1; then
    echo "✅ ONLINE"
else
    echo "❌ OFFLINE"
    echo "    → Check if Orchestrator is running on port 8182"
fi

echo -n "  Checking Telemetry HTTP (${TELEMETRY_HTTP_ADDR})... "
if curl -s -f "http://${TELEMETRY_HTTP_ADDR}/v1/telemetry/stream" --max-time 2 > /dev/null 2>&1; then
    echo "✅ ONLINE"
else
    echo "❌ OFFLINE"
    echo "    → Check if Telemetry is running on port 8183"
fi

echo -n "  Checking Gateway HTTP (${GATEWAY_HTTP_ADDR})... "
if curl -s -f "http://${GATEWAY_HTTP_ADDR}/api/health" > /dev/null 2>&1; then
    echo "✅ ONLINE"
else
    echo "❌ OFFLINE"
    echo "    → Check if Gateway is running on port 8181"
fi

echo ""
echo "📡 Step 2: Checking gRPC Service Liveness"
echo "--------------------------------------------"

echo -n "  Checking Orchestrator gRPC (${ORCHESTRATOR_GRPC_ADDR})... "
if grpcurl -plaintext "${ORCHESTRATOR_GRPC_ADDR}" list > /dev/null 2>&1; then
    echo "✅ ONLINE"
    echo "    Available services:"
    grpcurl -plaintext "${ORCHESTRATOR_GRPC_ADDR}" list | sed 's/^/      - /'
else
    echo "❌ OFFLINE"
    echo "    → Check if Orchestrator gRPC is running on port 50057"
    echo "    → Verify ORCHESTRATOR_GRPC_PORT environment variable"
fi

echo -n "  Checking Memory gRPC (${MEMORY_GRPC_ADDR})... "
if grpcurl -plaintext "${MEMORY_GRPC_ADDR}" list > /dev/null 2>&1; then
    echo "✅ ONLINE"
    echo "    Available services:"
    grpcurl -plaintext "${MEMORY_GRPC_ADDR}" list | sed 's/^/      - /'
else
    echo "❌ OFFLINE"
    echo "    → Check if Memory service is running on port 50052"
    echo "    → Verify MEMORY_GRPC_PORT environment variable"
fi

echo ""
echo "🔗 Step 3: Testing Telemetry -> Orchestrator Handshake (Summarization)"
echo "------------------------------------------------------------------------"

# Test transcript summarization
TEST_TRANSCRIPT='{"transcript_text": "This is a test of the automated insight system. We discussed several important topics and made key decisions."}'

echo "  Sending test transcript to Orchestrator..."
SUMMARY_RESPONSE=$(grpcurl -plaintext -d "${TEST_TRANSCRIPT}" \
    "${ORCHESTRATOR_GRPC_ADDR}" \
    orchestrator.OrchestratorService/SummarizeTranscript 2>&1)

if echo "${SUMMARY_RESPONSE}" | grep -q "summary"; then
    echo "  ✅ SUCCESS: Orchestrator processed the transcript."
    echo "  Response preview:"
    echo "${SUMMARY_RESPONSE}" | head -5 | sed 's/^/    /'
else
    echo "  ❌ FAILED: Telemetry cannot get a summary from Orchestrator."
    echo "  Error details:"
    echo "${SUMMARY_RESPONSE}" | sed 's/^/    /'
    echo ""
    echo "  Troubleshooting:"
    echo "    → Check ORCHESTRATOR_GRPC_ADDR in Telemetry service"
    echo "    → Verify LLM_PROVIDER=openrouter and OPENROUTER_API_KEY is set"
    echo "    → Check Orchestrator logs for errors"
fi

echo ""
echo "🔗 Step 4: Testing Orchestrator -> Memory Handshake (Neural Archive)"
echo "------------------------------------------------------------------------"

# Test memory commit
TEST_MEMORY='{
  "content": "Deployment test successful. This is a diagnostic entry.",
  "namespace": "system_logs",
  "twin_id": "diagnostic-test",
  "memory_type": "RAGSource",
  "risk_level": "Low"
}'

echo "  Attempting to commit test entry to Memory service..."
MEMORY_RESPONSE=$(grpcurl -plaintext -d "${TEST_MEMORY}" \
    "${MEMORY_GRPC_ADDR}" \
    memory.MemoryService/CommitMemory 2>&1)

if echo "${MEMORY_RESPONSE}" | grep -q "success"; then
    echo "  ✅ SUCCESS: Orchestrator successfully committed to Neural Archive."
    MEMORY_ID=$(echo "${MEMORY_RESPONSE}" | grep -o '"memory_id":"[^"]*"' | cut -d'"' -f4 || echo "unknown")
    echo "  Memory ID: ${MEMORY_ID}"
else
    echo "  ❌ FAILED: Orchestrator cannot write to Memory service."
    echo "  Error details:"
    echo "${MEMORY_RESPONSE}" | sed 's/^/    /'
    echo ""
    echo "  Troubleshooting:"
    echo "    → Check MEMORY_GRPC_ADDR in Orchestrator service"
    echo "    → Verify Memory service is running and Qdrant is accessible"
    echo "    → Check Memory service logs for errors"
fi

echo ""
echo "=================================================="
echo "✅ Diagnostic Complete!"
echo ""
echo "If all tests passed, your AGI Closed-Loop Intelligence System is active."
echo ""
echo "Next Steps:"
echo "  - Monitor service logs for any warnings"
echo "  - Check that transcript files are being processed"
echo "  - Verify insights are appearing in the Memory service"
echo ""
