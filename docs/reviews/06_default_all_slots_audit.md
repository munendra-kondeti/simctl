# Audit & Verification Report: Default All-Slots Improvement

**Date**: 2026-10-30  
**Target Hardware**: Qualcomm Snapdragon MSM8953 (`aarch64`), Nura OS edge (`<target-device-ip>`)  
**Scope**: Making all general commands default to "All Slots" unless a specific slot is targeted, answering the user's architectural question on config vs code changes, and verifying zero Clippy warnings.

---

## 1. Architectural Answer: Config vs Code Change

**Question**: *Is it simply changing config, or is there a code change?*

**Answer**: **Both were required, and a code change was essential for three reasons:**
1. **Zero-Config Fallback**: Without a code change, if `/etc/dimsd/config.toml` was absent or did not specify `default_slot`, compiled Rust defaults fell back to `"1"`. Changing `fn default_slot() -> "all"` in `src/config.rs` makes "All Slots" the global out-of-the-box standard.
2. **CLI Contract**: Clap's CLI documentation (`dimsd --help`) displayed `(default: 1)`. It was updated to `(default: all)`.
3. **Qualcomm DSDS NVRAM Profile Lifecycle**:
   - In Qualcomm modem firmware, switching the UIM provisioning session (`--uim-change-provisioning-session`) dynamically reloads the 3GPP profile table from that SIM card's Elementary Files (EF).
   - In DSDS mode, only the **active `Primary GW` slot** has an active packet data context and can attempt IMS attachment. The secondary SIM is on standby for CS/PS paging and has no routable packet data.
   - To make dual-SIM management completely seamless, `switch_slot` now automatically disables IMS profiles on the newly activated slot upon switching, ensuring IMS never attaches regardless of which SIM is activated.
   - All general commands (`status`, `disable`, `enable`, `daemon`, default boot action) now resolve through `config.resolve_slot(cli.slot)` which defaults to `All Slots`.

---

## 2. Review Findings & Fixes

1. **Dead Code Elimination**:
   - Clippy warning `method as_u8 is never used` in `src/cli.rs:56:12`.
   - Removed unused method `SlotSelection::as_u8`.
2. **Code Formatting**:
   - Cleaned up line wrapping in `src/modem/mod.rs` to satisfy `cargo fmt --check`.
3. **Verification Toolchain**:
   - `cargo fmt --check`: **PASSED**.
   - `cargo clippy --all-targets --all-features -- -W clippy::pedantic`: **0 warnings**.
   - `cargo test`: **8/8 unit tests passed**.

---

## 3. Live Target Hardware Verification

- **Command**: `dimsd disable` (no slot flag)
  - Result: `=== Operation: DISABLE All Slots ===`
  - Status: `SUCCESS`
- **Command**: `dimsd switch-slot 2 --no-restart-mm`
  - Output: `INFO Automatically disabled IMS profiles on newly activated Slot 2: [2]`
  - Status: Slot 2 became Primary Active and profile 2 was disabled automatically.
- **Command**: `dimsd switch-slot 1 --no-restart-mm`
  - Output: `INFO Automatically disabled IMS profiles on newly activated Slot 1: [2]`
  - Status: Slot 1 restored as Primary Active and profile 2 remained disabled.
- **Command**: `dimsd status` (no slot flag)
  - Result: Defaults to `Target Selection: All Slots (Dual SIM)`, displaying both Slot 1 and Slot 2.
