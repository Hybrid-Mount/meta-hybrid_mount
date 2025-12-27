// Copyright 2025 Meta-Hybrid Mount Authors
// SPDX-License-Identifier: GPL-3.0-or-later

mod conf;
mod core;
mod defs;
#[cfg(any(target_os = "linux", target_os = "android"))]
mod ksu;
mod mount;
mod utils;

use anyhow::{Context, Result};
use clap::Parser;
use mimalloc::MiMalloc;
use std::path::{Path, PathBuf};

use conf::{
    cli::{Cli, Commands},
    cli_handlers,
    config::{CONFIG_FILE_DEFAULT, Config},
};
use core::{OryzaEngine, executor, granary, inventory, planner, winnow};

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn load_config(cli: &Cli) -> Result<Config> {
    if let Some(config_path) = &cli.config {
        return Config::from_file(config_path).with_context(|| {
            format!(
                "Failed to load config from custom path: {}",
                config_path.display()
            )
        });
    }

    match Config::load_default() {
        Ok(config) => Ok(config),
        Err(e) => {
            let is_not_found = e
                .root_cause()
                .downcast_ref::<std::io::Error>()
                .map(|io_err| io_err.kind() == std::io::ErrorKind::NotFound)
                .unwrap_or(false);

            if is_not_found {
                Ok(Config::default())
            } else {
                Err(e).context(format!(
                    "Failed to load default config from {}",
                    CONFIG_FILE_DEFAULT
                ))
            }
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Some(command) = &cli.command {
        match command {
            Commands::GenConfig { output } => cli_handlers::handle_gen_config(output)?,
            Commands::ShowConfig => cli_handlers::handle_show_config(&cli)?,
            Commands::SaveConfig { payload } => cli_handlers::handle_save_config(&cli, payload)?,
            Commands::SaveRules { module, payload } => {
                cli_handlers::handle_save_rules(module, payload)?
            }
            Commands::Storage => cli_handlers::handle_storage()?,
            Commands::Modules => cli_handlers::handle_modules(&cli)?,
            Commands::Conflicts => cli_handlers::handle_conflicts(&cli)?,
            Commands::Diagnostics => cli_handlers::handle_diagnostics(&cli)?,
            Commands::SystemAction { action, value } => {
                cli_handlers::handle_system_action(&cli, action, value.as_deref())?
            }
        }

        return Ok(());
    }

    let mut config = load_config(&cli)?;

    config.merge_with_cli(
        cli.moduledir.clone(),
        cli.mountsource.clone(),
        cli.verbose,
        cli.partitions.clone(),
        cli.dry_run,
    );

    if !config.dry_run
        && let Err(e) = granary::engage_ratoon_protocol()
    {
        log::error!("Failed to engage Ratoon Protocol: {}", e);
    }

    ksu::info::check();

    if utils::check_zygisksu_enforce_status() {
        if config.allow_umount_coexistence {
            if config.verbose {
                println!(
                    ">> ZygiskSU Enforce!=0 detected, but Umount Coexistence enabled. Respecting user config."
                );
            }
        } else {
            if config.verbose {
                println!(">> ZygiskSU Enforce!=0 detected. Forcing DISABLE_UMOUNT to TRUE.");
            }

            config.disable_umount = true;
        }
    }

    if config.dry_run {
        env_logger::builder()
            .filter_level(if config.verbose {
                log::LevelFilter::Debug
            } else {
                log::LevelFilter::Info
            })
            .init();

        log::info!(":: DRY-RUN / DIAGNOSTIC MODE ::");

        let module_list =
            inventory::scan(&config.moduledir, &config).context("Inventory scan failed")?;

        log::info!(">> Inventory: Found {} modules", module_list.len());

        let plan = planner::generate(&config, &module_list, &config.moduledir)
            .context("Plan generation failed")?;

        plan.print_visuals();

        log::info!(">> Analyzing File Conflicts...");

        let report = plan.analyze_conflicts();

        if report.details.is_empty() {
            log::info!("   No file conflicts detected. Clean.");
        } else {
            log::warn!("!! DETECTED {} FILE CONFLICTS !!", report.details.len());

            let winnowed = winnow::sift_conflicts(report.details, &config.winnowing);

            for c in winnowed {
                let status = if c.is_forced { "(FORCED)" } else { "" };

                log::warn!(
                    "   [{}] {} <== {:?} >> Selected: {} {}",
                    "CONFLICT",
                    c.path.display(),
                    c.contenders,
                    c.selected,
                    status
                );
            }
        }

        log::info!(">> Running System Diagnostics...");

        let issues = executor::diagnose_plan(&plan);

        let mut critical_count = 0;

        for issue in issues {
            match issue.level {
                core::executor::DiagnosticLevel::Critical => {
                    log::error!("[CRITICAL][{}] {}", issue.context, issue.message);

                    critical_count += 1;
                }
                core::executor::DiagnosticLevel::Warning => {
                    log::warn!("[WARN][{}] {}", issue.context, issue.message);
                }
                core::executor::DiagnosticLevel::Info => {
                    log::info!("[INFO][{}] {}", issue.context, issue.message);
                }
            }
        }

        if critical_count > 0 {
            log::error!(
                ">> ❌ DIAGNOSTICS FAILED: {} critical issues found.",
                critical_count
            );

            log::error!(">> Mounting now would likely result in a bootloop.");

            std::process::exit(1);
        } else {
            log::info!(">> ✅ Diagnostics passed. System looks healthy.");
        }

        return Ok(());
    }

    let _log_guard = utils::init_logging(config.verbose, Path::new(defs::DAEMON_LOG_FILE))
        .context("Failed to initialize logging")?;

    let camouflage_name = utils::random_kworker_name();

    if let Err(e) = utils::camouflage_process(&camouflage_name) {
        log::warn!("Failed to camouflage process: {:#}", e);
    }

    log::info!(">> Initializing Meta-Hybrid Mount Daemon...");

    log::debug!("Process camouflaged as: {}", camouflage_name);

    if let Ok(version) = std::fs::read_to_string("/proc/sys/kernel/osrelease") {
        log::debug!("Kernel Version: {}", version.trim());
    }

    if config.disable_umount {
        log::warn!("!! Umount is DISABLED via config.");
    }

    utils::ensure_dir_exists(defs::RUN_DIR)
        .with_context(|| format!("Failed to create run directory: {}", defs::RUN_DIR))?;

    let mnt_base = PathBuf::from(defs::HYBRID_MNT_DIR);

    let img_path = Path::new(defs::BASE_DIR).join("modules.img");

    if let Err(e) = granary::create_silo(&config, "Boot Backup", "Automatic Pre-Mount") {
        log::warn!("Granary: Failed to create boot snapshot: {}", e);
    }

    OryzaEngine::new(config)
        .init_storage(&mnt_base, &img_path)
        .context("Failed to initialize storage")?
        .scan_and_sync()
        .context("Failed to scan and sync modules")?
        .generate_plan()
        .context("Failed to generate mount plan")?
        .execute()
        .context("Failed to execute mount plan")?
        .finalize()
        .context("Failed to finalize boot sequence")?;

    Ok(())
}
