#!/bin/sh
# simctl - Cellular SIM & Modem Management Daemon & CLI Installation Script for Nura OS
set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo "${BLUE}======================================================${NC}"
echo "${BLUE}    simctl (Cellular SIM & Modem Manager) - Install   ${NC}"
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
for p in "$PROJECT_ROOT/target/release/simctl" \
         "$PROJECT_ROOT/target/debug/simctl" \
         "$PROJECT_ROOT/simctl"; do
    if [ -f "$p" ]; then
        BINARY_SRC="$p"
        break
    fi
done

if [ -z "$BINARY_SRC" ]; then
    echo "${YELLOW}[!] Compiled binary not found in project root or target directories.${NC}"
    if command -v cargo >/dev/null 2>&1; then
        echo "Building debug binary via cargo..."
        (cd "$PROJECT_ROOT" && cargo build)
        BINARY_SRC="$PROJECT_ROOT/target/debug/simctl"
    else
        echo "${RED}[ERROR] Neither pre-compiled binary nor cargo was found.${NC}"
        echo "Please place the compiled 'simctl' binary in $PROJECT_ROOT or install cargo."
        exit 1
    fi
fi

echo "\n${YELLOW}--> Step 2: Installing binary to /usr/local/bin/simctl...${NC}"
install -m 755 "$BINARY_SRC" /usr/local/bin/simctl
# Clean up any legacy dimsd binary/symlink
rm -f /usr/local/bin/dimsd
echo "${GREEN}[OK] Binary installed to /usr/local/bin/simctl${NC}"

echo "\n${YELLOW}--> Step 3: Setting up configuration (/etc/simctl/)...${NC}"
mkdir -p /etc/simctl /var/lib/simctl
if [ ! -f /etc/simctl/config.toml ]; then
    if [ -f "$PROJECT_ROOT/config.example.toml" ]; then
        cp "$PROJECT_ROOT/config.example.toml" /etc/simctl/config.toml
    else
        cat << 'EOF' > /etc/simctl/config.toml
[modem]
device = "auto"
modem_id = "auto"
default_slot = "all"
net_interface = "auto"

[daemon]
timeout_secs = 30
poll_interval_secs = 10
auto_sync_config = true

[logging]
level = "info"
EOF
    fi
    chmod 644 /etc/simctl/config.toml
    echo "${GREEN}[OK] Configuration installed at /etc/simctl/config.toml${NC}"
else
    echo "${BLUE}[i] Existing configuration found at /etc/simctl/config.toml (preserved).${NC}"
fi

# Clean up any legacy /etc/dimsd directory/symlink
rm -rf /etc/dimsd /var/lib/dimsd

echo "\n${YELLOW}--> Step 4: Installing and activating systemd service...${NC}"
# Stop and disable any legacy dimsd service
systemctl stop dimsd.service 2>/dev/null || true
systemctl disable dimsd.service 2>/dev/null || true
rm -f /etc/systemd/system/dimsd.service

if [ -f "$PROJECT_ROOT/systemd/simctl.service" ]; then
    cp "$PROJECT_ROOT/systemd/simctl.service" /etc/systemd/system/simctl.service
else
    cat << 'EOF' > /etc/systemd/system/simctl.service
[Unit]
Description=Cellular SIM & Modem Management Daemon (simctl)
Documentation=https://github.com/nura-eco/simctl
After=ModemManager.service qmi-proxy.service
Wants=ModemManager.service qmi-proxy.service

[Service]
Type=oneshot
RemainAfterExit=yes
ExecStart=/usr/local/bin/simctl disable
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF
fi

systemctl daemon-reload
systemctl enable simctl.service
systemctl restart simctl.service
echo "${GREEN}[OK] systemd service enabled and started: simctl.service${NC}"

echo "\n${YELLOW}--> Step 5: Executing initial hardware verification...${NC}"
/usr/local/bin/simctl check || true

echo "\n${GREEN}======================================================${NC}"
echo "${GREEN}  simctl successfully installed and enabled on boot!  ${NC}"
echo "${GREEN}======================================================${NC}"
echo "Quick Commands:"
echo "  simctl status            - Check modem, SIM slots & profile state"
echo "  simctl switch-slot       - Inspect SIM slots & get switch suggestion"
echo "  simctl switch-slot <1|2> - Switch active primary data SIM"
echo "  simctl profile list      - List active APN profiles"
echo "  simctl disable           - Disable IMS across all SIM slots"
echo "  simctl enable            - Re-enable IMS profiles"
echo ""
