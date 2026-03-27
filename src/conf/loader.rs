// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::{Context, Result};

use crate::{
    conf::{cli::Cli, config::Config},
    defs,
};

#[derive(Clone, Copy, Debug)]
pub enum LoadPolicy {
    FallbackToDefault,
    ErrorOnInvalidDefault,
}

pub fn load_config(cli: &Cli, policy: LoadPolicy) -> Result<Config> {
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

            match policy {
                LoadPolicy::FallbackToDefault => {
                    if !is_not_found {
                        log::warn!("Failed to load default config, using defaults: {:#}", e);
                    }
                    Ok(Config::default())
                }
                LoadPolicy::ErrorOnInvalidDefault => {
                    if is_not_found {
                        Ok(Config::default())
                    } else {
                        Err(e).context(format!(
                            "Failed to load default config from {}",
                            defs::CONFIG_FILE
                        ))
                    }
                }
            }
        }
    }
}
