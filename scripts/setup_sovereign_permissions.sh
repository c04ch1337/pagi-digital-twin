#!/bin/bash

# Ferrellgas AGI - Sovereign Permissions Setup Script
# This script configures the bare-metal environment to allow the Orchestrator
# service to execute system commands without manual password entry.
#
# Usage: sudo ./setup_sovereign_permissions.sh [TELEMETRY_STORAGE_DIR]
#
# Prerequisites:
# - Must be run as root (sudo)
# - Linux system with systemd
# - sudo package installed

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
ORCHESTRATOR_USER="agi-orchestrator"
SUDOERS_FILE="/etc/sudoers.d/${ORCHESTRATOR_USER}"
TELEMETRY_STORAGE_DIR="${1:-./storage}"

echo -e "${GREEN}🚀 Setting up Sovereign Permissions for AGI Orchestrator${NC}"
echo "=================================================="
echo ""

# Check if running as root
if [ "$EUID" -ne 0 ]; then 
    echo -e "${RED}❌ Error: This script must be run as root (use sudo)${NC}"
    exit 1
fi

# Step 1: Create dedicated system user
echo -e "${YELLOW}📝 Step 1: Creating system user '${ORCHESTRATOR_USER}'${NC}"
if id "$ORCHESTRATOR_USER" &>/dev/null; then
    echo -e "   User '${ORCHESTRATOR_USER}' already exists, skipping creation"
else
    useradd -r -s /bin/bash -d /var/lib/${ORCHESTRATOR_USER} -m ${ORCHESTRATOR_USER}
    echo -e "${GREEN}   ✅ User '${ORCHESTRATOR_USER}' created successfully${NC}"
fi

# Step 2: Create sudoers file
echo -e "${YELLOW}📝 Step 2: Configuring sudoers permissions${NC}"
cat > "${SUDOERS_FILE}" << 'EOF'
# Ferrellgas AGI Orchestrator - Sudoers Configuration
# This file allows the agi-orchestrator user to manage peer services without password prompts
# Created by setup_sovereign_permissions.sh

# Allow the Orchestrator to manage its peer services without a password
agi-orchestrator ALL=(ALL) NOPASSWD: /usr/bin/systemctl restart telemetry
agi-orchestrator ALL=(ALL) NOPASSWD: /usr/bin/systemctl restart gateway
agi-orchestrator ALL=(ALL) NOPASSWD: /usr/bin/systemctl restart sys_control
agi-orchestrator ALL=(ALL) NOPASSWD: /usr/bin/systemctl start telemetry
agi-orchestrator ALL=(ALL) NOPASSWD: /usr/bin/systemctl start gateway
agi-orchestrator ALL=(ALL) NOPASSWD: /usr/bin/systemctl start sys_control
agi-orchestrator ALL=(ALL) NOPASSWD: /usr/bin/systemctl stop telemetry
agi-orchestrator ALL=(ALL) NOPASSWD: /usr/bin/systemctl stop gateway
agi-orchestrator ALL=(ALL) NOPASSWD: /usr/bin/systemctl stop sys_control
agi-orchestrator ALL=(ALL) NOPASSWD: /usr/bin/systemctl status telemetry
agi-orchestrator ALL=(ALL) NOPASSWD: /usr/bin/systemctl status gateway
agi-orchestrator ALL=(ALL) NOPASSWD: /usr/bin/systemctl status sys_control
agi-orchestrator ALL=(ALL) NOPASSWD: /usr/bin/systemctl enable telemetry
agi-orchestrator ALL=(ALL) NOPASSWD: /usr/bin/systemctl enable gateway
agi-orchestrator ALL=(ALL) NOPASSWD: /usr/bin/systemctl enable sys_control
agi-orchestrator ALL=(ALL) NOPASSWD: /usr/bin/systemctl disable telemetry
agi-orchestrator ALL=(ALL) NOPASSWD: /usr/bin/systemctl disable gateway
agi-orchestrator ALL=(ALL) NOPASSWD: /usr/bin/systemctl disable sys_control
agi-orchestrator ALL=(ALL) NOPASSWD: /usr/bin/journalctl -u telemetry *
agi-orchestrator ALL=(ALL) NOPASSWD: /usr/bin/journalctl -u gateway *
agi-orchestrator ALL=(ALL) NOPASSWD: /usr/bin/journalctl -u sys_control *
EOF

# Set proper permissions on sudoers file (must be 0440)
chmod 0440 "${SUDOERS_FILE}"
echo -e "${GREEN}   ✅ Sudoers file created at ${SUDOERS_FILE}${NC}"

# Validate sudoers file syntax
if visudo -c -f "${SUDOERS_FILE}" 2>/dev/null; then
    echo -e "${GREEN}   ✅ Sudoers file syntax validated${NC}"
else
    echo -e "${RED}   ❌ Warning: Sudoers file syntax validation failed${NC}"
    echo -e "${YELLOW}   Please review ${SUDOERS_FILE} manually${NC}"
fi

# Step 3: Setup storage directory permissions
echo -e "${YELLOW}📝 Step 3: Configuring storage directory permissions${NC}"
if [ -d "${TELEMETRY_STORAGE_DIR}" ]; then
    # Get absolute path
    ABS_STORAGE_DIR=$(realpath "${TELEMETRY_STORAGE_DIR}")
    
    # Change ownership to orchestrator user
    chown -R ${ORCHESTRATOR_USER}:${ORCHESTRATOR_USER} "${ABS_STORAGE_DIR}"
    echo -e "${GREEN}   ✅ Changed ownership of ${ABS_STORAGE_DIR} to ${ORCHESTRATOR_USER}${NC}"
    
    # Set permissions (read/write/execute for owner, read/execute for group, read/execute for others)
    chmod -R 755 "${ABS_STORAGE_DIR}"
    echo -e "${GREEN}   ✅ Set permissions on ${ABS_STORAGE_DIR}${NC}"
else
    echo -e "${YELLOW}   ⚠️  Storage directory '${TELEMETRY_STORAGE_DIR}' does not exist${NC}"
    echo -e "${YELLOW}   Creating directory and setting permissions...${NC}"
    
    # Create directory
    mkdir -p "${TELEMETRY_STORAGE_DIR}"
    ABS_STORAGE_DIR=$(realpath "${TELEMETRY_STORAGE_DIR}")
    
    # Set ownership and permissions
    chown -R ${ORCHESTRATOR_USER}:${ORCHESTRATOR_USER} "${ABS_STORAGE_DIR}"
    chmod -R 755 "${ABS_STORAGE_DIR}"
    
    echo -e "${GREEN}   ✅ Created and configured ${ABS_STORAGE_DIR}${NC}"
fi

# Step 4: Create recordings subdirectory if needed
RECORDINGS_DIR="${ABS_STORAGE_DIR}/recordings"
if [ ! -d "${RECORDINGS_DIR}" ]; then
    mkdir -p "${RECORDINGS_DIR}"
    chown ${ORCHESTRATOR_USER}:${ORCHESTRATOR_USER} "${RECORDINGS_DIR}"
    chmod 755 "${RECORDINGS_DIR}"
    echo -e "${GREEN}   ✅ Created recordings directory at ${RECORDINGS_DIR}${NC}"
fi

echo ""
echo -e "${GREEN}✅ Sovereign Permissions Setup Complete!${NC}"
echo ""
echo "Summary:"
echo "  - User created: ${ORCHESTRATOR_USER}"
echo "  - Sudoers file: ${SUDOERS_FILE}"
echo "  - Storage directory: ${ABS_STORAGE_DIR}"
echo ""
echo "Next steps:"
echo "  1. Ensure services are configured to run as ${ORCHESTRATOR_USER}"
echo "  2. Set TELEMETRY_STORAGE_DIR=${ABS_STORAGE_DIR} in your environment"
echo "  3. Test sudo access: sudo -u ${ORCHESTRATOR_USER} sudo systemctl status telemetry"
echo ""
