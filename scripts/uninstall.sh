#!/bin/sh
# simctl - Uninstallation Script for Nura OS
set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo "${BLUE}======================================================${NC}"
echo "${BLUE}       simctl - Uninstallation & Teardown             ${NC}"
echo "${BLUE}======================================================${NC}"

if [ "$(id -u)" -ne 0 ]; then
    echo "${RED}[ERROR] This uninstaller must be run as root (or via sudo).${NC}"
    exit 1
fi

RESTORE=0
PURGE=0

for arg in "$@"; do
    case "$arg" in
        --restore)
            RESTORE=1
            ;;
        --purge)
            PURGE=1
            ;;
        --help|-h)
            echo "Usage: sudo sh uninstall.sh [OPTIONS]"
            echo "Options:"
            echo "  --restore    Re-enable IMS profiles before uninstalling"
            echo "  --purge      Remove configuration and state directories"
            exit 0
            ;;
    esac
done

if [ "$RESTORE" -eq 1 ]; then
    echo "\n${YELLOW}--> Restoring IMS profiles to enabled...${NC}"
    if [ -x /usr/local/bin/simctl ]; then
        /usr/local/bin/simctl enable || true
    elif [ -x /usr/local/bin/dimsd ]; then
        /usr/local/bin/dimsd enable || true
    fi
    echo "${GREEN}[OK] IMS profiles restored.${NC}"
fi

echo "\n${YELLOW}--> Stopping and disabling services...${NC}"
systemctl stop simctl.service 2>/dev/null || true
systemctl disable simctl.service 2>/dev/null || true
systemctl stop dimsd.service 2>/dev/null || true
systemctl disable dimsd.service 2>/dev/null || true
rm -f /etc/systemd/system/simctl.service /etc/systemd/system/dimsd.service
systemctl daemon-reload

echo "\n${YELLOW}--> Removing binaries and symlinks...${NC}"
rm -f /usr/local/bin/simctl /usr/local/bin/dimsd

if [ "$PURGE" -eq 1 ]; then
    echo "\n${YELLOW}--> Purging configuration and state...${NC}"
    rm -rf /etc/simctl /var/lib/simctl /etc/dimsd /var/lib/dimsd
fi

echo "\n${GREEN}[OK] simctl uninstallation complete.${NC}"
