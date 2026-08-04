// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::{collections::HashMap, path::PathBuf};

use serde::{Deserialize, Serialize};

#[cfg(not(feature = "kasumi"))]
use crate::domain::MountMode;
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
    pub target_ino: u64,
    pub target_dev: u64,
    pub spoofed_ino: u64,
    pub spoofed_dev: u64,
    pub spoofed_pathname: PathBuf,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
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
    pub enable_overlay_xattr_hide: bool,
    #[serde(default, rename = "enable_hidexattr", skip_serializing)]
    pub legacy_enable_hidexattr: bool,
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
            enable_overlay_xattr_hide: false,
            legacy_enable_hidexattr: false,
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

impl KasumiConfig {
    pub(crate) fn normalize_legacy_fields(&mut self) {
        if self.legacy_enable_hidexattr {
            self.enable_stealth = true;
            self.enable_overlay_xattr_hide = true;
            self.enable_mount_hide = true;
            self.enable_maps_spoof = true;
            self.enable_statfs_spoof = true;
            self.legacy_enable_hidexattr = false;
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
#[serde(default)]
pub struct Config {
    pub moduledir: PathBuf,
    pub mountsource: String,
    pub overlay_mode: OverlayMode,
    pub disable_umount: bool,
    pub default_mode: DefaultMode,
    #[serde(default, skip_serializing_if = "kasumi_feature_disabled")]
    pub kasumi: KasumiConfig,
    pub rules: HashMap<String, ModuleRules>,
    #[serde(alias = "customMounts")]
    pub custom_mounts: Vec<CustomBindMount>,
    #[serde(skip)]
    pub module_blacklist: Vec<String>,
}

impl Config {
    pub(crate) fn sanitize_disabled_features(&mut self) {
        #[cfg(not(feature = "kasumi"))]
        {
            self.kasumi = KasumiConfig::default();
            if matches!(self.default_mode, DefaultMode::Kasumi) {
                self.default_mode = DefaultMode::Magic;
            }
            for rules in self.rules.values_mut() {
                if matches!(rules.default_mode, MountMode::Kasumi) {
                    rules.default_mode = MountMode::Magic;
                }
                for mode in rules.paths.values_mut() {
                    if matches!(mode, MountMode::Kasumi) {
                        *mode = MountMode::Magic;
                    }
                }
            }
        }
    }
}

fn kasumi_feature_disabled(_config: &KasumiConfig) -> bool {
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
            custom_mounts: Vec::new(),
            module_blacklist: Vec::new(),
        }
    }
}
