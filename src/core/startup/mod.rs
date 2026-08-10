// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use anyhow::{Context, Result};

#[cfg(feature = "control-plane")]
use crate::{conf::cli::Cli, core::daemon};
use crate::{conf::loader, defs, sys, utils};

#[cfg(feature = "control-plane")]
pub fn run(cli: &Cli) -> Result<()> {
    run_mount(cli).map(|_| ())
}

#[cfg(feature = "control-plane")]
pub fn run_and_serve(cli: &Cli) -> Result<()> {
    run_mount(cli)?;
    daemon::serve()
}

#[cfg(not(feature = "control-plane"))]
pub fn run_default() -> Result<()> {
    run_default_mount().map(|_| ())
}

#[cfg(feature = "control-plane")]
pub fn run_mount(cli: &Cli) -> Result<crate::conf::config::Config> {
    run_with_config_loader(|| loader::load_config(cli))
}

#[cfg(not(feature = "control-plane"))]
pub fn run_default_mount() -> Result<crate::conf::config::Config> {
    run_with_config_loader(loader::load_default_config)
}

fn run_with_config_loader<F>(load_config: F) -> Result<crate::conf::config::Config>
where
    F: FnOnce() -> Result<crate::conf::config::Config>,
{
    sys::fs::ensure_dir_exists(defs::RUN_DIR)
        .with_context(|| format!("Failed to create run directory: {}", defs::RUN_DIR))?;

    utils::init_logging().context("Failed to initialize logging")?;
    crate::scoped_log!(info, "startup", "init: daemon=hybrid-mount");

    let mount_timer = utils::StageTimer::start("startup", "mount_total");
    let preflight_timer = utils::StageTimer::start("startup", "preflight");
    utils::check_ksu();
    preflight_timer.finish();

    let config_timer = utils::StageTimer::start("startup", "config_load");
    let config = match load_config() {
        Ok(config) => config,
        Err(error) => {
            crate::scoped_log!(error, "startup", "config load failed: error={:#}", error);
            return Err(error);
        }
    };
    config_timer.finish();

    if matches!(std::env::var("KSU_LATE_LOAD").as_deref(), Ok("1")) {
        crate::scoped_log!(info, "startup", "mode: late_load=true");
        let unmounted = crate::core::late_load::detach_stale_mounts(&config)?;
        crate::scoped_log!(
            info,
            "startup",
            "late_load stale mounts detached: unmounted={}",
            unmounted
        );
    }

    #[cfg(feature = "kasumi")]
    if config.kasumi.enabled {
        let lkm_timer = utils::StageTimer::start("startup", "kasumi_lkm_autoload");
        let loaded = sys::lkm::autoload_if_needed(&config.kasumi)?;
        lkm_timer.finish();
        if loaded {
            crate::scoped_log!(
                info,
                "startup",
                "kasumi lkm autoload: loaded=true, dir={}",
                config.kasumi.lkm_dir.display()
            );
        }
    } else {
        crate::scoped_log!(debug, "startup", "kasumi disabled: skip_lkm_autoload=true");
    }

    if config.disable_umount {
        crate::scoped_log!(warn, "startup", "config: disable_umount=true");
    }

    let workspace_timer = utils::StageTimer::start("startup", "workspace_setup");
    let mnt_base = utils::get_mnt()?;
    sys::fs::ensure_dir_exists(&mnt_base)?;
    workspace_timer.finish();
    crate::core::MountController::new(config.clone(), &mnt_base)?
        .init_storage(&mnt_base)
        .context("Failed to initialize storage")?
        .scan_and_prepare_plan()
        .context("Failed to scan modules and prepare mount plan")?
        .execute()
        .context("Failed to execute mount plan")?
        .finalize()
        .context("Failed to finalize boot sequence")?;
    mount_timer.finish();

    Ok(config)
}
