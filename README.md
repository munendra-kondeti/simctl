# dimsd - Disable IMS Daemon & CLI

`dimsd` is a lightweight, robust system daemon and command-line utility written in Rust for **Nura OS** (postmarketOS / Alpine Linux `aarch64` Qualcomm MSM8953).

It detects available SIM cards across **Dual SIM slots** (Slot 1 & Slot 2), defaulting to Slot 1, and disables IP Multimedia Subsystem (IMS/VoLTE) bearers and profiles on boot. It also provides a rich CLI to monitor status, switch active SIM data sessions, manage APN profiles, and enable/disable IMS.

> [!TIP]
> For in-depth tutorials, DSDS hardware architecture explanations, and step-by-step guides, consult the [Comprehensive User Guide](docs/USER_GUIDE.md).

---

## Features

- **Dual-SIM Slot Management**: Dynamically queries QMI UIM card status (`Slot [1]` & `Slot [2]`). Defaults to Slot 1 (`-s 1`), with support for Slot 2 (`-s 2`) or all slots (`-s all`).
- **SIM Session Switching**: Switch the active cellular data provisioning session (`Primary GW`) between Slot 1 and Slot 2 on the fly via `dimsd switch-slot <1|2>`.
- **WDS 3GPP APN Profile Management**: Create, list, and delete APN profiles directly in modem NVRAM via `dimsd profile add|delete|list`.
- **Multi-Layer IMS Disabling**:
  1. Identifies and disconnects active ModemManager bearers configured with APN `ims` via native JSON (`mmcli -J`).
  2. Modifies Qualcomm QMI WDS 3GPP profiles (`qmicli -d qrtr://0 --wds-modify-profile="3gpp,<index>,disabled=yes"`).
  3. Disables autoconnect on the IMS context to prevent background re-attachment.
  4. Verifies de-registration via QMI IMSA status.
- **Dynamic Port & Hardware Discovery**:
  - Dynamically discovers active ModemManager indices (`Modem/0`, `Modem/1`) and QRTR endpoints (`qrtr://0`) on every run.
  - Built-in `dimsd check --update` command to verify live hardware and atomically sync `/etc/dimsd/config.toml`.
- **Systemd Boot Integration**:
  - Runs on boot after `ModemManager.service` and `qmi-proxy.service`.
  - Implements an intelligent retry loop (`timeout_secs = 30`) to eliminate cold-boot race conditions while SIM cards initialize.
- **Safe State Rollback**:
  - Saves modified profile state to `/var/lib/dimsd/state.json`.
  - Uninstaller includes `--restore` to re-enable IMS profiles to their default state.
- **Native JSON Support**:
  - All status and action commands support `--json` for automation and script integration.

---

## Directory Structure

```text
dimsd/
├── Cargo.toml             # Rust package manifest
├── config.example.toml    # Default example configuration
├── README.md              # Project manual & CLI overview
├── docs/
│   ├── USER_GUIDE.md      # Comprehensive operator manual & user guide
│   ├── plan.md            # Architecture & implementation plan specification
│   └── reviews/           # Rust best-practices audits and verification logs
├── scripts/
│   ├── install.sh         # Installation script (builds/installs binary & systemd service)
│   └── uninstall.sh       # Uninstallation script (with optional IMS restoration)
├── systemd/
│   └── dimsd.service      # Systemd unit file
└── src/
    ├── main.rs            # Application entrypoint & CLI handlers
    ├── cli.rs             # Clap arguments & subcommands
    ├── error.rs           # Thiserror error definitions
    ├── config.rs          # Configuration loader, writer & auto-detection
    └── modem/
        ├── mod.rs         # Orchestrator for Dual SIM, APN profiles & IMS management
        ├── types.rs       # Data models for SIM, bearers, profiles & status
        ├── mmcli.rs       # ModemManager wrapper using native JSON (-J)
        └── qmicli.rs      # QMI wrapper with regex/token parsing
```

---

## CLI Usage

```bash
# View live status of modem, SIM slots, WDS profiles, and IMS registration
sudo dimsd status

# Status in JSON format
sudo dimsd status --json

# Manually disable IMS on Slot 1 (or Slot 2)
sudo dimsd disable -s 1

# Preview disable actions without making changes
sudo dimsd disable -s 1 --dry-run

# Re-enable IMS profiles
sudo dimsd enable -s 1

# Switch active data SIM provisioning session between slots
sudo dimsd switch-slot 2
sudo dimsd switch-slot 1

# List, add, and delete WDS 3GPP APN profiles
sudo dimsd profile list
sudo dimsd profile add customapn --pdp-type IPV4V6
sudo dimsd profile delete 5

# Check hardware topology and sync /etc/dimsd/config.toml
sudo dimsd check --update

# Run as persistent watchdog daemon checking every 10s
sudo dimsd daemon --watch --interval 10
```

---

## Configuration (`/etc/dimsd/config.toml`)

```toml
[modem]
# "auto" dynamically resolves qrtr://0 or /dev/cdc-wdm0 on every run
device = "auto"

# "auto" dynamically discovers the active modem index via mmcli -L
modem_id = "auto"

# Default SIM slot to manage: "1", "2", or "all"
default_slot = "1"

# Cellular network interface (e.g. "rmnet_ipa0" or "auto")
net_interface = "auto"

[daemon]
# Maximum seconds to wait on boot for modem readiness
timeout_secs = 30

# Watchdog interval in seconds
poll_interval_secs = 10

# Auto-update config file when new ports or modem indices are detected
auto_sync_config = true

[logging]
level = "info"
```

---

## Installation & Deployment

### Quick Install (on target device)
```bash
# Clone / copy the dimsd directory to the device, then run:
sudo sh scripts/install.sh
```

### Uninstallation
```bash
# Clean uninstall
sudo sh scripts/uninstall.sh

# Uninstall and restore IMS profiles back to enabled
sudo sh scripts/uninstall.sh --restore --purge
```

---

## Systemd Service

The service runs on boot after ModemManager and QMI proxy are active:
```bash
sudo systemctl status dimsd.service
```

