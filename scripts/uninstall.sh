#!/bin/sh
# dimsd - Disable IMS Daemon & CLI Uninstaller Script for Nura OS
set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo "${BLUE}======================================================${NC}"
echo "${BLUE}   dimsd (Disable IMS Daemon) - Uninstallation Script   ${NC}"
echo "${BLUE}======================================================${NC}"

if [ "$(id -u)" -ne 0 ]; then
    echo "${RED}[ERROR] This uninstaller must be run as root (or via sudo).${NC}"
    exit 1
fi

RESTORE_IMS=0
PURGE_CONFIG=0

for arg in "$@"; do
    case "$arg" in
        --restore|-r)
            RESTORE_IMS=1
            ;;
        --purge|-p)
            PURGE_CONFIG=1
            ;;
        --help|-h)
            echo "Usage: $0 [--restore] [--purge]"
            echo "  --restore, -r : Restore IMS/VoLTE profiles and bearers back to enabled"
            echo "  --purge, -p   : Remove /etc/dimsd configuration and /var/lib/dimsd state"
            exit 0
            ;;
    esac
done

# Step 1: Optional IMS restoration
if [ "$RESTORE_IMS" -eq 1 ]; then
    echo "\n${YELLOW}--> Step 1: Restoring IMS profiles to enabled state...${NC}"
    if [ -x /usr/local/bin/dimsd ]; then
        /usr/local/bin/dimsd enable --slot all || echo "${YELLOW}[!] Warning: Failed to re-enable all IMS profiles.${NC}"
        echo "${GREEN}[OK] IMS profiles restored.${NC}"
    else
        echo "${YELLOW}[!] /usr/local/bin/dimsd not found; skipping automated profile restore.${NC}"
    fi
else
    echo "\n${BLUE}[i] Note: IMS profiles left in their current state. (Use --restore to re-enable).${NC}"
fi

# Step 2: Stopping and disabling systemd service
echo "\n${YELLOW}--> Step 2: Stopping and disabling dimsd.service...${NC}"
if systemctl is-active --quiet dimsd.service 2>/dev/null; then
    systemctl stop dimsd.service || true
fi

if systemctl is-enabled --quiet dimsd.service 2>/dev/null; then
    systemctl disable dimsd.service || true
fi

if [ -f /etc/systemd/system/dimsd.service ]; then
    rm -f /etc/systemd/system/dimsd.service
    systemctl daemon-reload
    echo "${GREEN}[OK] dimsd.service removed and systemd reloaded.${NC}"
fi

# Step 3: Removing binary
echo "\n${YELLOW}--> Step 3: Removing /usr/local/bin/dimsd...${NC}"
if [ -f /usr/local/bin/dimsd ]; then
    rm -f /usr/local/bin/dimsd
    echo "${GREEN}[OK] Binary /usr/local/bin/dimsd removed.${NC}"
fi

# Step 4: Purge configuration if requested
if [ "$PURGE_CONFIG" -eq 1 ]; then
    echo "\n${YELLOW}--> Step 4: Purging configuration and state...${NC}"
    rm -rf /etc/dimsd /var/lib/dimsd
    echo "${GREEN}[OK] Configuration and state purged.${NC}"
else
    echo "\n${BLUE}[i] Configuration preserved in /etc/dimsd. (Pass --purge to remove).${NC}"
fi

echo "\n${GREEN}======================================================${NC}"
echo "${GREEN}     dimsd has been successfully uninstalled.          ${NC}"
echo "${GREEN}======================================================${NC}"

