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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum DaemonStartupMode {
    #[default]
    OnDemand,
    Persistent,
}

impl std::fmt::Display for DaemonStartupMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OnDemand => write!(f, "on-demand"),
            Self::Persistent => write!(f, "persistent"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct BlacklistConfig {
    pub blacklist: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Config {
    pub moduledir: PathBuf,
    pub mountsource: String,
    pub overlay_mode: OverlayMode,
    pub disable_umount: bool,
    pub default_mode: DefaultMode,
    pub rules: HashMap<String, ModuleRules>,
    pub daemon_startup_mode: DaemonStartupMode,
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
            daemon_startup_mode: DaemonStartupMode::default(),
            module_blacklist: Vec::new(),
        }
    }
}
