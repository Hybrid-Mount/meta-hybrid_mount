// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

mod conf;
mod core;
mod defs;
mod mount;
mod sys;
mod utils;

use core::MountController;

use anyhow::{Context, Result};
use clap::Parser;
use conf::{
    cli::{Cli, Commands},
    cli_handlers,
    config::Config,
};
use mimalloc::MiMalloc;

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

    Ok(Config::load_default().unwrap_or_else(|e| {
        let is_not_found = e
            .root_cause()
            .downcast_ref::<std::io::Error>()
            .map(|io_err| io_err.kind() == std::io::ErrorKind::NotFound)
            .unwrap_or(false);

        if !is_not_found {
            log::warn!("Failed to load default config, using defaults: {:#}", e);
        }

        Config::default()
    }))
}

fn load_final_config(cli: &Cli) -> Result<Config> {
    let mut config = load_config(cli)?;
    config.merge_with_cli(
        cli.moduledir.clone(),
        cli.mountsource.clone(),
        cli.partitions.clone(),
    );
    Ok(config)
}

fn main() -> Result<()> {
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);

    let _ = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build_global();

    let cli = Cli::parse();

    if let Some(command) = &cli.command {
        match command {
            Commands::GenConfig { output, force } => {
                cli_handlers::handle_gen_config(output, *force)?
            }
            Commands::ShowConfig => cli_handlers::handle_show_config(&cli)?,
            Commands::SaveConfig { payload } => cli_handlers::handle_save_config(payload)?,
            Commands::SaveModuleRules { module, payload } => {
                cli_handlers::handle_save_module_rules(module, payload)?
            }
            Commands::Modules => cli_handlers::handle_modules(&cli)?,
            Commands::Conflicts => cli_handlers::handle_conflicts(&cli)?,
            Commands::Diagnostics => cli_handlers::handle_diagnostics(&cli)?,
        }

        return Ok(());
    }

    sys::fs::ensure_dir_exists(defs::RUN_DIR)
        .with_context(|| format!("Failed to create run directory: {}", defs::RUN_DIR))?;

    let config = load_final_config(&cli)?;

    utils::init_logging().context("Failed to initialize logging")?;

    log::info!(">> Initializing Hybrid Mount Daemon...");

    if let Ok(version) = std::fs::read_to_string("/proc/sys/kernel/osrelease") {
        log::debug!("Kernel Version: {}", version.trim());
    }

    utils::check_ksu();

    if config.disable_umount {
        log::warn!("!! Umount is DISABLED via config.");
    }

    let mnt_base = utils::get_mnt();

    sys::fs::ensure_dir_exists(&mnt_base)?;

    let daemon_result = (|| -> Result<()> {
        MountController::new(config, &mnt_base)
            .init_storage(&mnt_base)
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
    })();

    if let Err(e) = daemon_result {
        let err_msg = format!("{:#}", e).replace('\n', " -> ");
        core::inventory::model::update_crash_description(&err_msg);
        return Err(e);
    }

    Ok(())
}
