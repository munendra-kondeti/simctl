use crate::cli::SlotSelection;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub modem: ModemConfig,
    #[serde(default)]
    pub daemon: DaemonConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModemConfig {
    #[serde(default = "default_auto")]
    pub device: String,

    #[serde(default = "default_auto")]
    pub modem_id: String,

    #[serde(default = "default_slot")]
    pub default_slot: String,

    #[serde(default = "default_auto")]
    pub net_interface: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,

    #[serde(default = "default_poll_interval_secs")]
    pub poll_interval_secs: u64,

    #[serde(default = "default_auto_sync")]
    pub auto_sync_config: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
}

fn default_auto() -> String {
    "auto".to_string()
}

fn default_slot() -> String {
    "all".to_string()
}

fn default_timeout_secs() -> u64 {
    30
}

fn default_poll_interval_secs() -> u64 {
    10
}

fn default_auto_sync() -> bool {
    true
}

fn default_log_level() -> String {
    "info".to_string()
}

impl Default for ModemConfig {
    fn default() -> Self {
        Self {
            device: default_auto(),
            modem_id: default_auto(),
            default_slot: default_slot(),
            net_interface: default_auto(),
        }
    }
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            timeout_secs: default_timeout_secs(),
            poll_interval_secs: default_poll_interval_secs(),
            auto_sync_config: default_auto_sync(),
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
        }
    }
}

impl Config {
    /// Loads configuration from given path, with fallback to ./config.toml or default
    pub fn load_or_default(path: &Path) -> (Self, PathBuf) {
        if path.exists() {
            debug!("Loading config from {}", path.display());
            match fs::read_to_string(path) {
                Ok(content) => match toml::from_str::<Config>(&content) {
                    Ok(cfg) => return (cfg, path.to_path_buf()),
                    Err(e) => {
                        eprintln!(
                            "Warning: Failed to parse config at {}: {}. Using defaults.",
                            path.display(),
                            e
                        );
                    }
                },
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to read config at {}: {}. Using defaults.",
                        path.display(),
                        e
                    );
                }
            }
        }

        let legacy_path = PathBuf::from("/etc/dimsd/config.toml");
        if legacy_path.exists() {
            debug!("Loading fallback legacy config from /etc/dimsd/config.toml");
            if let Ok(content) = fs::read_to_string(&legacy_path) {
                if let Ok(cfg) = toml::from_str::<Config>(&content) {
                    return (cfg, legacy_path);
                }
            }
        }

        let local_path = PathBuf::from("./config.toml");
        if local_path.exists() {
            debug!("Loading fallback local config from ./config.toml");
            if let Ok(content) = fs::read_to_string(&local_path) {
                if let Ok(cfg) = toml::from_str::<Config>(&content) {
                    return (cfg, local_path);
                }
            }
        }

        (Config::default(), path.to_path_buf())
    }

    /// Atomically writes configuration to target path
    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }

        let serialized = toml::to_string_pretty(self)?;
        let temp_path = path.with_extension("tmp");
        fs::write(&temp_path, serialized)?;
        fs::rename(&temp_path, path)?;
        info!("Updated configuration written to {}", path.display());
        Ok(())
    }

    /// Resolves target slot using CLI option or config default
    pub fn resolve_slot(&self, cli_slot: Option<SlotSelection>) -> SlotSelection {
        if let Some(s) = cli_slot {
            return s;
        }

        match self.modem.default_slot.trim().to_lowercase().as_str() {
            "1" => SlotSelection::Slot1,
            "2" => SlotSelection::Slot2,
            _ => SlotSelection::All,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let cfg = Config::default();
        assert_eq!(cfg.modem.device, "auto");
        assert_eq!(cfg.modem.modem_id, "auto");
        assert_eq!(cfg.modem.default_slot, "all");
        assert_eq!(cfg.daemon.timeout_secs, 30);
        assert_eq!(cfg.daemon.poll_interval_secs, 10);
    }

    #[test]
    fn test_resolve_slot() {
        let mut cfg = Config::default();
        assert_eq!(cfg.resolve_slot(None), SlotSelection::All);
        assert_eq!(
            cfg.resolve_slot(Some(SlotSelection::Slot2)),
            SlotSelection::Slot2
        );
        assert_eq!(
            cfg.resolve_slot(Some(SlotSelection::Slot1)),
            SlotSelection::Slot1
        );

        cfg.modem.default_slot = "1".to_string();
        assert_eq!(cfg.resolve_slot(None), SlotSelection::Slot1);

        cfg.modem.default_slot = "2".to_string();
        assert_eq!(cfg.resolve_slot(None), SlotSelection::Slot2);

        cfg.modem.default_slot = "all".to_string();
        assert_eq!(cfg.resolve_slot(None), SlotSelection::All);
    }

    #[test]
    fn test_config_serde_roundtrip() {
        let cfg = Config::default();
        let toml_str = toml::to_string(&cfg).unwrap();
        let deserialized: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(deserialized.modem.device, "auto");
        assert_eq!(deserialized.modem.default_slot, "all");
    }
}
