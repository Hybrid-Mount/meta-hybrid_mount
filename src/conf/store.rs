// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::{fs, path::Path};

use anyhow::{Context, Result};

use crate::conf::schema::Config;
#[cfg(feature = "control-plane")]
use crate::sys::fs::atomic_write;

#[cfg(feature = "control-plane")]
const CONFIG_BACKUP_HISTORY: usize = 3;

#[cfg(feature = "control-plane")]
fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("failed to create config directory")?;
    }
    Ok(())
}

#[cfg(feature = "control-plane")]
fn config_backup_path(path: &Path, index: Option<usize>) -> std::path::PathBuf {
    let ext = path
        .extension()
        .map(|e| format!("{}.bak", e.to_string_lossy()))
        .unwrap_or_else(|| "bak".to_string());
    let ext = match index {
        Some(index) => format!("{ext}.{index}"),
        None => ext,
    };
    path.with_extension(ext)
}

#[cfg(feature = "control-plane")]
fn rotate_config_backups(path: &Path) -> Result<()> {
    for index in (1..CONFIG_BACKUP_HISTORY).rev() {
        let src = if index == 1 {
            config_backup_path(path, None)
        } else {
            config_backup_path(path, Some(index - 1))
        };
        if !src.exists() {
            continue;
        }

        let dst = config_backup_path(path, Some(index));
        if dst.exists() {
            fs::remove_file(&dst)
                .with_context(|| format!("failed to remove old config backup {}", dst.display()))?;
        }
        fs::rename(&src, &dst).with_context(|| {
            format!(
                "failed to rotate config backup from {} to {}",
                src.display(),
                dst.display()
            )
        })?;
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
            rotate_config_backups(main_path)?;
            let backup_path = config_backup_path(main_path, None);
            fs::copy(main_path, &backup_path).with_context(|| {
                format!(
                    "failed to create config backup at {}",
                    backup_path.display()
                )
            })?;
        }
        atomic_write(main_path, content)
            .with_context(|| format!("failed to write config file {}", main_path.display()))?;
        Ok(())
    }
}

#[cfg(all(test, feature = "control-plane"))]
mod tests {
    use super::*;

    #[test]
    fn save_to_file_replaces_existing_config_and_keeps_backup() {
        let temp = tempfile::tempdir().unwrap();
        let config_path = temp.path().join("config.toml");
        fs::write(&config_path, "default_mode = \"magic\"\n").unwrap();

        let config = Config::default();
        config.save_to_file(&config_path).unwrap();

        let saved = fs::read_to_string(&config_path).unwrap();
        assert!(saved.contains("default_mode = \"overlay\""));

        let backup = fs::read_to_string(temp.path().join("config.toml.bak")).unwrap();
        assert!(backup.contains("default_mode = \"magic\""));
    }

    #[test]
    fn save_to_file_rotates_existing_backups() {
        let temp = tempfile::tempdir().unwrap();
        let config_path = temp.path().join("config.toml");
        fs::write(&config_path, "default_mode = \"magic\"\n").unwrap();

        let config = Config::default();
        config.save_to_file(&config_path).unwrap();
        config.save_to_file(&config_path).unwrap();

        let latest_backup = fs::read_to_string(temp.path().join("config.toml.bak")).unwrap();
        assert!(latest_backup.contains("default_mode = \"overlay\""));

        let previous_backup = fs::read_to_string(temp.path().join("config.toml.bak.1")).unwrap();
        assert!(previous_backup.contains("default_mode = \"magic\""));
    }
}
