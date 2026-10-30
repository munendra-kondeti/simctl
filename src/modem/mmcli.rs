use crate::error::{DimsdError, Result};
use crate::modem::types::{BearerSummary, ModemIdentity};
use serde_json::Value;
use std::process::Command;
use tracing::{debug, warn};

pub struct Mmcli {
    pub mmcli_path: String,
}

impl Default for Mmcli {
    fn default() -> Self {
        Self {
            mmcli_path: "mmcli".to_string(),
        }
    }
}

impl Mmcli {
    pub fn new() -> Self {
        Self::default()
    }

    /// Executes mmcli command and returns stdout
    fn run_args(&self, args: &[&str]) -> Result<String> {
        debug!("Executing: {} {}", self.mmcli_path, args.join(" "));
        let output = Command::new(&self.mmcli_path)
            .args(args)
            .output()
            .map_err(|e| DimsdError::CommandFailed {
                command: format!("{} {}", self.mmcli_path, args.join(" ")),
                message: e.to_string(),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(DimsdError::CommandFailed {
                command: format!("{} {}", self.mmcli_path, args.join(" ")),
                message: stderr.trim().to_string(),
            });
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Discovers the active `ModemManager` modem index
    pub fn discover_modem_id(&self) -> Result<String> {
        let stdout = self.run_args(&["-L", "-J"])?;
        if let Some(id) = parse_modem_id_from_json(&stdout) {
            return Ok(id);
        }

        // Fallback to plain mmcli -L
        let plain_out = self.run_args(&["-L"])?;
        for line in plain_out.lines() {
            if line.contains("/Modem/") {
                if let Some(idx_start) = line.find("/Modem/") {
                    let sub = &line[idx_start + 7..];
                    let id: String = sub.chars().take_while(char::is_ascii_digit).collect();
                    if !id.is_empty() {
                        return Ok(id);
                    }
                }
            }
        }

        Err(DimsdError::ModemNotFound(
            "No active modem discovered via mmcli -L".to_string(),
        ))
    }

    /// Queries modem details using mmcli -J -m <id>
    pub fn get_modem_identity(&self, modem_id: &str) -> Result<ModemIdentity> {
        let stdout = self.run_args(&["-J", "-m", modem_id])?;
        let parsed: Value = serde_json::from_str(&stdout)?;

        let modem = parsed.get("modem").ok_or_else(|| {
            DimsdError::ModemNotFound(format!("No 'modem' key in mmcli json for modem {modem_id}"))
        })?;

        let generic = modem.get("generic").cloned().unwrap_or(Value::Null);
        let manufacturer = generic
            .get("manufacturer")
            .and_then(|v| v.as_str())
            .unwrap_or("Qualcomm")
            .to_string();
        let device_model = generic
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("0")
            .to_string();
        let primary_port = generic
            .get("primary-port")
            .and_then(|v| v.as_str())
            .unwrap_or("qrtr0")
            .to_string();
        let state = generic
            .get("state")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let dbus_path = modem
            .get("dbus-path")
            .and_then(|v| v.as_str())
            .unwrap_or("/org/freedesktop/ModemManager1/Modem/0")
            .to_string();
        let sim_path = generic
            .get("sim")
            .and_then(|v| v.as_str())
            .map(ToString::to_string);

        let mut ports = Vec::new();
        if let Some(port_arr) = generic.get("ports").and_then(|v| v.as_array()) {
            for p in port_arr {
                if let Some(p_str) = p.as_str() {
                    ports.push(p_str.to_string());
                }
            }
        }

        Ok(ModemIdentity {
            modem_id: modem_id.to_string(),
            dbus_path,
            manufacturer,
            model: device_model,
            primary_port,
            ports,
            state,
            sim_path,
        })
    }

    /// Lists all bearers and filters those connected or containing IMS APN
    pub fn get_bearers(&self, modem_id: &str) -> Result<Vec<BearerSummary>> {
        let stdout = match self.run_args(&["-J", "-m", modem_id]) {
            Ok(out) => out,
            Err(e) => {
                warn!("Failed to query mmcli -J -m {}: {}", modem_id, e);
                return Ok(Vec::new());
            }
        };

        let parsed: Value = serde_json::from_str(&stdout)?;
        let bearer_paths = parsed
            .get("modem")
            .and_then(|m| m.get("generic"))
            .and_then(|g| g.get("bearers"))
            .and_then(|b| b.as_array());

        let mut summaries = Vec::new();

        if let Some(paths) = bearer_paths {
            for p_val in paths {
                if let Some(path_str) = p_val.as_str() {
                    let bearer_id = path_str.split('/').next_back().unwrap_or("").to_string();
                    if bearer_id.is_empty() {
                        continue;
                    }

                    if let Ok(b_out) = self.run_args(&["-J", "-b", &bearer_id]) {
                        if let Some(summary) = parse_bearer_summary(&bearer_id, &b_out) {
                            summaries.push(summary);
                        }
                    }
                }
            }
        }

        Ok(summaries)
    }

    /// Disconnects a specific bearer by ID
    pub fn disconnect_bearer(&self, bearer_id: &str) -> Result<()> {
        debug!("Disconnecting bearer {}", bearer_id);
        let _ = self.run_args(&["-b", bearer_id, "--disconnect"])?;
        Ok(())
    }
}

pub fn parse_modem_id_from_json(json_str: &str) -> Option<String> {
    let parsed: Value = serde_json::from_str(json_str).ok()?;
    if let Some(modem_list) = parsed.get("modem-list").and_then(|v| v.as_array()) {
        if let Some(first_path) = modem_list.first().and_then(|v| v.as_str()) {
            return first_path.split('/').next_back().map(ToString::to_string);
        }
    }
    None
}

pub fn parse_bearer_summary(bearer_id: &str, json_str: &str) -> Option<BearerSummary> {
    let parsed: Value = serde_json::from_str(json_str).ok()?;
    let bearer = parsed.get("bearer")?;

    let apn = bearer
        .get("properties")
        .and_then(|p| p.get("apn"))
        .and_then(|a| a.as_str())
        .unwrap_or("")
        .to_string();

    let connected = bearer
        .get("status")
        .and_then(|s| s.get("connected"))
        .and_then(|c| c.as_str())
        .is_some_and(|c| c == "yes");

    let interface = bearer
        .get("status")
        .and_then(|s| s.get("interface"))
        .and_then(|i| i.as_str())
        .map(ToString::to_string);

    let ip_type = bearer
        .get("properties")
        .and_then(|p| p.get("ip-type"))
        .and_then(|t| t.as_str())
        .map(ToString::to_string);

    let is_ims = apn.eq_ignore_ascii_case("ims") || apn.to_ascii_lowercase().contains("ims");

    Some(BearerSummary {
        id: bearer_id.to_string(),
        dbus_path: format!("/org/freedesktop/ModemManager1/Bearer/{bearer_id}"),
        apn,
        connected,
        interface,
        ip_type,
        is_ims,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_modem_id_from_json() {
        let json_data = r#"{"modem-list": ["/org/freedesktop/ModemManager1/Modem/0"]}"#;
        assert_eq!(parse_modem_id_from_json(json_data), Some("0".to_string()));

        let json_multi = r#"{"modem-list": ["/org/freedesktop/ModemManager1/Modem/3"]}"#;
        assert_eq!(parse_modem_id_from_json(json_multi), Some("3".to_string()));
    }

    #[test]
    fn test_parse_bearer_summary_ims() {
        let json_data = r#"{
            "bearer": {
                "dbus-path": "/org/freedesktop/ModemManager1/Bearer/1",
                "properties": {
                    "apn": "ims",
                    "ip-type": "ipv6"
                },
                "status": {
                    "connected": "yes",
                    "interface": "qmapmux0.0"
                }
            }
        }"#;

        let summary = parse_bearer_summary("1", json_data).unwrap();
        assert_eq!(summary.id, "1");
        assert_eq!(summary.apn, "ims");
        assert!(summary.connected);
        assert!(summary.is_ims);
        assert_eq!(summary.interface.as_deref(), Some("qmapmux0.0"));
        assert_eq!(summary.ip_type.as_deref(), Some("ipv6"));
    }

    #[test]
    fn test_parse_bearer_summary_internet() {
        let json_data = r#"{
            "bearer": {
                "dbus-path": "/org/freedesktop/ModemManager1/Bearer/0",
                "properties": {
                    "apn": "jionet",
                    "ip-type": "ipv4v6"
                },
                "status": {
                    "connected": "yes",
                    "interface": "rmnet_ipa0"
                }
            }
        }"#;

        let summary = parse_bearer_summary("0", json_data).unwrap();
        assert_eq!(summary.id, "0");
        assert_eq!(summary.apn, "jionet");
        assert!(summary.connected);
        assert!(!summary.is_ims);
    }
}
