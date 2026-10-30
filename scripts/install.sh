#!/bin/sh
# dimsd - Disable IMS Daemon & CLI Installation Script for Nura OS
set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo "${BLUE}======================================================${NC}"
echo "${BLUE}    dimsd (Disable IMS Daemon) - Installation Script    ${NC}"
echo "${BLUE}======================================================${NC}"

# Check root privileges
if [ "$(id -u)" -ne 0 ]; then
    echo "${RED}[ERROR] This installer must be run as root (or via sudo).${NC}"
    exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "\n${YELLOW}--> Step 1: Pre-flight Environment Checks...${NC}"

# Check for required tools
MISSING_TOOLS=""
for tool in mmcli qmicli systemctl; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        MISSING_TOOLS="$MISSING_TOOLS $tool"
    fi
done

if [ -n "$MISSING_TOOLS" ]; then
    echo "${RED}[ERROR] Missing required dependencies:${MISSING_TOOLS}${NC}"
    echo "Please ensure ModemManager, libqmi, and systemd are installed."
    exit 1
fi
echo "${GREEN}[OK] Required binaries found (mmcli, qmicli, systemctl).${NC}"

# Locate compiled binary
BINARY_SRC=""
if [ -f "$PROJECT_ROOT/target/release/dimsd" ]; then
    BINARY_SRC="$PROJECT_ROOT/target/release/dimsd"
elif [ -f "$PROJECT_ROOT/target/debug/dimsd" ]; then
    BINARY_SRC="$PROJECT_ROOT/target/debug/dimsd"
elif [ -f "$PROJECT_ROOT/dimsd" ]; then
    BINARY_SRC="$PROJECT_ROOT/dimsd"
else
    echo "${YELLOW}[!] Compiled binary not found in project root or target directories.${NC}"
    if command -v cargo >/dev/null 2>&1; then
        echo "Building debug binary via cargo..."
        (cd "$PROJECT_ROOT" && cargo build)
        BINARY_SRC="$PROJECT_ROOT/target/debug/dimsd"
    else
        echo "${RED}[ERROR] Neither pre-compiled binary nor cargo was found.${NC}"
        echo "Please place the compiled 'dimsd' binary in $PROJECT_ROOT or install cargo."
        exit 1
    fi
fi

echo "\n${YELLOW}--> Step 2: Installing binary to /usr/local/bin/dimsd...${NC}"
install -m 755 "$BINARY_SRC" /usr/local/bin/dimsd
echo "${GREEN}[OK] Binary installed to /usr/local/bin/dimsd${NC}"

echo "\n${YELLOW}--> Step 3: Setting up configuration (/etc/dimsd/)...${NC}"
mkdir -p /etc/dimsd /var/lib/dimsd
if [ ! -f /etc/dimsd/config.toml ]; then
    if [ -f "$PROJECT_ROOT/config.example.toml" ]; then
        cp "$PROJECT_ROOT/config.example.toml" /etc/dimsd/config.toml
    else
        cat << 'EOF' > /etc/dimsd/config.toml
[modem]
device = "auto"
modem_id = "auto"
default_slot = "1"
net_interface = "auto"

[daemon]
timeout_secs = 30
poll_interval_secs = 10
auto_sync_config = true

[logging]
level = "info"
EOF
    fi
    chmod 644 /etc/dimsd/config.toml
    echo "${GREEN}[OK] Default configuration installed at /etc/dimsd/config.toml${NC}"
else
    echo "${BLUE}[i] Existing configuration found at /etc/dimsd/config.toml (preserved).${NC}"
fi

echo "\n${YELLOW}--> Step 4: Installing and activating systemd service...${NC}"
if [ -f "$PROJECT_ROOT/systemd/dimsd.service" ]; then
    cp "$PROJECT_ROOT/systemd/dimsd.service" /etc/systemd/system/dimsd.service
else
    cat << 'EOF' > /etc/systemd/system/dimsd.service
[Unit]
Description=Disable IMS Daemon (dimsd)
Documentation=https://github.com/nura-eco/dimsd
After=ModemManager.service qmi-proxy.service
Wants=ModemManager.service qmi-proxy.service

[Service]
Type=oneshot
RemainAfterExit=yes
ExecStart=/usr/local/bin/dimsd disable
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF
fi

chmod 644 /etc/systemd/system/dimsd.service
systemctl daemon-reload
echo "${GREEN}[OK] systemd daemon reloaded.${NC}"

echo "Enabling and starting dimsd.service..."
systemctl enable --now dimsd.service || true

echo "\n${YELLOW}--> Step 5: Live Hardware Check and Configuration Sync...${NC}"
echo "Running /usr/local/bin/dimsd check --update to verify live hardware..."
/usr/local/bin/dimsd check --update || true

echo "\n${GREEN}======================================================${NC}"
echo "${GREEN}      dimsd Installation Completed Successfully!       ${NC}"
echo "${GREEN}======================================================${NC}"
echo "Useful Commands:"
echo "  dimsd status          # Check live SIM, bearer, and IMS status"
echo "  dimsd disable         # Disconnect IMS bearers & disable profiles"
echo "  dimsd enable          # Re-enable IMS profiles"
echo "  dimsd check           # Verify hardware against config.toml"
echo "  systemctl status dimsd.service # Check systemd service status"

