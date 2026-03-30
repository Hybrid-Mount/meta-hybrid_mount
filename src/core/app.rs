// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    collections::HashSet,
    fs::OpenOptions,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

use crate::{
    conf::{
        cli::{Cli, Commands},
        cli_handlers,
        config::Config,
        loader::{self, LoadPolicy},
    },
    core::{MountController, recovery::ModuleStageFailure},
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

    let module_dirs = list_module_dirs(&config.moduledir)?;
    let max_restarts = module_dirs.len().saturating_add(1);
    let mut restart_round = 0usize;
    let mut auto_skipped = HashSet::new();
    log::info!(
        "[stage:recovery] initialized: module_candidates={}, restart_limit={}",
        module_dirs.len(),
        max_restarts
    );

    loop {
        let attempt = restart_round + 1;
        let mnt_base = utils::get_mnt();
        sys::fs::ensure_dir_exists(&mnt_base)?;
        log::info!(
            "[stage:recovery] attempt {}/{} started with runtime mount {}",
            attempt,
            max_restarts,
            mnt_base.display()
        );

        let daemon_result = (|| -> Result<()> {
            MountController::new(config.clone(), &mnt_base)
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

        match daemon_result {
            Ok(()) => {
                if auto_skipped.is_empty() {
                    log::info!("[stage:recovery] completed without auto-skip");
                } else {
                    let mut skipped: Vec<String> = auto_skipped.iter().cloned().collect();
                    skipped.sort();
                    log::warn!(
                        "[stage:recovery] completed after auto-skipping modules: {}",
                        skipped.join(", ")
                    );
                }
                return Ok(());
            }
            Err(e) => {
                if let Some(module_failure) = e.downcast_ref::<ModuleStageFailure>() {
                    if module_failure.module_ids.is_empty() {
                        log::error!(
                            "[stage:recovery] {} failure did not include module ids; aborting recovery",
                            module_failure.stage
                        );
                    } else {
                        log::warn!(
                            "[stage:recovery] detected {} failure for modules: {}",
                            module_failure.stage,
                            module_failure.module_ids.join(", ")
                        );
                    }
                    let mut newly_marked = Vec::new();
                    let mut already_marked = Vec::new();
                    let mut unknown_modules = Vec::new();
                    for module_id in &module_failure.module_ids {
                        if auto_skipped.contains(module_id) {
                            already_marked.push(module_id.clone());
                            continue;
                        }
                        if let Some(module_dir) = module_dirs.get(module_id) {
                            create_skip_mount_marker(module_dir)?;
                            auto_skipped.insert(module_id.clone());
                            newly_marked.push(module_id.clone());
                        } else {
                            unknown_modules.push(module_id.clone());
                        }
                    }

                    if !already_marked.is_empty() {
                        log::debug!(
                            "[stage:recovery] already marked modules ignored: {}",
                            already_marked.join(", ")
                        );
                    }
                    if !unknown_modules.is_empty() {
                        log::error!(
                            "[stage:recovery] failed modules not found in module dir map: {}",
                            unknown_modules.join(", ")
                        );
                    }

                    if !newly_marked.is_empty() {
                        restart_round += 1;
                        log::warn!(
                            "[stage:recovery] marked skip_mount after {} failure: {}",
                            module_failure.stage,
                            newly_marked.join(", ")
                        );
                        if restart_round > max_restarts {
                            let loop_error = anyhow::anyhow!(
                                "Auto-recovery reached restart limit ({} rounds), aborting to avoid loop",
                                max_restarts
                            );
                            log::error!("[stage:recovery] {}", loop_error);
                            crate::core::inventory::model::update_crash_description(
                                &loop_error.to_string(),
                            );
                            return Err(loop_error);
                        }
                        log::info!(
                            "[stage:recovery] restarting daemon pipeline ({}/{})",
                            restart_round + 1,
                            max_restarts
                        );
                        continue;
                    }

                    log::error!(
                        "[stage:recovery] no newly marked modules for {} failure; aborting to avoid retry loop",
                        module_failure.stage
                    );
                }

                let err_msg = format!("{:#}", e).replace('\n', " -> ");
                log::error!("[stage:recovery] unrecoverable error: {}", err_msg);
                crate::core::inventory::model::update_crash_description(&err_msg);
                return Err(e);
            }
        }
    }
}

fn list_module_dirs(base: &Path) -> Result<std::collections::HashMap<String, PathBuf>> {
    let mut modules = std::collections::HashMap::new();
    if !base.exists() {
        return Ok(modules);
    }

    for entry in std::fs::read_dir(base)?.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let id = entry.file_name().to_string_lossy().to_string();
        if crate::core::inventory::is_reserved_module_dir(&id) {
            continue;
        }
        modules.insert(id, path);
    }

    Ok(modules)
}

fn create_skip_mount_marker(module_dir: &Path) -> Result<()> {
    let marker = module_dir.join(defs::SKIP_MOUNT_FILE_NAME);
    log::info!(
        "[stage:recovery] creating skip marker at {}",
        marker.display()
    );
    OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&marker)
        .with_context(|| format!("Failed to create {}", marker.display()))?;
    log::debug!(
        "[stage:recovery] skip marker ready for module dir {}",
        module_dir.display()
    );
    Ok(())
}

pub fn run(cli: Cli) -> Result<()> {
    if let Some(command) = &cli.command {
        return run_command(&cli, command);
    }

    run_daemon(&cli)
}
