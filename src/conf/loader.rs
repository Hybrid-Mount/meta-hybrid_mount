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

use std::path::Path;

use anyhow::{Context, Result};

#[cfg(feature = "control-plane")]
use crate::conf::cli::Cli;
use crate::{
    conf::{config::Config, schema::BlacklistConfig},
    defs,
};

fn load_module_blacklist(mut config: Config) -> Config {
    let path = Path::new(defs::MODULE_BLACKLIST_FILE);
    if !path.exists() {
        return config;
    }

    match std::fs::read_to_string(path)
        .with_context(|| format!("failed to read blacklist file {}", path.display()))
        .and_then(|content| {
            toml::from_str::<BlacklistConfig>(&content)
                .with_context(|| format!("failed to parse blacklist file {}", path.display()))
        }) {
        Ok(bl) => {
            crate::scoped_log!(
                debug,
                "conf:loader",
                "blacklist loaded: path={}, entries={}",
                path.display(),
                bl.blacklist.len()
            );
            config.module_blacklist = bl.blacklist;
        }
        Err(err) => {
            crate::scoped_log!(
                warn,
                "conf:loader",
                "blacklist parse failed, ignoring: path={}, error={:#}",
                path.display(),
                err
            );
        }
    }

    config
}

pub fn load_default_config() -> Result<Config> {
    let default_path = Path::new(defs::CONFIG_FILE);
    crate::scoped_log!(
        debug,
        "conf:loader",
        "start: mode=default, path={}",
        default_path.display()
    );
    if !default_path.exists() {
        crate::scoped_log!(
            debug,
            "conf:loader",
            "fallback: mode=default, reason=config_missing, path={}",
            default_path.display()
        );
        return Ok(load_module_blacklist(Config::default()));
    }

    let config = Config::load_optional_from_file(default_path).with_context(|| {
        format!(
            "Failed to load config from default path: {}",
            default_path.display()
        )
    })?;

    let config = load_module_blacklist(config);

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
    if let Some(config_path) = &cli.config {
        crate::scoped_log!(
            debug,
            "conf:loader",
            "start: mode=custom, path={}",
            config_path.display()
        );

        let config = Config::load_optional_from_file(config_path).with_context(|| {
            format!(
                "Failed to load config from custom path: {}",
                config_path.display()
            )
        })?;

        let config = load_module_blacklist(config);

        crate::scoped_log!(
            debug,
            "conf:loader",
            "complete: mode=custom, path={}",
            config_path.display()
        );

        return Ok(config);
    }

    load_default_config()
}
