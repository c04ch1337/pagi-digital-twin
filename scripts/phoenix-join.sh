#!/bin/bash
# Phoenix Fleet Manager - Node Registration Script
# 
# This script allows a new bare-metal machine to register itself with
# a primary Phoenix Orchestrator instance.
#
# Usage: ./phoenix-join.sh --gateway <IP_ADDRESS> [--port <PORT>] [--node-id <NODE_ID>]

set -e

GATEWAY_IP=""
GATEWAY_PORT="3000"
NODE_ID=""
HOSTNAME=$(hostname)
IP_ADDRESS=""

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --gateway)
            GATEWAY_IP="$2"
            shift 2
            ;;
        --port)
            GATEWAY_PORT="$2"
            shift 2
            ;;
        --node-id)
            NODE_ID="$2"
            shift 2
            ;;
        *)
            echo "Unknown option: $1"
            echo "Usage: $0 --gateway <IP_ADDRESS> [--port <PORT>] [--node-id <NODE_ID>]"
            exit 1
            ;;
    esac
done

# Validate required arguments
if [ -z "$GATEWAY_IP" ]; then
    echo "Error: --gateway is required"
    echo "Usage: $0 --gateway <IP_ADDRESS> [--port <PORT>] [--node-id <NODE_ID>]"
    exit 1
fi

# Generate node ID if not provided
if [ -z "$NODE_ID" ]; then
    NODE_ID="node-$(uuidgen 2>/dev/null || cat /proc/sys/kernel/random/uuid 2>/dev/null || echo $(date +%s))"
fi

# Get local IP address (prefer non-loopback, non-docker interfaces)
IP_ADDRESS=$(ip route get 8.8.8.8 2>/dev/null | grep -oP 'src \K\S+' || \
            hostname -I 2>/dev/null | awk '{print $1}' || \
            ipconfig getifaddr en0 2>/dev/null || \
            echo "127.0.0.1")

echo "Phoenix Fleet Manager - Node Registration"
echo "=========================================="
echo "Gateway: http://${GATEWAY_IP}:${GATEWAY_PORT}"
echo "Node ID: ${NODE_ID}"
echo "Hostname: ${HOSTNAME}"
echo "IP Address: ${IP_ADDRESS}"
echo ""

# Send heartbeat to gateway
HEARTBEAT_URL="http://${GATEWAY_IP}:${GATEWAY_PORT}/api/fleet/heartbeat"

echo "Sending heartbeat to ${HEARTBEAT_URL}..."

RESPONSE=$(curl -s -w "\n%{http_code}" -X POST "${HEARTBEAT_URL}" \
    -H "Content-Type: application/json" \
    -d "{
        \"node_id\": \"${NODE_ID}\",
        \"hostname\": \"${HOSTNAME}\",
        \"ip_address\": \"${IP_ADDRESS}\",
        \"software_version\": \"2.1.0\"
    }")

HTTP_CODE=$(echo "$RESPONSE" | tail -n1)
BODY=$(echo "$RESPONSE" | sed '$d')

if [ "$HTTP_CODE" -eq 200 ] || [ "$HTTP_CODE" -eq 201 ]; then
    echo "✓ Successfully registered with Phoenix Fleet Manager"
    echo ""
    echo "Response:"
    echo "$BODY" | jq '.' 2>/dev/null || echo "$BODY"
    echo ""
    echo "To keep this node registered, you should set up a periodic heartbeat:"
    echo "  */30 * * * * curl -X POST ${HEARTBEAT_URL} -H 'Content-Type: application/json' -d '{\"node_id\":\"${NODE_ID}\",\"hostname\":\"${HOSTNAME}\",\"ip_address\":\"${IP_ADDRESS}\",\"software_version\":\"2.1.0\"}'"
    echo ""
    echo "Or use a systemd timer or cron job to run this script periodically."
else
    echo "✗ Failed to register (HTTP ${HTTP_CODE})"
    echo "Response:"
    echo "$BODY"
    exit 1
fi
