use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlotCardInfo {
    pub slot: u8,
    pub present: bool,
    pub state_description: String,
    pub has_usim: bool,
    pub has_isim: bool,
    #[serde(default)]
    pub is_primary: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pin1_state: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BearerSummary {
    pub id: String,
    pub dbus_path: String,
    pub apn: String,
    pub connected: bool,
    pub interface: Option<String>,
    pub ip_type: Option<String>,
    pub is_ims: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WdsProfile {
    pub index: u32,
    pub apn: String,
    pub pdp_type: String,
    pub disabled: bool,
    pub is_ims: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImsRegistrationStatus {
    pub status: String,
    pub technology: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImsServiceInfo {
    pub service_name: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModemIdentity {
    pub modem_id: String,
    pub dbus_path: String,
    pub manufacturer: String,
    pub model: String,
    pub primary_port: String,
    pub ports: Vec<String>,
    pub state: String,
    pub sim_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHardwareCheck {
    pub modem_detected: Option<ModemIdentity>,
    pub qmi_device_resolved: String,
    pub net_interface_resolved: Option<String>,
    pub slots: Vec<SlotCardInfo>,
    pub qrtr_available: bool,
    pub config_matches_hardware: bool,
    pub config_path: String,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullStatusReport {
    pub modem: Option<ModemIdentity>,
    pub slots: Vec<SlotCardInfo>,
    pub qmi_device: String,
    pub wds_profiles: Vec<WdsProfile>,
    pub active_bearers: Vec<BearerSummary>,
    pub ims_registration: Option<ImsRegistrationStatus>,
    pub ims_services: Vec<ImsServiceInfo>,
    pub target_slot: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationResult {
    pub action: String,
    pub slot: String,
    pub success: bool,
    pub bearers_disconnected: Vec<String>,
    pub profiles_modified: Vec<u32>,
    pub message: String,
    pub dry_run: bool,
}
