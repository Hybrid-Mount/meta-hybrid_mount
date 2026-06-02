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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KasumiMapsRuleConfig {
    #[serde(default)]
    pub target_ino: u64,
    #[serde(default)]
    pub target_dev: u64,
    #[serde(default)]
    pub spoofed_ino: u64,
    #[serde(default)]
    pub spoofed_dev: u64,
    #[serde(default)]
    pub spoofed_pathname: PathBuf,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct KasumiKstatRuleConfig {
    pub target_ino: u64,
    pub target_pathname: PathBuf,
    pub spoofed_ino: u64,
    pub spoofed_dev: u64,
    pub spoofed_nlink: u32,
    pub spoofed_size: i64,
    pub spoofed_atime_sec: i64,
    pub spoofed_atime_nsec: i64,
    pub spoofed_mtime_sec: i64,
    pub spoofed_mtime_nsec: i64,
    pub spoofed_ctime_sec: i64,
    pub spoofed_ctime_nsec: i64,
    pub spoofed_blksize: u64,
    pub spoofed_blocks: u64,
    pub is_static: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct KasumiUnameConfig {
    pub sysname: String,
    pub nodename: String,
    pub release: String,
    pub version: String,
    pub machine: String,
    pub domainname: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum KasumiUnameMode {
    #[default]
    Scoped,
    Global,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct KasumiMountHideConfig {
    pub enabled: bool,
    pub path_pattern: PathBuf,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct KasumiStatfsSpoofConfig {
    pub enabled: bool,
    pub path: PathBuf,
    pub spoof_f_type: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct KasumiConfig {
    pub enabled: bool,
    pub lkm_autoload: bool,
    pub lkm_dir: PathBuf,
    pub lkm_kmi_override: String,
    pub mirror_path: PathBuf,
    pub enable_kernel_debug: bool,
    pub enable_stealth: bool,
    pub enable_hidexattr: bool,
    pub enable_mount_hide: bool,
    pub enable_maps_spoof: bool,
    pub enable_statfs_spoof: bool,
    pub enable_selinux_fix: bool,
    pub mount_hide: KasumiMountHideConfig,
    pub statfs_spoof: KasumiStatfsSpoofConfig,
    pub hide_uids: Vec<u32>,
    pub uname_mode: KasumiUnameMode,
    pub uname: KasumiUnameConfig,
    pub cmdline_value: String,
    pub kstat_rules: Vec<KasumiKstatRuleConfig>,
    pub maps_rules: Vec<KasumiMapsRuleConfig>,
}

impl Default for KasumiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            lkm_autoload: true,
            lkm_dir: PathBuf::from(defs::KASUMI_LKM_DIR),
            lkm_kmi_override: String::new(),
            mirror_path: PathBuf::from(defs::KASUMI_MIRROR_DIR),
            enable_kernel_debug: false,
            enable_stealth: false,
            enable_hidexattr: false,
            enable_mount_hide: false,
            enable_maps_spoof: false,
            enable_statfs_spoof: false,
            enable_selinux_fix: false,
            mount_hide: KasumiMountHideConfig::default(),
            statfs_spoof: KasumiStatfsSpoofConfig::default(),
            hide_uids: Vec::new(),
            uname_mode: KasumiUnameMode::Scoped,
            uname: KasumiUnameConfig::default(),
            cmdline_value: String::new(),
            kstat_rules: Vec::new(),
            maps_rules: Vec::new(),
        }
    }
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
    #[serde(skip_serializing_if = "is_kasumi_default")]
    pub kasumi: KasumiConfig,
    pub rules: HashMap<String, ModuleRules>,
    pub daemon_startup_mode: DaemonStartupMode,
    #[serde(skip)]
    pub module_blacklist: Vec<String>,
}

fn is_kasumi_default(_kasumi: &KasumiConfig) -> bool {
    // In lite/nano builds the kasumi feature is not compiled in, so the
    // kasumi config section must never appear in any JSON response sent
    // to the WebUI or API consumers.
    !cfg!(feature = "kasumi")
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
            kasumi: KasumiConfig::default(),
            rules: HashMap::new(),
            daemon_startup_mode: DaemonStartupMode::default(),
            module_blacklist: Vec::new(),
        }
    }
}
