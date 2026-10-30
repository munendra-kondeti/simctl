use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "dimsd",
    author = "roy <roy@nura.eco>",
    version,
    about = "Disable IMS Daemon & CLI for Nura OS",
    long_about = "Lightweight, robust daemon and CLI tool to disable and manage IP Multimedia Subsystem (IMS/VoLTE) across dual SIM slots on Qualcomm-based Nura OS."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Path to config file
    #[arg(short, long, global = true, default_value = "/etc/dimsd/config.toml")]
    pub config: PathBuf,

    /// Target QMI device (e.g. `<qrtr://0>`, `/dev/cdc-wdm0`, or `auto`)
    #[arg(short, long, global = true)]
    pub device: Option<String>,

    /// Target `ModemManager` modem index (e.g. `0` or `auto`)
    #[arg(short, long, global = true)]
    pub modem: Option<String>,

    /// Target SIM slot to manage (default: 1)
    #[arg(short, long, global = true, value_enum)]
    pub slot: Option<SlotSelection>,

    /// Output results in JSON format
    #[arg(long, global = true)]
    pub json: bool,

    /// Verbose debug logging
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Suppress informational messages (show errors only)
    #[arg(short, long, global = true)]
    pub quiet: bool,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotSelection {
    #[value(name = "1")]
    Slot1,
    #[value(name = "2")]
    Slot2,
    #[value(name = "all")]
    All,
}

impl SlotSelection {
    pub fn as_u8(self) -> Option<u8> {
        match self {
            SlotSelection::Slot1 => Some(1),
            SlotSelection::Slot2 => Some(2),
            SlotSelection::All => None,
        }
    }
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Show current status of SIM slots, bearers, and IMS profiles
    Status(StatusArgs),

    /// Disable IMS bearers, profiles, and autoconnect for selected slot
    Disable(ActionArgs),

    /// Re-enable IMS profiles and autoconnect for selected slot
    Enable(ActionArgs),

    /// Check modem hardware, ports, and compare/update config.toml
    Check(CheckArgs),

    /// Run daemon mode (default action if no subcommand passed)
    Daemon(DaemonArgs),
}

#[derive(Args, Debug, Default, Clone)]
pub struct StatusArgs {
    /// Target SIM slot (1, 2, or all)
    #[arg(short, long, value_enum)]
    pub slot: Option<SlotSelection>,
}

#[derive(Args, Debug, Default, Clone)]
pub struct ActionArgs {
    /// Target SIM slot (1, 2, or all)
    #[arg(short, long, value_enum)]
    pub slot: Option<SlotSelection>,

    /// Perform a trial run with no changes made
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args, Debug, Default, Clone)]
pub struct CheckArgs {
    /// Atomically update /etc/dimsd/config.toml with live detected hardware
    #[arg(short, long)]
    pub update: bool,
}

#[derive(Args, Debug, Default, Clone)]
pub struct DaemonArgs {
    /// Stay resident and continuously monitor to prevent automatic IMS re-attachment
    #[arg(long)]
    pub watch: bool,

    /// Polling interval in seconds if --watch is enabled
    #[arg(long, default_value = "10")]
    pub interval: u64,
}
