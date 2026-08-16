// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use anyhow::{Context, Result};

#[cfg(feature = "control-plane")]
use crate::{conf::cli::Cli, core::daemon};
use crate::{conf::loader, defs, sys, utils};

mod recovery;

#[cfg(feature = "control-plane")]
pub fn run(cli: &Cli) -> Result<()> {
    run_mount(cli).map(|_| ())
}

#[cfg(feature = "control-plane")]
pub fn run_and_serve(cli: &Cli) -> Result<()> {
    let config = run_mount(cli)?;
    daemon::serve(config)
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

    utils::check_ksu();

    let config = load_config()?;

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

    if let Ok(version) = std::fs::read_to_string("/proc/sys/kernel/osrelease") {
        crate::scoped_log!(debug, "startup", "kernel: version={}", version.trim());
    }

    if config.disable_umount {
        crate::scoped_log!(warn, "startup", "config: disable_umount=true");
    }

    recovery::run(config)
}
