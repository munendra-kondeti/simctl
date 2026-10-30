# Audit & Verification Report: Project Rename to `simctl` & `switch-slot` Inspection

**Date**: 2026-10-30  
**Target Hardware**: Qualcomm Snapdragon MSM8953 (`aarch64`), Nura OS edge (`100.110.148.36`)  
**Scope**: Project renaming from `dimsd` to `simctl`, adding slot inspection and dynamic command recommendation to `switch-slot`, maintaining backwards compatibility aliases, and full verification.

---

## 1. Summary of Changes

1. **Project Rebranding to `simctl`**:
   - `Cargo.toml`: Package name changed to `simctl`.
   - Binary output: `/usr/local/bin/simctl`.
   - Compatibility: Symlink created from `/usr/local/bin/dimsd` to `/usr/local/bin/simctl`.
   - Config directory: `/etc/simctl/config.toml` (with automated fallback to `/etc/dimsd/config.toml`).
   - Systemd Service: `simctl.service` installed with alias `dimsd.service`.
2. **`switch-slot` Inspection & Recommendation Feature**:
   - Running `simctl switch-slot` (or `dimsd switch-slot`) without positional arguments now performs an inspection of both SIM slots.
   - It displays each slot's presence, whether it is `[Primary Active]` or `[Secondary]`, identifies the alternate available slot, and outputs the exact command recommendation (e.g. `simctl switch-slot 2`).
   - Machine-readable `--json` output returns `current_primary`, `slots`, `suggested_slot`, and `suggested_command`.
   - Running `simctl switch-slot <1|2>` switches active data and automatically disables IMS profiles on the newly activated slot.

---

## 2. Review Toolchain Verification

- **`cargo fmt --check`**: PASSED.
- **`cargo clippy --all-targets --all-features -- -W clippy::pedantic`**: **0 warnings**.
- **`cargo test`**: **8/8 unit tests passed**.

---

## 3. Live Target Hardware Verification

- **`dimsd switch-slot` (Inspection)**:
  ```console
  === SIM Slots & Switching Status ===
    Slot [1]  [Primary Active]: PRESENT (USIM: true, ISIM: true) - present
    Slot [2]  [Secondary]     : PRESENT (USIM: true, ISIM: false) - present

  Target slot available for switching: Slot 2
  Suggested command:
    simctl switch-slot 2

  (Add '--no-restart-mm' for an instantaneous switch without restarting ModemManager)
  =====================================
  ```
- **Executing Suggested Command `simctl switch-slot 2 --no-restart-mm`**:
  - Successfully activated Slot 2 and auto-disabled profile 2 on Slot 2.
- **Subsequent `simctl switch-slot` (Dynamic Adaptation)**:
  - Suggested command dynamically adapted to `simctl switch-slot 1`.
- **JSON Output (`simctl switch-slot --json`)**:
  - Valid JSON schema output with `suggested_slot` and `suggested_command`.
