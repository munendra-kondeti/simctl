mod cli;
mod config;
mod error;
mod modem;

use clap::Parser;
use cli::{
    ActionArgs, CheckArgs, Cli, Commands, DaemonArgs, ProfileCommands, SlotSelection, StatusArgs,
    SwitchSlotArgs,
};
use config::Config;
use error::Result;
use modem::types::{
    FullStatusReport, OperationResult, ProfileOperationResult, SwitchSlotResult,
    SystemHardwareCheck, WdsProfile,
};
use modem::ModemManager;
use std::thread::sleep;
use std::time::Duration;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Configure logging
    let filter_level = if cli.verbose {
        Level::DEBUG
    } else if cli.quiet {
        Level::ERROR
    } else {
        Level::INFO
    };

    let subscriber = FmtSubscriber::builder()
        .with_max_level(filter_level)
        .with_target(false)
        .without_time()
        .finish();

    let _ = tracing::subscriber::set_global_default(subscriber);

    // Load config
    let (mut config, config_path) = Config::load_or_default(&cli.config);

    // CLI overrides
    if let Some(dev) = &cli.device {
        config.modem.device.clone_from(dev);
    }
    if let Some(m) = &cli.modem {
        config.modem.modem_id.clone_from(m);
    }

    let modem_mgr = ModemManager::new();

    let command = cli.command.clone();
    match command {
        Some(Commands::Status(args)) => handle_status(&modem_mgr, &config, &cli, &args),
        Some(Commands::Disable(args)) => handle_disable(&modem_mgr, &config, &cli, &args),
        Some(Commands::Enable(args)) => handle_enable(&modem_mgr, &config, &cli, &args),
        Some(Commands::SwitchSlot(args)) => handle_switch_slot(&modem_mgr, &config, &cli, &args),
        Some(Commands::Profile(args)) => handle_profile(&modem_mgr, &config, &cli, &args),
        Some(Commands::Check(args)) => {
            handle_check(&modem_mgr, &mut config, &config_path, &cli, &args)
        }
        Some(Commands::Daemon(args)) => handle_daemon(&modem_mgr, &config, &cli, &args),
        None => {
            // Default boot action: disable IMS once and exit 0 for systemd
            handle_default_boot(&modem_mgr, &config, &cli)
        }
    }
}

fn handle_status(mgr: &ModemManager, config: &Config, cli: &Cli, args: &StatusArgs) -> Result<()> {
    let slot = config.resolve_slot(args.slot.or(cli.slot));
    let report = mgr.get_status(config, slot);

    if cli.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    print_status_report(&report);
    Ok(())
}

fn handle_disable(mgr: &ModemManager, config: &Config, cli: &Cli, args: &ActionArgs) -> Result<()> {
    let slot = config.resolve_slot(args.slot.or(cli.slot));
    let res = mgr.disable_ims(config, slot, args.dry_run)?;

    if cli.json {
        println!("{}", serde_json::to_string_pretty(&res)?);
        return Ok(());
    }

    print_operation_result(&res);
    Ok(())
}

fn handle_enable(mgr: &ModemManager, config: &Config, cli: &Cli, args: &ActionArgs) -> Result<()> {
    let slot = config.resolve_slot(args.slot.or(cli.slot));
    let res = mgr.enable_ims(config, slot, args.dry_run)?;

    if cli.json {
        println!("{}", serde_json::to_string_pretty(&res)?);
        return Ok(());
    }

    print_operation_result(&res);
    Ok(())
}

fn handle_switch_slot(
    mgr: &ModemManager,
    config: &Config,
    cli: &Cli,
    args: &SwitchSlotArgs,
) -> Result<()> {
    let res = mgr.switch_slot(config, args.target_slot, !args.no_restart_mm)?;

    if cli.json {
        println!("{}", serde_json::to_string_pretty(&res)?);
        return Ok(());
    }

    print_switch_slot_result(&res);
    Ok(())
}

fn handle_profile(
    mgr: &ModemManager,
    config: &Config,
    cli: &Cli,
    cmd: &ProfileCommands,
) -> Result<()> {
    match cmd {
        ProfileCommands::List => {
            let qmi_device = mgr.resolve_device(&config.modem.device);
            let status = mgr.get_status(config, SlotSelection::All);
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&status.wds_profiles)?);
                return Ok(());
            }
            print_profile_list(&status.wds_profiles, &qmi_device);
        }
        ProfileCommands::Add(args) => {
            let res = mgr.create_profile(config, &args.apn, &args.pdp_type, args.disabled)?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&res)?);
                return Ok(());
            }
            print_profile_operation_result(&res);
        }
        ProfileCommands::Delete(args) => {
            let res = mgr.delete_profile(config, args.index)?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&res)?);
                return Ok(());
            }
            print_profile_operation_result(&res);
        }
    }
    Ok(())
}

fn handle_check(
    mgr: &ModemManager,
    config: &mut Config,
    config_path: &std::path::Path,
    cli: &Cli,
    args: &CheckArgs,
) -> Result<()> {
    let check = mgr.check_system(config, config_path);

    if args.update {
        // Update config with detected hardware
        config.modem.device.clone_from(&check.qmi_device_resolved);
        if let Some(m) = &check.modem_detected {
            config.modem.modem_id.clone_from(&m.modem_id);
        }
        if let Some(iface) = &check.net_interface_resolved {
            config.modem.net_interface.clone_from(iface);
        }

        config.save_to(config_path)?;
        info!("Updated configuration written to {}", config_path.display());
    }

    if cli.json {
        println!("{}", serde_json::to_string_pretty(&check)?);
        return Ok(());
    }

    print_check_report(&check, args.update);
    Ok(())
}

fn handle_daemon(mgr: &ModemManager, config: &Config, cli: &Cli, args: &DaemonArgs) -> Result<()> {
    let slot = config.resolve_slot(cli.slot);
    let qmi_dev = mgr.resolve_device(&config.modem.device);
    let modem_id = mgr
        .resolve_modem_id(&config.modem.modem_id)
        .unwrap_or_else(|_| "0".to_string());

    info!(
        "dimsd daemon starting (waiting up to {}s for modem readiness)...",
        config.daemon.timeout_secs
    );

    mgr.wait_for_ready(&modem_id, &qmi_dev, config.daemon.timeout_secs);

    let res = mgr.disable_ims(config, slot, false)?;
    info!("{}", res.message);

    if args.watch {
        info!(
            "Watching for IMS re-attachment every {} seconds (press Ctrl+C to stop)...",
            args.interval
        );
        loop {
            sleep(Duration::from_secs(args.interval));
            let _ = mgr.disable_ims(config, slot, false);
        }
    }

    Ok(())
}

fn handle_default_boot(mgr: &ModemManager, config: &Config, cli: &Cli) -> Result<()> {
    let slot = config.resolve_slot(cli.slot);
    let qmi_dev = mgr.resolve_device(&config.modem.device);
    let modem_id = mgr
        .resolve_modem_id(&config.modem.modem_id)
        .unwrap_or_else(|_| "0".to_string());

    info!(
        "dimsd boot execution (modem: {}, qmi: {}, slot: {:?})",
        modem_id, qmi_dev, slot
    );

    mgr.wait_for_ready(&modem_id, &qmi_dev, config.daemon.timeout_secs);
    let res = mgr.disable_ims(config, slot, false)?;

    if cli.json {
        println!("{}", serde_json::to_string_pretty(&res)?);
    } else {
        info!("Result: {}", res.message);
    }

    Ok(())
}

fn print_status_report(r: &FullStatusReport) {
    println!("=== dimsd Status Report ===");
    println!("Target Selection : {}", r.target_slot);
    println!("QMI Device       : {}", r.qmi_device);

    if let Some(m) = &r.modem {
        println!("Modem ID         : {}", m.modem_id);
        println!("Modem State      : {}", m.state);
        println!("Primary Port     : {}", m.primary_port);
        println!("Manufacturer     : {}", m.manufacturer);
        println!("Model            : {}", m.model);
    } else {
        println!("Modem            : Not detected via ModemManager");
    }

    println!("\n--- SIM Slots ---");
    if r.slots.is_empty() {
        println!("  (QMI query restricted - run with 'sudo dimsd status' for direct SIM hardware queries)");
    } else {
        for s in &r.slots {
            let status_badge = if s.present { "PRESENT" } else { "EMPTY" };
            let role = if s.is_primary {
                " [Primary Active]"
            } else if s.present {
                " [Secondary]     "
            } else {
                "                 "
            };
            let cap_desc = if s.has_isim {
                " - VoLTE / IMS capable (ISIM detected)"
            } else if s.has_usim {
                " - Standard USIM"
            } else {
                ""
            };
            println!(
                "  Slot [{}]{}: {} (USIM: {}, ISIM: {}) - {}{}",
                s.slot, role, status_badge, s.has_usim, s.has_isim, s.state_description, cap_desc
            );
        }
    }

    println!("\n--- WDS 3GPP Profiles (QMI) ---");
    if r.wds_profiles.is_empty() {
        println!(
            "  (QMI query restricted - run with 'sudo dimsd status' for modem profile details)"
        );
    } else {
        for p in &r.wds_profiles {
            let ims_tag = if p.is_ims { " [IMS PROFILE]" } else { "" };
            let state_tag = if p.disabled { "DISABLED" } else { "ACTIVE" };
            println!(
                "  [{}] APN: '{:<12}' | PDP: {:<12} | Status: {}{}",
                p.index, p.apn, p.pdp_type, state_tag, ims_tag
            );
        }
    }

    println!("\n--- ModemManager Bearers ---");
    if r.active_bearers.is_empty() {
        println!("  (No bearers tracked)");
    } else {
        for b in &r.active_bearers {
            let ims_tag = if b.is_ims { " [IMS BEARER]" } else { "" };
            let iface = b.interface.as_deref().unwrap_or("none");
            let state = if b.connected {
                "CONNECTED"
            } else {
                "DISCONNECTED"
            };
            println!(
                "  Bearer {} | APN: '{:<10}' | Interface: {:<8} | State: {}{}",
                b.id, b.apn, iface, state, ims_tag
            );
        }
    }

    println!("\n--- IMS Subsystem Status ---");
    if let Some(reg) = &r.ims_registration {
        let tech = reg.technology.as_deref().unwrap_or("none");
        println!("  Registration     : {} (Technology: {})", reg.status, tech);
    } else {
        println!("  Registration     : Unavailable / Not Registered");
    }

    if !r.ims_services.is_empty() {
        for svc in &r.ims_services {
            let note = match svc.service_name.as_str() {
                "Voice service" if svc.status == "unavailable" => " (VoLTE disabled)",
                "SMS service" if svc.status == "unavailable" => " (SMSoIP disabled)",
                "Video Telephony service" if svc.status == "unavailable" => " (ViLTE disabled)",
                _ => "",
            };
            println!("  {:<24} : {}{}", svc.service_name, svc.status, note);
        }
    }
    println!("===========================");
}

fn print_operation_result(res: &OperationResult) {
    let mode = if res.dry_run { " [DRY RUN]" } else { "" };
    println!(
        "=== Operation: {} {}{} ===",
        res.action.to_uppercase(),
        res.slot,
        mode
    );
    println!(
        "Status               : {}",
        if res.success { "SUCCESS" } else { "FAILED" }
    );
    println!("Message              : {}", res.message);
    println!(
        "Bearers Disconnected : {}",
        if res.bearers_disconnected.is_empty() {
            "None".to_string()
        } else {
            res.bearers_disconnected.join(", ")
        }
    );
    println!(
        "Profiles Modified    : {}",
        if res.profiles_modified.is_empty() {
            "None".to_string()
        } else {
            res.profiles_modified
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        }
    );
    println!("==========================================");
}

fn print_check_report(c: &SystemHardwareCheck, updated: bool) {
    println!("=== dimsd System Check & Verification ===");
    println!("Config File Path     : {}", c.config_path);
    println!("QMI Device Resolved  : {}", c.qmi_device_resolved);
    println!(
        "Modem Detected       : {}",
        c.modem_detected.as_ref().map_or_else(
            || "Not found".to_string(),
            |m| format!("Modem/{} ({})", m.modem_id, m.primary_port)
        )
    );
    println!(
        "Network Interface    : {}",
        c.net_interface_resolved.as_deref().unwrap_or("none")
    );
    println!(
        "QRTR Node Available  : {}",
        if c.qrtr_available { "YES" } else { "NO" }
    );

    println!("\n--- SIM Slots ---");
    for s in &c.slots {
        println!(
            "  Slot [{}]: {} (USIM: {}, ISIM: {})",
            s.slot,
            if s.present { "PRESENT" } else { "EMPTY" },
            s.has_usim,
            s.has_isim
        );
    }

    println!("\n--- Verification Notes ---");
    for note in &c.notes {
        println!("  - {note}");
    }

    if updated {
        println!(
            "\n[OK] Configuration file successfully updated with verified live hardware values!"
        );
    } else if c.config_matches_hardware {
        println!("\n[OK] Config perfectly matches live system hardware.");
    } else {
        println!("\n[!] Configuration differs from detected hardware. Run 'dimsd check --update' to sync.");
    }
    println!("=========================================");
}

fn print_switch_slot_result(res: &SwitchSlotResult) {
    println!("=== SIM Provisioning Switch ===");
    println!("Status          : SUCCESS");
    println!("Active SIM Slot : Slot {}", res.new_slot);
    if let Some(prev) = res.previous_slot {
        println!("Previous Slot   : Slot {prev}");
    }
    println!("Message         : {}", res.message);
    println!("===============================");
}

fn print_profile_operation_result(res: &ProfileOperationResult) {
    println!("=== WDS Profile Operation ===");
    println!("Action  : {}", res.action.to_uppercase());
    println!("Status  : SUCCESS");
    if let Some(idx) = res.index {
        println!("Index   : {idx}");
    }
    if let Some(apn) = &res.apn {
        println!("APN     : '{apn}'");
    }
    println!("Message : {}", res.message);
    println!("=============================");
}

fn print_profile_list(profiles: &[WdsProfile], device: &str) {
    println!("=== WDS 3GPP APN Profiles ({device}) ===");
    if profiles.is_empty() {
        println!("  (No profiles retrieved or QMI restricted - run with 'sudo')");
    } else {
        for p in profiles {
            let ims_tag = if p.is_ims { " [IMS PROFILE]" } else { "" };
            let state_tag = if p.disabled { "DISABLED" } else { "ACTIVE" };
            println!(
                "  [{}] APN: '{:<12}' | PDP: {:<12} | Status: {}{}",
                p.index, p.apn, p.pdp_type, state_tag, ims_tag
            );
        }
    }
    println!("=========================================");
}
