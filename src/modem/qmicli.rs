use crate::error::{DimsdError, Result};
use crate::modem::types::{ImsRegistrationStatus, ImsServiceInfo, SlotCardInfo, WdsProfile};
use once_cell::sync::Lazy;
use regex::Regex;
use std::process::Command;
use tracing::debug;

#[allow(clippy::non_std_lazy_statics)]
static SLOT_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"Slot\s*\[(\d+)\]:").expect("valid regex"));
#[allow(clippy::non_std_lazy_statics)]
static CARD_STATE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"Card\s+state:\s*'([^']+)'").expect("valid regex"));
#[allow(clippy::non_std_lazy_statics)]
static APP_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"Application\s+type:\s*'([^']+)'").expect("valid regex"));
#[allow(clippy::non_std_lazy_statics)]
static PIN1_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"PIN1\s+state:\s*'([^']+)'").expect("valid regex"));
#[allow(clippy::non_std_lazy_statics)]
static PRIMARY_GW_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"Primary GW:\s*slot '(\d+)'").expect("valid regex"));

#[allow(clippy::non_std_lazy_statics)]
static PROFILE_START_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\s*\[(\d+)\]\s*3gpp").expect("valid regex"));
#[allow(clippy::non_std_lazy_statics)]
static APN_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"APN:\s*'([^']*)'").expect("valid regex"));
#[allow(clippy::non_std_lazy_statics)]
static PDP_TYPE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"PDP\s+type:\s*'([^']*)'").expect("valid regex"));
#[allow(clippy::non_std_lazy_statics)]
static DISABLED_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"APN\s+disabled:\s*'([^']*)'").expect("valid regex"));

#[allow(clippy::non_std_lazy_statics)]
static STATUS_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"Status:\s*'([^']+)'").expect("valid regex"));
#[allow(clippy::non_std_lazy_statics)]
static TECH_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"Technology:\s*'([^']+)'").expect("valid regex"));
#[allow(clippy::non_std_lazy_statics)]
static SERVICE_NAME_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\s*([A-Za-z0-9 ]+service)\s*$").expect("valid regex"));

pub struct Qmicli {
    pub qmicli_path: String,
}

impl Default for Qmicli {
    fn default() -> Self {
        Self {
            qmicli_path: "qmicli".to_string(),
        }
    }
}

impl Qmicli {
    pub fn new() -> Self {
        Self::default()
    }

    /// Runs qmicli command with device flag (-p -d <device>)
    fn run_device(&self, device: &str, args: &[&str]) -> Result<String> {
        let mut full_args = vec!["-p", "-d", device];
        full_args.extend_from_slice(args);

        debug!("Executing: {} {}", self.qmicli_path, full_args.join(" "));
        let output = Command::new(&self.qmicli_path)
            .args(&full_args)
            .output()
            .map_err(|e| DimsdError::CommandFailed {
                command: format!("{} {}", self.qmicli_path, full_args.join(" ")),
                message: e.to_string(),
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() {
            let combined = format!("{}\n{}", stdout.trim(), stderr.trim());
            return Err(DimsdError::QmiError {
                device: device.to_string(),
                message: combined.trim().to_string(),
            });
        }

        Ok(stdout)
    }

    /// Queries SIM slot status via --uim-get-card-status
    pub fn get_card_status(&self, device: &str) -> Result<Vec<SlotCardInfo>> {
        let stdout = self.run_device(device, &["--uim-get-card-status"])?;
        parse_uim_card_status(&stdout)
    }

    /// Lists 3GPP profiles via --wds-get-profile-list=3gpp
    pub fn get_wds_profiles(&self, device: &str) -> Result<Vec<WdsProfile>> {
        let stdout = self.run_device(device, &["--wds-get-profile-list=3gpp"])?;
        parse_wds_profiles(&stdout)
    }

    /// Disables or enables a 3GPP profile at index
    pub fn set_profile_disabled(&self, device: &str, index: u32, disabled: bool) -> Result<()> {
        let flag_val = if disabled { "yes" } else { "no" };
        let modify_arg = format!("--wds-modify-profile=3gpp,{index},disabled={flag_val}");
        debug!("Modifying profile: {}", modify_arg);
        let _ = self.run_device(device, &[&modify_arg])?;
        Ok(())
    }

    /// Queries IMS registration status via --imsa-get-ims-registration-status
    pub fn get_ims_registration(&self, device: &str) -> Result<ImsRegistrationStatus> {
        let stdout = self.run_device(device, &["--imsa-get-ims-registration-status"])?;
        parse_ims_registration(&stdout)
    }

    /// Queries IMS services status via --imsa-get-ims-services-status
    pub fn get_ims_services(&self, device: &str) -> Result<Vec<ImsServiceInfo>> {
        let stdout = self.run_device(device, &["--imsa-get-ims-services-status"])?;
        parse_ims_services(&stdout)
    }
}

#[allow(clippy::similar_names)]
pub fn parse_uim_card_status(output: &str) -> Result<Vec<SlotCardInfo>> {
    let mut slots = Vec::new();

    let lines: Vec<&str> = output.lines().collect();
    let mut primary_slot: Option<u8> = None;
    for line in &lines {
        if let Some(cap) = PRIMARY_GW_RE.captures(line) {
            if let Some(m) = cap.get(1) {
                primary_slot = m.as_str().parse::<u8>().ok();
            }
        }
    }

    let mut current_slot: Option<u8> = None;
    let mut current_state = String::new();
    let mut has_usim = false;
    let mut has_isim = false;
    let mut pin1_state = None;

    for line in lines {
        if let Some(cap) = SLOT_RE.captures(line) {
            if let Some(s) = current_slot {
                let present = current_state.eq_ignore_ascii_case("present");
                let is_primary = primary_slot.is_some_and(|p| p == s);
                slots.push(SlotCardInfo {
                    slot: s,
                    present,
                    state_description: current_state.clone(),
                    has_usim,
                    has_isim,
                    is_primary,
                    pin1_state,
                });
            }
            current_slot = cap.get(1).and_then(|m| m.as_str().parse::<u8>().ok());
            current_state = "unknown".to_string();
            has_usim = false;
            has_isim = false;
            pin1_state = None;
            continue;
        }

        if current_slot.is_some() {
            if let Some(cap) = CARD_STATE_RE.captures(line) {
                if let Some(m) = cap.get(1) {
                    current_state = m.as_str().to_string();
                }
            }
            if let Some(cap) = APP_RE.captures(line) {
                if let Some(m) = cap.get(1) {
                    let app_str = m.as_str().to_lowercase();
                    if app_str.contains("usim") {
                        has_usim = true;
                    }
                    if app_str.contains("isim") {
                        has_isim = true;
                    }
                }
            }
            if let Some(cap) = PIN1_RE.captures(line) {
                if pin1_state.is_none() {
                    if let Some(m) = cap.get(1) {
                        pin1_state = Some(m.as_str().to_string());
                    }
                }
            }
        }
    }

    if let Some(s) = current_slot {
        let present = current_state.eq_ignore_ascii_case("present");
        let is_primary = primary_slot.is_some_and(|p| p == s);
        slots.push(SlotCardInfo {
            slot: s,
            present,
            state_description: current_state,
            has_usim,
            has_isim,
            is_primary,
            pin1_state,
        });
    }

    Ok(slots)
}

pub fn parse_wds_profiles(output: &str) -> Result<Vec<WdsProfile>> {
    let mut profiles = Vec::new();

    let mut current_idx: Option<u32> = None;
    let mut current_apn = String::new();
    let mut current_pdp = String::new();
    let mut current_disabled = false;

    for line in output.lines() {
        if let Some(cap) = PROFILE_START_RE.captures(line) {
            if let Some(idx) = current_idx {
                let is_ims = current_apn.eq_ignore_ascii_case("ims")
                    || current_apn.to_ascii_lowercase().contains("ims");
                profiles.push(WdsProfile {
                    index: idx,
                    apn: current_apn.clone(),
                    pdp_type: current_pdp.clone(),
                    disabled: current_disabled,
                    is_ims,
                });
            }
            current_idx = cap.get(1).and_then(|m| m.as_str().parse::<u32>().ok());
            current_apn.clear();
            current_pdp.clear();
            current_disabled = false;
            continue;
        }

        if current_idx.is_some() {
            if let Some(cap) = APN_RE.captures(line) {
                if let Some(m) = cap.get(1) {
                    current_apn = m.as_str().to_string();
                }
            }
            if let Some(cap) = PDP_TYPE_RE.captures(line) {
                if let Some(m) = cap.get(1) {
                    current_pdp = m.as_str().to_string();
                }
            }
            if let Some(cap) = DISABLED_RE.captures(line) {
                if let Some(m) = cap.get(1) {
                    current_disabled = m.as_str().eq_ignore_ascii_case("yes");
                }
            }
        }
    }

    if let Some(idx) = current_idx {
        let is_ims = current_apn.eq_ignore_ascii_case("ims")
            || current_apn.to_ascii_lowercase().contains("ims");
        profiles.push(WdsProfile {
            index: idx,
            apn: current_apn,
            pdp_type: current_pdp,
            disabled: current_disabled,
            is_ims,
        });
    }

    Ok(profiles)
}

pub fn parse_ims_registration(output: &str) -> Result<ImsRegistrationStatus> {
    let mut status = "unknown".to_string();
    let mut technology = None;

    for line in output.lines() {
        if let Some(cap) = STATUS_RE.captures(line) {
            if let Some(m) = cap.get(1) {
                status = m.as_str().to_string();
            }
        }
        if let Some(cap) = TECH_RE.captures(line) {
            if let Some(m) = cap.get(1) {
                technology = Some(m.as_str().to_string());
            }
        }
    }

    Ok(ImsRegistrationStatus { status, technology })
}

pub fn parse_ims_services(output: &str) -> Result<Vec<ImsServiceInfo>> {
    let mut services = Vec::new();
    let mut current_service = String::new();

    for line in output.lines() {
        if let Some(cap) = SERVICE_NAME_RE.captures(line) {
            if let Some(m) = cap.get(1) {
                current_service = m.as_str().trim().to_string();
            }
            continue;
        }

        if !current_service.is_empty() {
            if let Some(cap) = STATUS_RE.captures(line) {
                if let Some(m) = cap.get(1) {
                    services.push(ImsServiceInfo {
                        service_name: current_service.clone(),
                        status: m.as_str().to_string(),
                    });
                    current_service.clear();
                }
            }
        }
    }

    Ok(services)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_uim_card_status() {
        let sample = r"
[qrtr://0] Successfully got card status
Provisioning applications:
	Primary GW:   slot '1', application '1'
	Primary 1X:   session doesn't exist
Slot [1]:
	Card state: 'present'
	UPIN state: 'not-initialized'
	Application [1]:
		Application type:  'usim (2)'
		Application state: 'ready'
	Application [2]:
		Application type:  'isim (5)'
Slot [2]:
	Card state: 'error: no-atr-received (3)'
";
        let slots = parse_uim_card_status(sample).unwrap();
        assert_eq!(slots.len(), 2);
        assert_eq!(slots[0].slot, 1);
        assert!(slots[0].present);
        assert!(slots[0].has_usim);
        assert!(slots[0].has_isim);
        assert_eq!(slots[1].slot, 2);
        assert!(!slots[1].present);
    }

    #[test]
    fn test_parse_wds_profiles() {
        let sample = r"
Profile list retrieved:
	[1] 3gpp - 
		APN: 'jionet'
		PDP type: 'ipv4-or-ipv6'
		APN disabled: 'no'
	[2] 3gpp - 
		APN: 'ims'
		PDP type: 'ipv4-or-ipv6'
		APN disabled: 'no'
";
        let profiles = parse_wds_profiles(sample).unwrap();
        assert_eq!(profiles.len(), 2);
        assert_eq!(profiles[0].index, 1);
        assert!(!profiles[0].is_ims);
        assert_eq!(profiles[1].index, 2);
        assert!(profiles[1].is_ims);
        assert!(!profiles[1].disabled);
    }
}
