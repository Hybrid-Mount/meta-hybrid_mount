// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::{
    defs,
    sys::fs::{atomic_write, xattr},
};

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct MountStatistics {
    #[serde(default)]
    pub total_mounts: usize,
    #[serde(default)]
    pub successful_mounts: usize,
    #[serde(default)]
    pub failed_mounts: usize,
    #[serde(default)]
    pub tmpfs_created: usize,
    #[serde(default)]
    pub files_mounted: usize,
    #[serde(default)]
    pub dirs_mounted: usize,
    #[serde(default)]
    pub symlinks_created: usize,
    #[serde(default)]
    pub overlayfs_mounts: usize,
    #[serde(default)]
    pub ignored_entries: usize,
}

impl MountStatistics {
    pub fn record_file(&mut self) {
        self.total_mounts += 1;
        self.successful_mounts += 1;
        self.files_mounted += 1;
    }

    pub fn record_dir(&mut self) {
        self.total_mounts += 1;
        self.successful_mounts += 1;
        self.dirs_mounted += 1;
    }

    pub fn record_symlink(&mut self) {
        self.total_mounts += 1;
        self.successful_mounts += 1;
        self.symlinks_created += 1;
    }

    pub fn record_failed(&mut self) {
        self.total_mounts += 1;
        self.failed_mounts += 1;
    }

    pub fn record_tmpfs(&mut self) {
        self.tmpfs_created += 1;
    }

    pub fn record_overlay_mount(&mut self) {
        self.total_mounts += 1;
        self.successful_mounts += 1;
        self.overlayfs_mounts += 1;
    }

    pub fn record_ignored(&mut self) {
        self.ignored_entries += 1;
    }

    pub fn success_rate(&self) -> f64 {
        if self.total_mounts == 0 {
            0.0
        } else {
            self.successful_mounts as f64 * 100.0 / self.total_mounts as f64
        }
    }

    pub fn merge(&mut self, other: &Self) {
        self.total_mounts += other.total_mounts;
        self.successful_mounts += other.successful_mounts;
        self.failed_mounts += other.failed_mounts;
        self.tmpfs_created += other.tmpfs_created;
        self.files_mounted += other.files_mounted;
        self.dirs_mounted += other.dirs_mounted;
        self.symlinks_created += other.symlinks_created;
        self.overlayfs_mounts += other.overlayfs_mounts;
        self.ignored_entries += other.ignored_entries;
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct HymoFsRuntimeInfo {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub available: bool,
    #[serde(default)]
    pub lkm_loaded: bool,
    #[serde(default)]
    pub lkm_autoload: bool,
    #[serde(default)]
    pub lkm_kmi_override: String,
    #[serde(default)]
    pub lkm_current_kmi: String,
    #[serde(default)]
    pub lkm_dir: PathBuf,
    #[serde(default)]
    pub protocol_version: Option<i32>,
    #[serde(default)]
    pub feature_bits: Option<i32>,
    #[serde(default)]
    pub feature_names: Vec<String>,
    #[serde(default)]
    pub hooks: Vec<String>,
    #[serde(default)]
    pub rule_count: usize,
    #[serde(default)]
    pub user_hide_rule_count: usize,
    #[serde(default)]
    pub mirror_path: PathBuf,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct RuntimeState {
    pub timestamp: u64,
    pub pid: u32,
    pub storage_mode: String,
    pub mount_point: PathBuf,
    pub overlay_modules: Vec<String>,
    pub magic_modules: Vec<String>,
    #[serde(default)]
    pub hymofs_modules: Vec<String>,
    #[serde(default)]
    pub mount_error_modules: Vec<String>,
    #[serde(default)]
    pub mount_error_reasons: BTreeMap<String, String>,
    #[serde(default)]
    pub skip_mount_modules: Vec<String>,
    #[serde(default)]
    pub active_mounts: Vec<String>,
    #[serde(default)]
    pub tmpfs_xattr_supported: bool,
    #[serde(default)]
    pub mount_stats: MountStatistics,
    #[serde(default)]
    pub hymofs: HymoFsRuntimeInfo,
    #[serde(default = "default_log_file")]
    pub log_file: PathBuf,
}

impl RuntimeState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        storage_mode: String,
        mount_point: PathBuf,
        overlay_modules: Vec<String>,
        magic_modules: Vec<String>,
        hymofs_modules: Vec<String>,
        active_mounts: Vec<String>,
        mount_stats: MountStatistics,
        hymofs: HymoFsRuntimeInfo,
        log_file: PathBuf,
    ) -> Self {
        let start = SystemTime::now();

        let timestamp = start
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let pid = std::process::id();

        let tmpfs_xattr_supported = xattr::is_overlay_xattr_supported().unwrap_or(false);

        Self {
            timestamp,
            pid,
            storage_mode,
            mount_point,
            overlay_modules,
            magic_modules,
            hymofs_modules,
            mount_error_modules: Vec::new(),
            mount_error_reasons: BTreeMap::new(),
            skip_mount_modules: Vec::new(),
            active_mounts,
            tmpfs_xattr_supported,
            mount_stats,
            hymofs,
            log_file,
        }
    }

    pub fn save(&self) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        atomic_write(defs::STATE_FILE, json.as_bytes())?;
        Ok(())
    }

    pub fn mounted_module_ids(&self) -> HashSet<&str> {
        self.overlay_modules
            .iter()
            .chain(self.magic_modules.iter())
            .chain(self.hymofs_modules.iter())
            .map(|s| s.as_str())
            .collect()
    }

    pub fn load() -> Result<Self> {
        if !std::path::Path::new(defs::STATE_FILE).exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(defs::STATE_FILE)?;
        let state = serde_json::from_str(&content)?;
        Ok(state)
    }
}

fn default_log_file() -> PathBuf {
    PathBuf::from(defs::DAEMON_LOG_FILE)
}
