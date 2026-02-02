// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::defs;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BackupConfig {
    #[serde(default = "default_max_backups")]
    pub max_backups: usize,
    #[serde(default = "default_retention_days")]
    pub retention_days: u64,
}

fn default_max_backups() -> usize {
    20
}

fn default_retention_days() -> u64 {
    0
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            max_backups: default_max_backups(),
            retention_days: default_retention_days(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum OverlayMode {
    #[default]
    Tmpfs,
    Ext4,
    Erofs,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum DefaultMode {
    #[default]
    Overlay,
    Magic,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum MountMode {
    #[default]
    Overlay,
    Magic,
    Ignore,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModuleRules {
    #[serde(default)]
    pub default_mode: MountMode,
    #[serde(default)]
    pub paths: HashMap<String, MountMode>,
}

impl ModuleRules {
    pub fn get_mode(&self, relative_path: &str) -> MountMode {
        if let Some(mode) = self.paths.get(relative_path) {
            return mode.clone();
        }
        self.default_mode.clone()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    #[serde(default = "default_moduledir")]
    pub moduledir: PathBuf,
    #[serde(default = "default_mountsource")]
    pub mountsource: String,
    pub verbose: bool,
    #[serde(default, deserialize_with = "deserialize_partitions_flexible")]
    pub partitions: Vec<String>,
    #[serde(default)]
    pub overlay_mode: OverlayMode,
    #[serde(default)]
    pub disable_umount: bool,
    #[serde(default)]
    pub allow_umount_coexistence: bool,
    #[serde(default, alias = "granary")]
    pub backup: BackupConfig,
    #[serde(default = "default_hybrid_mnt_dir")]
    pub hybrid_mnt_dir: String,
    #[serde(default)]
    pub default_mode: DefaultMode,
    #[serde(default)]
    pub rules: HashMap<String, ModuleRules>,
}

fn default_hybrid_mnt_dir() -> String {
    defs::DEFAULT_HYBRID_MNT_DIR.to_string()
}

fn default_moduledir() -> PathBuf {
    PathBuf::from(defs::MODULES_DIR)
}

fn default_mountsource() -> String {
    crate::sys::mount::detect_mount_source()
}

fn deserialize_partitions_flexible<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrVec {
        String(String),
        Vec(Vec<String>),
    }

    match StringOrVec::deserialize(deserializer)? {
        StringOrVec::Vec(v) => Ok(v),
        StringOrVec::String(s) => Ok(s
            .split(',')
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .collect()),
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            moduledir: default_moduledir(),
            mountsource: default_mountsource(),
            verbose: false,
            partitions: Vec::new(),
            overlay_mode: OverlayMode::default(),
            disable_umount: false,
            allow_umount_coexistence: false,
            backup: BackupConfig::default(),
            hybrid_mnt_dir: default_hybrid_mnt_dir(),
            default_mode: DefaultMode::default(),
            rules: HashMap::new(),
        }
    }
}

impl Config {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path.as_ref()).context("failed to read config file")?;

        let config: Config = toml::from_str(&content).context("failed to parse config file")?;

        Ok(config)
    }

    pub fn load_default() -> Result<Self> {
        Self::from_file(defs::CONFIG_FILE)
    }

    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = toml::to_string_pretty(self).context("failed to serialize config")?;

        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent).context("failed to create config directory")?;
        }

        fs::write(path.as_ref(), content).context("failed to write config file")?;

        Ok(())
    }

    pub fn merge_with_cli(
        &mut self,
        moduledir: Option<PathBuf>,
        mountsource: Option<String>,
        verbose: bool,
        partitions: Vec<String>,
    ) {
        if let Some(dir) = moduledir {
            self.moduledir = dir;
        }

        if let Some(source) = mountsource {
            self.mountsource = source;
        }

        if verbose {
            self.verbose = true;
        }

        if !partitions.is_empty() {
            self.partitions = partitions;
        }
    }
}
