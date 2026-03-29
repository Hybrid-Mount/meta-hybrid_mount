// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    collections::HashMap,
    path::PathBuf,
};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::defs;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum OverlayMode {
    Tmpfs,
    #[default]
    Ext4,
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
    #[serde(default, deserialize_with = "deserialize_partitions_flexible")]
    pub partitions: Vec<String>,
    #[serde(default)]
    pub overlay_mode: OverlayMode,
    #[serde(default)]
    pub disable_umount: bool,
    #[serde(default)]
    pub allow_umount_coexistence: bool,
    #[serde(default)]
    pub default_mode: DefaultMode,
    #[serde(default)]
    pub rules: HashMap<String, ModuleRules>,
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
            partitions: Vec::new(),
            overlay_mode: OverlayMode::default(),
            disable_umount: false,
            allow_umount_coexistence: false,
            default_mode: DefaultMode::default(),
            rules: HashMap::new(),
        }
    }
}

impl Config {
    pub fn merge_with_cli(
        &mut self,
        moduledir: Option<PathBuf>,
        mountsource: Option<String>,
        partitions: Vec<String>,
    ) {
        if let Some(dir) = moduledir {
            self.moduledir = dir;
        }

        if let Some(source) = mountsource {
            self.mountsource = source;
        }

        if !partitions.is_empty() {
            self.partitions = partitions;
        }
    }
}
