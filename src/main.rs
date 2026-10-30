mod cli;
mod config;
mod error;
mod modem;

use clap::Parser;
use cli::{ActionArgs, CheckArgs, Cli, Commands, DaemonArgs, StatusArgs};
use config::Config;
use error::Result;
use modem::types::{FullStatusReport, OperationResult, SystemHardwareCheck};
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
    let report = mgr.get_status(config, slot)?;

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

fn handle_check(
    mgr: &ModemManager,
    config: &mut Config,
    config_path: &std::path::Path,
    cli: &Cli,
    args: &CheckArgs,
) -> Result<()> {
    let check = mgr.check_system(config, config_path)?;

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

    let _ = mgr.wait_for_ready(&modem_id, &qmi_dev, config.daemon.timeout_secs);

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

    let _ = mgr.wait_for_ready(&modem_id, &qmi_dev, config.daemon.timeout_secs);
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
        println!("  (No slot information available)");
    } else {
        for s in &r.slots {
            let status_badge = if s.present { "PRESENT" } else { "EMPTY" };
            println!(
                "  Slot [{}]: {} (USIM: {}, ISIM: {}) - {}",
                s.slot, status_badge, s.has_usim, s.has_isim, s.state_description
            );
        }
    }

    println!("\n--- WDS 3GPP Profiles (QMI) ---");
    if r.wds_profiles.is_empty() {
        println!("  (No profiles retrieved)");
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

    println!("\n--- Active ModemManager Bearers ---");
    if r.active_bearers.is_empty() {
        println!("  (No active bearers)");
    } else {
        for b in &r.active_bearers {
            let ims_tag = if b.is_ims { " [IMS BEARER]" } else { "" };
            let iface = b.interface.as_deref().unwrap_or("none");
            println!(
                "  Bearer {} | APN: '{:<10}' | Dev: {:<10} | Connected: {}{}",
                b.id, b.apn, iface, b.connected, ims_tag
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
            println!("  {:<18} : {}", svc.service_name, svc.status);
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
