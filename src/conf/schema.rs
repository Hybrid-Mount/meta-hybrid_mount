// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::{collections::HashMap, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::{
    defs,
    domain::{DefaultMode, ModuleRules},
};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum OverlayMode {
    Tmpfs,
    #[default]
    Ext4,
}

impl OverlayMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Tmpfs => "tmpfs",
            Self::Ext4 => "ext4",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct BlacklistConfig {
    pub blacklist: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CustomBindMount {
    pub source: PathBuf,
    pub target: PathBuf,
}

impl Default for CustomBindMount {
    fn default() -> Self {
        Self {
            source: PathBuf::new(),
            target: PathBuf::new(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub moduledir: PathBuf,
    pub mountsource: String,
    pub overlay_mode: OverlayMode,
    pub disable_umount: bool,
    pub default_mode: DefaultMode,
    #[serde(default)]
    pub rules: HashMap<String, ModuleRules>,
    #[serde(default)]
    pub custom_mounts: Vec<CustomBindMount>,
    #[serde(skip)]
    pub module_blacklist: Vec<String>,
}

fn default_moduledir() -> PathBuf {
    PathBuf::from(defs::MODULES_DIR)
}

fn default_mountsource() -> String {
    crate::sys::mount::detect_mount_source()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            moduledir: default_moduledir(),
            mountsource: default_mountsource(),
            overlay_mode: OverlayMode::default(),
            disable_umount: false,
            default_mode: DefaultMode::default(),
            rules: HashMap::new(),
            custom_mounts: Vec::new(),
            module_blacklist: Vec::new(),
        }
    }
}
