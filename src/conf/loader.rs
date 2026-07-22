// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::path::Path;

use anyhow::{Context, Result};

#[cfg(feature = "control-plane")]
use crate::conf::cli::Cli;
use crate::{
    conf::{config::Config, schema::BlacklistConfig},
    defs,
};

pub(crate) fn load_module_blacklist(mut config: Config) -> Result<Config> {
    let path = Path::new(defs::MODULE_BLACKLIST_FILE);
    let blacklist = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read blacklist file {}", path.display()))
        .and_then(|content| {
            toml::from_str::<BlacklistConfig>(&content)
                .with_context(|| format!("failed to parse blacklist file {}", path.display()))
        })?;
    crate::scoped_log!(
        debug,
        "conf:loader",
        "blacklist loaded: path={}, entries={}",
        path.display(),
        blacklist.blacklist.len()
    );
    config.module_blacklist = blacklist.blacklist;

    Ok(config)
}

pub fn load_default_config() -> Result<Config> {
    let default_path = Path::new(defs::CONFIG_FILE);
    crate::scoped_log!(
        debug,
        "conf:loader",
        "start: mode=default, path={}",
        default_path.display()
    );
    let config = Config::load_from_file(default_path).with_context(|| {
        format!(
            "Failed to load config from default path: {}",
            default_path.display()
        )
    })?;

    let config = load_module_blacklist(config)?;

    crate::scoped_log!(
        debug,
        "conf:loader",
        "complete: mode=default, path={}",
        default_path.display()
    );

    Ok(config)
}

#[cfg(feature = "control-plane")]
pub fn load_config(cli: &Cli) -> Result<Config> {
    let config_path = &cli.config;
    crate::scoped_log!(
        debug,
        "conf:loader",
        "start: path={}",
        config_path.display()
    );

    let config = Config::load_from_file(config_path)
        .with_context(|| format!("Failed to load config from {}", config_path.display()))?;
    let config = load_module_blacklist(config)?;

    crate::scoped_log!(
        debug,
        "conf:loader",
        "complete: path={}",
        config_path.display()
    );

    Ok(config)
}
