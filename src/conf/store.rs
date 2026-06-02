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

use std::{fs, path::Path};

use anyhow::{Context, Result};

use crate::conf::schema::Config;

#[cfg(feature = "control-plane")]
fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("failed to create config directory")?;
    }
    Ok(())
}

fn load_merged_config(main_path: &Path) -> Result<Config> {
    crate::scoped_log!(
        debug,
        "conf:store:load_merged",
        "start: path={}",
        main_path.display()
    );

    let config = if main_path.exists() {
        let content = fs::read_to_string(main_path)
            .with_context(|| format!("failed to read config file {}", main_path.display()))?;
        toml::from_str::<Config>(&content)
            .with_context(|| format!("failed to parse config file {}", main_path.display()))?
    } else {
        crate::scoped_log!(
            debug,
            "conf:store:load_merged",
            "fallback: reason=config_missing, path={}",
            main_path.display()
        );
        Config::default()
    };

    crate::scoped_log!(
        debug,
        "conf:store:load_merged",
        "complete: path={}",
        main_path.display()
    );

    Ok(config)
}

impl Config {
    pub fn load_optional_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        load_merged_config(path.as_ref())
    }

    #[cfg(feature = "control-plane")]
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let main_path = path.as_ref();
        let content = toml::to_string_pretty(self).context("failed to serialize config")?;

        ensure_parent_dir(main_path)?;
        if main_path.exists() {
            let ext = main_path
                .extension()
                .map(|e| format!("{}.bak", e.to_string_lossy()))
                .unwrap_or_else(|| "bak".to_string());
            let backup_path = main_path.with_extension(&ext);
            fs::copy(main_path, &backup_path).with_context(|| {
                format!(
                    "failed to create config backup at {}",
                    backup_path.display()
                )
            })?;
        }
        fs::write(main_path, content)
            .with_context(|| format!("failed to write config file {}", main_path.display()))?;
        Ok(())
    }
}
