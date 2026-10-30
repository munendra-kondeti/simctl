use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "simctl",
    author = "Open Source Contributors",
    version,
    about = "Cellular SIM and modem management CLI and daemon for Nura OS",
    long_about = "Lightweight, robust CLI utility and system daemon to manage dual SIM slots, switch provisioning sessions, manage APN profiles, and control IMS/VoLTE on Qualcomm-based Nura OS."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Path to config file
    #[arg(short, long, global = true, default_value = "/etc/simctl/config.toml")]
    pub config: PathBuf,

    /// Target QMI device (e.g. `<qrtr://0>`, `/dev/cdc-wdm0`, or `auto`)
    #[arg(short, long, global = true)]
    pub device: Option<String>,

    /// Target `ModemManager` modem index (e.g. `0` or `auto`)
    #[arg(short, long, global = true)]
    pub modem: Option<String>,

    /// Target SIM slot to manage [1, 2, all] (default: all)
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

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Show current status of SIM slots, bearers, and IMS profiles
    Status(StatusArgs),

    /// Disable IMS bearers, profiles, and autoconnect for selected slot
    Disable(ActionArgs),

    /// Re-enable IMS profiles and autoconnect for selected slot
    Enable(ActionArgs),

    /// Switch active SIM provisioning session (1 or 2), or inspect slots if omitted
    #[command(alias = "switch-sim")]
    SwitchSlot(SwitchSlotArgs),

    /// Manage WDS 3GPP APN profiles (list, add, delete)
    #[command(subcommand)]
    Profile(ProfileCommands),

    /// Check modem hardware, ports, and compare/update config.toml
    Check(CheckArgs),

    /// Run daemon mode (default action if no subcommand passed)
    Daemon(DaemonArgs),
}

#[derive(Args, Debug, Clone)]
pub struct SwitchSlotArgs {
    /// Target SIM slot number (1 or 2). If omitted, displays current slots and suggests the alternate slot
    #[arg(value_name = "SLOT", value_parser = clap::value_parser!(u8).range(1..=2))]
    pub target_slot: Option<u8>,

    /// Do not restart `ModemManager` after switching
    #[arg(long)]
    pub no_restart_mm: bool,
}

#[derive(Subcommand, Debug, Clone)]
pub enum ProfileCommands {
    /// List all 3GPP APN profiles
    List,

    /// Add a new 3GPP APN profile
    Add(ProfileAddArgs),

    /// Delete an existing 3GPP APN profile by index
    #[command(alias = "del")]
    Delete(ProfileDeleteArgs),
}

#[derive(Args, Debug, Clone)]
pub struct ProfileAddArgs {
    /// APN string (e.g. 'jionet', 'internet', 'ims')
    pub apn: String,

    /// PDP type (IPV4, IPV6, or IPV4V6)
    #[arg(long = "pdp-type", visible_alias = "pdp", default_value = "IPV4V6")]
    pub pdp_type: String,

    /// Create profile in disabled state
    #[arg(long)]
    pub disabled: bool,
}

#[derive(Args, Debug, Clone)]
pub struct ProfileDeleteArgs {
    /// Profile index to delete
    pub index: u32,
}

#[derive(Args, Debug, Default, Clone)]
pub struct StatusArgs {
    /// Target SIM slot (1, 2, or all) (default: all)
    #[arg(short, long, value_enum)]
    pub slot: Option<SlotSelection>,
}

#[derive(Args, Debug, Default, Clone)]
pub struct ActionArgs {
    /// Target SIM slot (1, 2, or all) (default: all)
    #[arg(short, long, value_enum)]
    pub slot: Option<SlotSelection>,

    /// Perform a trial run with no changes made
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args, Debug, Default, Clone)]
pub struct CheckArgs {
    /// Atomically update /etc/simctl/config.toml with live detected hardware
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
