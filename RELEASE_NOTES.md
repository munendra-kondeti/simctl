# Release v0.1.0 — Initial Production Release

Welcome to the first public release of **`simctl`** (`v0.1.0`), a lightweight, production-grade cellular modem and SIM management daemon and CLI built specifically for **Nura OS** (postmarketOS / Alpine Linux `aarch64` on Qualcomm Snapdragon MSM8953).

---

> [!CAUTION]
> ### ⚠️ STRICT EDUCATIONAL & HOBBYIST USE DISCLAIMER — NO SUPPORT & NO WARRANTY
> **THIS SOFTWARE IS PROVIDED AS-IS WITHOUT ANY WARRANTY, SLA, OR TECHNICAL SUPPORT OF ANY KIND.**
>
> 1. **No Warranty**: The authors and contributors provide this code purely for educational and experimental study. There are **NO guarantees** of reliability, merchantability, or fitness for any purpose.
> 2. **Zero Liability**: Neither the authors nor any contributors accept any responsibility or legal liability for damages, device bricking, loss of cellular/emergency connectivity, account suspensions, or telecommunications infractions resulting from using this software.
> 3. **No Support**: There is **NO customer support or maintenance**. Anyone is free to use, fork, study, or adapt this project at their own risk.

---

## 🌟 What's New in v0.1.0

### 1. Dual-SIM Dual Standby (DSDS) Management
- **Automatic Multi-Slot Inspection**: Inspects all present SIM slots (`Slot [1]` and `Slot [2]`) simultaneously via direct QMI UIM queries (`--uim-get-card-status`).
- **Interactive SIM Session Switching (`simctl switch-slot`)**:
  - Run `simctl switch-slot` without arguments to inspect active/secondary slots and receive an immediate suggestion to switch (e.g. `simctl switch-slot 2`).
  - Run `simctl switch-slot <1|2>` to switch the active packet data session (`Primary GW`).
  - Automatically disables IMS profiles on the newly activated slot to maintain radio efficiency and avoid unsolicited registrations.

### 2. Multi-Layer IMS & VoLTE Control
- **True Baseband Profile Disabling**: Directly modifies Qualcomm QMI WDS 3GPP profile tables (`qmicli -d qrtr://0 --wds-modify-profile="3gpp,<index>,disabled=yes"`), preventing modem baseband from initiating IMS SIP REGISTER handshakes.
- **ModemManager Bearer Coordination**: Identifies and disconnects active bearers configured with APN `ims` using native JSON parsing (`mmcli -J`).
- **Zero-Config Default "All Slots" Target**:
  - `simctl disable` and `simctl enable` default to **all present SIM slots** when no slot flag is passed.
  - Specific slot targeting is always supported via `-s 1` or `-s 2`.

### 3. WDS 3GPP Profile Management
- Create, list, and delete custom carrier or private APN profiles directly in modem NVRAM:
  - `simctl profile list`
  - `simctl profile add <apn> [--pdp-type IPV4|IPV6|IPV4V6]`
  - `simctl profile delete <index>`

### 4. Cold-Boot Race Elimination & Systemd Integration
- **`simctl.service` Boot Service**: Runs `ExecStart=/usr/local/bin/simctl disable` on boot.
- **Intelligent Readiness Wait**: Implements an active retry loop (`wait_for_ready`) that polls for both QMI card detection and ModemManager modem discovery before executing changes, eliminating cold-boot initialization races.
- **Built-in Resilience**: Configured with `Restart=on-failure` and `RestartSec=3s`.

### 5. Resilient Auto-Discovery (`"auto"`)
- Dynamically resolves QRTR endpoints (`qrtr://0`), serial QMI devices (`/dev/cdc-wdm0`), and ModemManager indexes (`Modem/0`, `Modem/1`) on every invocation.
- Built-in `simctl check --update` command audits the system and atomically synchronizes `/etc/simctl/config.toml`.

---

## 📦 Release Assets & Binaries

| Asset | Description | Size |
|---|---|---|
| `simctl` | Standalone stripped `aarch64` static executable | 1.9 MB |
| `simctl-v0.1.0-aarch64-linux-musl.tar.gz` | Complete release bundle with installer | ~908 KB |
| `SHA256SUMS` | Cryptographic integrity verification file | Text |

### SHA256 Checksums
```text
9ae359f7aa4ce0e18a0658026549f87441702f39f85c5b3734bfa3eb6b9e6155  simctl
8f32fe403565f78884b3d2a700c874ab2012b0683712a5604d7b250e6bf576af  simctl-v0.1.0-aarch64-linux-musl.tar.gz
```

---

## 🚀 Quick Install on Device

```bash
# Extract bundle and run installer
tar -xzf simctl-v0.1.0-aarch64-linux-musl.tar.gz
sudo sh scripts/install.sh

# Verify live status
sudo simctl status
```
