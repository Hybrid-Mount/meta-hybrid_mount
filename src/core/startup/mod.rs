// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

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

    if let Ok(version) = std::fs::read_to_string("/proc/sys/kernel/osrelease") {
        crate::scoped_log!(debug, "startup", "kernel: version={}", version.trim());
    }

    #[cfg(feature = "kasumi")]
    if config.kasumi.enabled {
        match sys::lkm::autoload_if_needed(&config.kasumi) {
            Ok(true) => {
                crate::scoped_log!(
                    info,
                    "startup",
                    "kasumi lkm autoload: loaded=true, dir={}",
                    config.kasumi.lkm_dir.display()
                );
            }
            Ok(false) => {
                crate::scoped_log!(
                    debug,
                    "startup",
                    "kasumi lkm autoload: loaded=false, reason=not_needed"
                );
            }
            Err(err) => {
                crate::scoped_log!(
                    warn,
                    "startup",
                    "kasumi lkm autoload failed: error={:#}",
                    err
                );
            }
        }
    } else {
        crate::scoped_log!(debug, "startup", "kasumi disabled: skip_lkm_autoload=true");
    }

    if config.disable_umount {
        crate::scoped_log!(warn, "startup", "config: disable_umount=true");
    }

    recovery::run(config)
}
