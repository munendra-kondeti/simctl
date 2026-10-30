# Post-Review & Production Deployment Fixes

## Overview
This document records the observations, edge-case bug fixes, and architectural adjustments made during and following the `/rust-best-practices` and `/rust-review` audits, specifically identified when running and verifying the binary directly on the target hardware (Qualcomm MSM8953 / Nura OS / Alpine Linux `aarch64`).

---

## 1. Disconnection Race Condition Fix (`src/modem/mod.rs`)

### Issue Discovered:
During initial deployment on the live device, `dimsd disable` successfully disconnected ModemManager Bearer 1 (`/org/freedesktop/ModemManager1/Bearer/1`) and disabled WDS Profile 2 (`APN: 'ims'`). However, inspecting the live state revealed that ModemManager reconnected Bearer 1 within ~1 second:
```
Status: connected: yes
Statistics | start date: 2026-10-30T05:54:53Z (1s after dimsd ran)
```

### Root Cause:
In the original implementation:
1. `dimsd` sent `mmcli -b <id> --disconnect` to ModemManager.
2. ModemManager disconnected the bearer, but because the underlying QMI WDS profile was still `ACTIVE`, ModemManager's auto-connect logic immediately renegotiated the bearer.
3. Only afterwards did `dimsd` disable the QMI profile. By that time, the active connection was already re-established in kernel space.

### Fix Applied:
Inverted the execution order in `ModemManager::disable_ims`:
1. **First**: Disable the WDS 3GPP profile in QMI via `qmicli -p -d qrtr://0 --wds-modify-profile=3gpp,<idx>,disabled=yes`.
2. **Second**: Disconnect any active ModemManager IMS bearers (`mmcli -b <id> --disconnect`).

### Verification & Result:
- When ModemManager attempts to auto-reconnect after bearer disconnect, the Qualcomm baseband modem rejects the request because the profile is disabled (`pdn-ipv6-call-throttled`).
- Live verification confirmed:
  - `Bearer 1 | APN: 'ims' | Dev: -- | Connected: false [IMS BEARER]`
  - IMS subsystem: `Registration: not-registered`, `Voice: unavailable`, `SMS: unavailable`.
  - Normal internet connection (`jionet` / APN 1) remains 100% operational (pinging 1.1.1.1 with ~9-11 ms latency).

---

## 2. Dev-First Deployment Rule

Per deployment guidelines:
- Debug/dev binary was built first inside `rust:alpine` (aarch64) and tested on the target hardware before any release artifacts were produced.
- All 8 unit tests passed on host and container.
- Zero Clippy warnings under strict `-D warnings` and pedantic mode.
