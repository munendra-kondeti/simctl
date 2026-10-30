pub mod mmcli;
pub mod qmicli;
pub mod types;

use crate::cli::SlotSelection;
use crate::config::Config;
use crate::error::{DimsdError, Result};
use mmcli::Mmcli;
use qmicli::Qmicli;
use std::fs;
use std::path::Path;
use std::thread::sleep;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};
use types::{
    FullStatusReport, OperationResult, ProfileOperationResult, SwitchSlotResult,
    SystemHardwareCheck,
};

pub struct ModemManager {
    mmcli: Mmcli,
    qmicli: Qmicli,
}

impl Default for ModemManager {
    fn default() -> Self {
        Self {
            mmcli: Mmcli::new(),
            qmicli: Qmicli::new(),
        }
    }
}

impl ModemManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Resolves target QMI device string ("auto" -> `<qrtr://0>` or fallback)
    #[allow(clippy::unused_self)]
    pub fn resolve_device(&self, configured: &str) -> String {
        if configured != "auto" && !configured.is_empty() {
            return configured.to_string();
        }

        // On Qualcomm msm8953 / Nura OS, qrtr://0 is the default high-performance socket
        if Path::new("/sys/bus/qrtr").exists() || Path::new("/dev/qrtr-radv").exists() {
            return "qrtr://0".to_string();
        }

        // Fallback to /dev/cdc-wdm0 if present
        if Path::new("/dev/cdc-wdm0").exists() {
            return "/dev/cdc-wdm0".to_string();
        }

        "qrtr://0".to_string()
    }

    /// Resolves target `ModemManager` modem id ("auto" -> discovered index, with auto-fallback)
    pub fn resolve_modem_id(&self, configured: &str) -> Result<String> {
        if configured != "auto" && !configured.is_empty() {
            if self.mmcli.get_modem_identity(configured).is_ok() {
                return Ok(configured.to_string());
            }
            debug!(
                "Configured modem_id '{}' not active in ModemManager, falling back to discovery",
                configured
            );
        }

        self.mmcli.discover_modem_id()
    }

    /// Waits for modem and SIM readiness on boot up to `timeout_secs`
    pub fn wait_for_ready(&self, modem_id: &str, device: &str, timeout_secs: u64) {
        let start = Instant::now();
        let timeout = Duration::from_secs(timeout_secs);

        debug!(
            "Waiting for modem {} and QMI device {} to be ready (timeout: {}s)...",
            modem_id, device, timeout_secs
        );

        while start.elapsed() < timeout {
            // Check if card status can be read
            if let Ok(slots) = self.qmicli.get_card_status(device) {
                if slots.iter().any(|s| s.present) {
                    debug!("Modem card status is ready, slots: {:?}", slots);
                    return;
                }
            }

            sleep(Duration::from_millis(1000));
        }

        warn!(
            "Modem readiness timed out after {}s, proceeding anyway",
            timeout_secs
        );
    }

    /// Performs complete system hardware and configuration check
    pub fn check_system(&self, config: &Config, config_path: &Path) -> SystemHardwareCheck {
        let qmi_device = self.resolve_device(&config.modem.device);
        let modem_id = self.resolve_modem_id(&config.modem.modem_id).ok();

        let modem_identity = match &modem_id {
            Some(id) => self.mmcli.get_modem_identity(id).ok(),
            None => None,
        };

        let slots = self.qmicli.get_card_status(&qmi_device).unwrap_or_default();

        let qrtr_available =
            Path::new("/sys/bus/qrtr").exists() || Path::new("/dev/qrtr-radv").exists();

        let net_interface_resolved = modem_identity.as_ref().and_then(|m| {
            m.ports
                .iter()
                .find(|p| p.contains("net") || p.contains("rmnet"))
                .map(|p| p.split_whitespace().next().unwrap_or(p).to_string())
        });

        let mut notes = Vec::new();

        // Check slots
        for s in &slots {
            if s.present {
                notes.push(format!(
                    "SIM Slot {}: PRESENT (USIM: {}, ISIM: {})",
                    s.slot, s.has_usim, s.has_isim
                ));
            } else {
                notes.push(format!("SIM Slot {}: EMPTY / NOT READY", s.slot));
            }
        }

        // Compare config with detected hardware
        let mut matches = true;
        if config.modem.device != "auto" && config.modem.device != qmi_device {
            matches = false;
            notes.push(format!(
                "Config device '{}' does not match detected '{}'",
                config.modem.device, qmi_device
            ));
        }

        if let Some(id) = &modem_id {
            if config.modem.modem_id != "auto" && config.modem.modem_id != *id {
                matches = false;
                notes.push(format!(
                    "Config modem_id '{}' does not match detected '{}'",
                    config.modem.modem_id, id
                ));
            }
        }

        SystemHardwareCheck {
            modem_detected: modem_identity,
            qmi_device_resolved: qmi_device,
            net_interface_resolved,
            slots,
            qrtr_available,
            config_matches_hardware: matches,
            config_path: config_path.display().to_string(),
            notes,
        }
    }

    /// Queries complete status report for display or JSON output
    pub fn get_status(&self, config: &Config, slot_sel: SlotSelection) -> FullStatusReport {
        let qmi_device = self.resolve_device(&config.modem.device);
        let modem_id = self.resolve_modem_id(&config.modem.modem_id).ok();

        let modem_identity = match &modem_id {
            Some(id) => self.mmcli.get_modem_identity(id).ok(),
            None => None,
        };

        let slots = self.qmicli.get_card_status(&qmi_device).unwrap_or_default();

        let wds_profiles = self
            .qmicli
            .get_wds_profiles(&qmi_device)
            .unwrap_or_default();

        let active_bearers = match &modem_id {
            Some(id) => self.mmcli.get_bearers(id).unwrap_or_default(),
            None => Vec::new(),
        };

        let ims_registration = self.qmicli.get_ims_registration(&qmi_device).ok();
        let ims_services = self
            .qmicli
            .get_ims_services(&qmi_device)
            .unwrap_or_default();

        let target_str = match slot_sel {
            SlotSelection::Slot1 => "Slot 1 (Single SIM)".to_string(),
            SlotSelection::Slot2 => "Slot 2 (Single SIM)".to_string(),
            SlotSelection::All => "All Slots (Dual SIM)".to_string(),
        };

        FullStatusReport {
            modem: modem_identity,
            slots,
            qmi_device,
            wds_profiles,
            active_bearers,
            ims_registration,
            ims_services,
            target_slot: target_str,
        }
    }

    fn disable_ims_profiles_on_active_session(
        &self,
        qmi_device: &str,
        dry_run: bool,
    ) -> Result<Vec<u32>> {
        let mut modified = Vec::new();
        let profiles = self.qmicli.get_wds_profiles(qmi_device)?;
        for p in &profiles {
            if p.is_ims && !p.disabled {
                if dry_run {
                    info!(
                        "[DRY RUN] Would disable WDS profile {} (APN: '{}')",
                        p.index, p.apn
                    );
                    modified.push(p.index);
                } else {
                    info!("Disabling WDS profile {} (APN: '{}')", p.index, p.apn);
                    if let Err(e) = self.qmicli.set_profile_disabled(qmi_device, p.index, true) {
                        warn!("Failed to disable profile {}: {}", p.index, e);
                    } else {
                        modified.push(p.index);
                    }
                }
            }
        }
        Ok(modified)
    }

    fn enable_ims_profiles_on_active_session(
        &self,
        qmi_device: &str,
        dry_run: bool,
    ) -> Result<Vec<u32>> {
        let mut modified = Vec::new();
        let profiles = self.qmicli.get_wds_profiles(qmi_device)?;
        for p in &profiles {
            if p.is_ims {
                if dry_run {
                    info!(
                        "[DRY RUN] Would enable WDS profile {} (APN: '{}')",
                        p.index, p.apn
                    );
                    modified.push(p.index);
                } else {
                    info!("Enabling WDS profile {} (APN: '{}')", p.index, p.apn);
                    if let Err(e) = self.qmicli.set_profile_disabled(qmi_device, p.index, false) {
                        warn!("Failed to enable profile {}: {}", p.index, e);
                    } else {
                        modified.push(p.index);
                    }
                }
            }
        }
        Ok(modified)
    }

    /// Disables IMS bearers, profiles, and autoconnect across targeted or all present SIM slots
    pub fn disable_ims(
        &self,
        config: &Config,
        slot_sel: SlotSelection,
        dry_run: bool,
    ) -> Result<OperationResult> {
        let qmi_device = self.resolve_device(&config.modem.device);
        let modem_id = self.resolve_modem_id(&config.modem.modem_id)?;

        let mut disconnected = Vec::new();
        let mut modified_profiles = Vec::new();

        let slots = self.qmicli.get_card_status(&qmi_device).unwrap_or_default();
        let current_primary = slots.iter().find(|s| s.is_primary).map(|s| s.slot);

        let target_slot = match slot_sel {
            SlotSelection::Slot1 => Some(1),
            SlotSelection::Slot2 => Some(2),
            SlotSelection::All => None,
        };

        if let Some(target) = target_slot {
            if current_primary != Some(target) {
                if let Some(target_card) = slots.iter().find(|s| s.slot == target) {
                    if !target_card.present {
                        info!("Target SIM slot {} is not present, skipping.", target);
                    } else if let Some(aid) = &target_card.usim_aid {
                        info!("Activating Slot {} to configure IMS profiles...", target);
                        self.qmicli
                            .change_provisioning_session(&qmi_device, target, aid)?;
                    }
                }
            }
        }

        let modified = self.disable_ims_profiles_on_active_session(&qmi_device, dry_run)?;
        modified_profiles.extend(modified);

        // Disconnect active IMS bearers in ModemManager AFTER profile is disabled
        let bearers = self.mmcli.get_bearers(&modem_id)?;
        for b in &bearers {
            if b.is_ims && b.connected {
                if dry_run {
                    info!("[DRY RUN] Would disconnect IMS bearer {}", b.id);
                    disconnected.push(b.id.clone());
                } else {
                    info!("Disconnecting IMS bearer {}", b.id);
                    if let Err(e) = self.mmcli.disconnect_bearer(&b.id) {
                        warn!("Failed to disconnect bearer {}: {}", b.id, e);
                    } else {
                        disconnected.push(b.id.clone());
                    }
                }
            }
        }

        // Backup state for restoration
        if !dry_run {
            Self::save_backup_state(&modified_profiles);
        }

        let slot_label = match slot_sel {
            SlotSelection::Slot1 => "Slot 1".to_string(),
            SlotSelection::Slot2 => "Slot 2".to_string(),
            SlotSelection::All => "All Slots".to_string(),
        };

        Ok(OperationResult {
            action: "disable".to_string(),
            slot: slot_label,
            success: true,
            bearers_disconnected: disconnected,
            profiles_modified: modified_profiles,
            message: "IMS disabled successfully".to_string(),
            dry_run,
        })
    }

    /// Re-enables IMS profiles across targeted or all present SIM slots
    pub fn enable_ims(
        &self,
        config: &Config,
        slot_sel: SlotSelection,
        dry_run: bool,
    ) -> Result<OperationResult> {
        let qmi_device = self.resolve_device(&config.modem.device);
        let mut modified_profiles = Vec::new();

        let slots = self.qmicli.get_card_status(&qmi_device).unwrap_or_default();
        let current_primary = slots.iter().find(|s| s.is_primary).map(|s| s.slot);

        let target_slot = match slot_sel {
            SlotSelection::Slot1 => Some(1),
            SlotSelection::Slot2 => Some(2),
            SlotSelection::All => None,
        };

        if let Some(target) = target_slot {
            if current_primary != Some(target) {
                if let Some(target_card) = slots.iter().find(|s| s.slot == target) {
                    if !target_card.present {
                        info!("Target SIM slot {} is not present, skipping.", target);
                    } else if let Some(aid) = &target_card.usim_aid {
                        info!("Activating Slot {} to configure IMS profiles...", target);
                        self.qmicli
                            .change_provisioning_session(&qmi_device, target, aid)?;
                    }
                }
            }
        }

        let modified = self.enable_ims_profiles_on_active_session(&qmi_device, dry_run)?;
        modified_profiles.extend(modified);

        let slot_label = match slot_sel {
            SlotSelection::Slot1 => "Slot 1".to_string(),
            SlotSelection::Slot2 => "Slot 2".to_string(),
            SlotSelection::All => "All Slots".to_string(),
        };

        Ok(OperationResult {
            action: "enable".to_string(),
            slot: slot_label,
            success: true,
            bearers_disconnected: Vec::new(),
            profiles_modified: modified_profiles,
            message: "IMS enabled successfully".to_string(),
            dry_run,
        })
    }

    /// Switches the active primary cellular provisioning session to the target SIM slot (1 or 2)
    pub fn switch_slot(
        &self,
        config: &Config,
        target_slot: u8,
        restart_mm: bool,
    ) -> Result<SwitchSlotResult> {
        let qmi_device = self.resolve_device(&config.modem.device);
        let slots = self.qmicli.get_card_status(&qmi_device)?;

        let current_primary = slots.iter().find(|s| s.is_primary).map(|s| s.slot);
        let target_info = slots
            .iter()
            .find(|s| s.slot == target_slot)
            .ok_or(DimsdError::SlotNotPresent(target_slot))?;

        if !target_info.present {
            return Err(DimsdError::CommandFailed {
                command: format!("switch-slot {target_slot}"),
                message: format!("SIM Slot {target_slot} is empty; cannot activate an empty slot"),
            });
        }

        let aid = target_info
            .usim_aid
            .as_deref()
            .ok_or_else(|| DimsdError::CommandFailed {
                command: format!("switch-slot {target_slot}"),
                message: format!("No valid USIM Application ID detected on Slot {target_slot}"),
            })?;

        info!(
            "Switching Primary GW provisioning session to Slot {} (AID: {})...",
            target_slot, aid
        );

        self.qmicli
            .change_provisioning_session(&qmi_device, target_slot, aid)?;

        // Automatically ensure IMS profiles are disabled on the newly activated slot
        if let Ok(disabled) = self.disable_ims_profiles_on_active_session(&qmi_device, false) {
            if !disabled.is_empty() {
                info!(
                    "Automatically disabled IMS profiles on newly activated Slot {}: {:?}",
                    target_slot, disabled
                );
            }
        }

        if restart_mm {
            info!("Restarting ModemManager to re-probe active SIM subscription...");
            let _ = std::process::Command::new("systemctl")
                .args(["restart", "ModemManager"])
                .status();
        }

        Ok(SwitchSlotResult {
            previous_slot: current_primary,
            new_slot: target_slot,
            success: true,
            message: format!(
                "Successfully switched active SIM provisioning session to Slot {target_slot}"
            ),
        })
    }

    /// Adds a new WDS 3GPP profile
    pub fn create_profile(
        &self,
        config: &Config,
        apn: &str,
        pdp_type: &str,
        disabled: bool,
    ) -> Result<ProfileOperationResult> {
        let qmi_device = self.resolve_device(&config.modem.device);
        let new_index = self
            .qmicli
            .create_wds_profile(&qmi_device, apn, pdp_type, disabled)?;

        Ok(ProfileOperationResult {
            action: "create".to_string(),
            index: Some(new_index),
            apn: Some(apn.to_string()),
            success: true,
            message: format!("WDS profile created at index {new_index} for APN '{apn}'"),
        })
    }

    /// Deletes an existing WDS 3GPP profile by index
    pub fn delete_profile(&self, config: &Config, index: u32) -> Result<ProfileOperationResult> {
        let qmi_device = self.resolve_device(&config.modem.device);
        self.qmicli.delete_wds_profile(&qmi_device, index)?;

        Ok(ProfileOperationResult {
            action: "delete".to_string(),
            index: Some(index),
            apn: None,
            success: true,
            message: format!("WDS profile {index} successfully deleted"),
        })
    }

    /// Saves state to /var/lib/dimsd/state.json for precise restoration
    fn save_backup_state(modified_profiles: &[u32]) {
        let state_dir = Path::new("/var/lib/dimsd");
        if let Err(e) = fs::create_dir_all(state_dir) {
            debug!("Cannot create /var/lib/dimsd: {}", e);
            return;
        }

        let state_file = state_dir.join("state.json");
        let json_val = serde_json::json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "disabled_profiles": modified_profiles,
        });

        if let Ok(content) = serde_json::to_string_pretty(&json_val) {
            let _ = fs::write(state_file, content);
        }
    }
}
