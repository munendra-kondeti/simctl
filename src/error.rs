use thiserror::Error;

#[derive(Error, Debug)]
#[allow(dead_code)]
pub enum DimsdError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization / Deserialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("TOML config error: {0}")]
    TomlDe(#[from] toml::de::Error),

    #[error("TOML serialization error: {0}")]
    TomlSer(#[from] toml::ser::Error),

    #[error("Command execution failed [{command}]: {message}")]
    CommandFailed { command: String, message: String },

    #[error("Modem not found: {0}")]
    ModemNotFound(String),

    #[error("Target SIM slot {0} is not present or not ready")]
    SlotNotPresent(u8),

    #[error("QMI communication error on device {device}: {message}")]
    QmiError { device: String, message: String },

    #[error("Timeout waiting for modem to become ready after {0} seconds")]
    ModemTimeout(u64),

    #[error("Invalid configuration: {0}")]
    Config(String),
}

pub type Result<T> = std::result::Result<T, DimsdError>;
