// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::{Context, Result};

use crate::{
    conf::{
        cli::{Cli, Commands},
        cli_handlers,
        config::Config,
        loader::{self, LoadPolicy},
    },
    core::MountController,
    defs, sys, utils,
};

fn load_final_config(cli: &Cli) -> Result<Config> {
    let mut config = loader::load_config(cli, LoadPolicy::FallbackToDefault)?;
    config.merge_with_cli(
        cli.moduledir.clone(),
        cli.mountsource.clone(),
        cli.partitions.clone(),
    );
    Ok(config)
}

fn run_command(cli: &Cli, command: &Commands) -> Result<()> {
    match command {
        Commands::GenConfig { output, force } => cli_handlers::handle_gen_config(output, *force),
        Commands::ShowConfig => cli_handlers::handle_show_config(cli),
        Commands::SaveConfig { payload } => cli_handlers::handle_save_config(payload),
        Commands::SaveModuleRules { module, payload } => {
            cli_handlers::handle_save_module_rules(module, payload)
        }
        Commands::Modules => cli_handlers::handle_modules(cli),
    }
}

fn run_daemon(cli: &Cli) -> Result<()> {
    sys::fs::ensure_dir_exists(defs::RUN_DIR)
        .with_context(|| format!("Failed to create run directory: {}", defs::RUN_DIR))?;

    let config = load_final_config(cli)?;

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
        crate::core::inventory::model::update_crash_description(&err_msg);
        return Err(e);
    }

    Ok(())
}

pub fn run(cli: Cli) -> Result<()> {
    if let Some(command) = &cli.command {
        return run_command(&cli, command);
    }

    run_daemon(&cli)
}
