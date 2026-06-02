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

use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[cfg(feature = "kasumi")]
use crate::mount::kasumi;
#[cfg(feature = "control-plane")]
use crate::sys::fs::xattr;
use crate::{
    conf::config::Config, core::ops::executor::ExecutionResult, defs, sys::fs::atomic_write, utils,
};

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
#[serde(default)]
pub struct MountStatistics {
    pub total_mounts: usize,
    pub successful_mounts: usize,
    pub failed_mounts: usize,
    pub tmpfs_created: usize,
    pub files_mounted: usize,
    pub dirs_mounted: usize,
    pub symlinks_created: usize,
    pub overlayfs_mounts: usize,
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

    #[cfg(feature = "control-plane")]
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

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
#[serde(default)]
pub struct ModuleModeStats {
    pub overlayfs: usize,
    pub magicmount: usize,
    pub kasumi: usize,
    pub blacklisted: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct KasumiRuntimeInfo {
    pub status: String,
    pub available: bool,
    pub kernel_supported: bool,
    pub lkm_loaded: bool,
    pub lkm_autoload: bool,
    pub lkm_kmi_override: String,
    pub lkm_current_kmi: String,
    pub lkm_dir: PathBuf,
    pub protocol_version: Option<i32>,
    pub feature_bits: Option<i32>,
    pub feature_names: Vec<String>,
    pub hooks: Vec<String>,
    pub rule_count: usize,
    pub user_hide_rule_count: usize,
    pub mirror_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DaemonRuntimeInfo {
    pub alive: bool,
    pub socket_path: String,
    pub last_refresh_ts: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RuntimeState {
    pub timestamp: u64,
    pub pid: u32,
    pub storage_mode: String,
    pub mount_point: PathBuf,
    pub overlay_modules: Vec<String>,
    pub magic_modules: Vec<String>,
    #[serde(default)]
    pub kasumi_modules: Vec<String>,
    #[serde(default)]
    pub mount_error_modules: Vec<String>,
    #[serde(default)]
    pub mount_error_reasons: BTreeMap<String, String>,
    #[serde(default)]
    pub skip_mount_modules: Vec<String>,
    #[serde(default)]
    pub blacklisted_modules: Vec<String>,
    #[serde(default)]
    pub active_mounts: Vec<String>,
    #[cfg(feature = "control-plane")]
    #[serde(default)]
    pub tmpfs_xattr_supported: bool,
    #[serde(default)]
    pub mount_stats: MountStatistics,
    #[serde(default)]
    pub mode_stats: ModuleModeStats,
    #[serde(default)]
    pub kasumi: KasumiRuntimeInfo,
    #[serde(default)]
    pub daemon: DaemonRuntimeInfo,
    #[serde(skip)]
    cached_status_value: Option<serde_json::Value>,
}

impl RuntimeState {
    #[cfg(feature = "control-plane")]
    pub fn status_value(&mut self) -> serde_json::Result<&serde_json::Value> {
        if self.cached_status_value.is_none() {
            self.cached_status_value = Some(serde_json::to_value(&*self)?);
        }
        Ok(self
            .cached_status_value
            .as_ref()
            .expect("cached_status_value was just populated above"))
    }

    fn invalidate_cache(&mut self) {
        self.cached_status_value = None;
    }
}

impl RuntimeState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        storage_mode: String,
        mount_point: PathBuf,
        overlay_modules: Vec<String>,
        magic_modules: Vec<String>,
        kasumi_modules: Vec<String>,
        active_mounts: Vec<String>,
        mount_stats: MountStatistics,
        mode_stats: ModuleModeStats,
        kasumi: KasumiRuntimeInfo,
    ) -> Self {
        let start = SystemTime::now();

        let timestamp = start
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let pid = std::process::id();

        #[cfg(feature = "control-plane")]
        let tmpfs_xattr_supported = xattr::is_overlay_xattr_supported().unwrap_or(false);

        let state = Self {
            timestamp,
            pid,
            storage_mode,
            mount_point,
            overlay_modules,
            magic_modules,
            kasumi_modules,
            mount_error_modules: Vec::new(),
            mount_error_reasons: BTreeMap::new(),
            skip_mount_modules: Vec::new(),
            blacklisted_modules: Vec::new(),
            active_mounts,
            #[cfg(feature = "control-plane")]
            tmpfs_xattr_supported,
            mount_stats,
            mode_stats,
            kasumi,
            daemon: DaemonRuntimeInfo::default(),
            cached_status_value: None,
        };

        #[cfg(feature = "control-plane")]
        crate::scoped_log!(
            debug,
            "runtime_state:new",
            "complete: storage_mode={}, mount_point={}, overlay_modules={}, magic_modules={}, kasumi_modules={}, active_mounts={}, tmpfs_xattr_supported={}",
            state.storage_mode,
            state.mount_point.display(),
            state.overlay_modules.len(),
            state.magic_modules.len(),
            state.kasumi_modules.len(),
            state.active_mounts.len(),
            state.tmpfs_xattr_supported
        );
        #[cfg(not(feature = "control-plane"))]
        crate::scoped_log!(
            debug,
            "runtime_state:new",
            "complete: storage_mode={}, mount_point={}, overlay_modules={}, magic_modules={}, kasumi_modules={}, active_mounts={}",
            state.storage_mode,
            state.mount_point.display(),
            state.overlay_modules.len(),
            state.magic_modules.len(),
            state.kasumi_modules.len(),
            state.active_mounts.len()
        );

        state
    }

    pub fn save(&self) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        if let Ok(existing) = std::fs::read_to_string(defs::STATE_FILE)
            && existing == json
        {
            return Ok(());
        }
        crate::scoped_log!(
            debug,
            "runtime_state:save",
            "start: path={}",
            defs::STATE_FILE
        );
        atomic_write(defs::STATE_FILE, json.as_bytes())?;
        crate::scoped_log!(
            debug,
            "runtime_state:save",
            "complete: path={}, bytes={}",
            defs::STATE_FILE,
            json.len()
        );
        if self.mount_error_modules.is_empty() {
            crate::scoped_log!(
                info,
                "runtime_state:summary",
                "saved: storage_mode={}, active_mounts={}, kasumi_modules={}, mount_errors=0, daemon_alive={}",
                self.storage_mode,
                self.active_mounts.join(","),
                self.kasumi_modules.join(","),
                self.daemon.alive
            );
        } else {
            crate::scoped_log!(
                warn,
                "runtime_state:summary",
                "saved: storage_mode={}, active_mounts={}, kasumi_modules={}, mount_errors={}, reasons={:?}, daemon_alive={}",
                self.storage_mode,
                self.active_mounts.join(","),
                self.kasumi_modules.join(","),
                self.mount_error_modules.join(","),
                self.mount_error_reasons,
                self.daemon.alive
            );
        }
        Ok(())
    }

    pub fn build_from_execution(
        config: &Config,
        storage_mode: crate::core::storage::StorageMode,
        mount_point: &Path,
        result: &ExecutionResult,
    ) -> Self {
        crate::scoped_log!(
            debug,
            "runtime_state:build",
            "start: storage_mode={}, mount_point={}, overlay_modules={}, magic_modules={}, kasumi_modules={}",
            storage_mode.as_str(),
            mount_point.display(),
            result.overlay_module_ids.len(),
            result.magic_module_ids.len(),
            result.kasumi_count()
        );

        let previous_state = match Self::load() {
            Ok(state) => state,
            Err(err) => {
                crate::scoped_log!(
                    warn,
                    "runtime_state:build",
                    "fallback: reason=load_previous_failed, error={:#}",
                    err
                );
                Self::default()
            }
        };

        #[cfg(feature = "kasumi")]
        let kasumi = kasumi::collect_runtime_info(config);
        #[cfg(not(feature = "kasumi"))]
        let kasumi = {
            let _ = config;
            KasumiRuntimeInfo::default()
        };
        let mut state = Self::new(
            storage_mode.as_str().to_owned(),
            mount_point.to_path_buf(),
            result.overlay_module_ids.clone(),
            result.magic_module_ids.clone(),
            {
                #[cfg(feature = "kasumi")]
                {
                    result.kasumi_module_ids.clone()
                }
                #[cfg(not(feature = "kasumi"))]
                {
                    Vec::new()
                }
            },
            collect_active_mounts(result),
            result.mount_stats.clone(),
            collect_mode_stats(result),
            kasumi,
        );
        state.mount_error_modules = previous_state.mount_error_modules;
        state.mount_error_reasons = previous_state.mount_error_reasons;
        clear_recovered_mount_errors(&mut state);
        state.skip_mount_modules = collect_skip_mount_modules(config);
        state.blacklisted_modules = collect_blacklisted_modules(config);
        state.mode_stats.blacklisted = state.blacklisted_modules.len();
        state.daemon = previous_state.daemon;
        state.invalidate_cache();

        crate::scoped_log!(
            debug,
            "runtime_state:build",
            "complete: mount_errors={}, skip_mount_modules={}, active_mounts={}",
            state.mount_error_modules.len(),
            state.skip_mount_modules.len(),
            state.active_mounts.len()
        );

        state
    }

    pub fn mounted_module_ids(&self) -> HashSet<&str> {
        self.overlay_modules
            .iter()
            .chain(self.magic_modules.iter())
            .chain(self.kasumi_modules.iter())
            .map(|s| s.as_str())
            .collect()
    }

    #[cfg(feature = "control-plane")]
    pub fn set_daemon_state(&mut self, alive: bool, socket_path: impl Into<String>) {
        let refreshed_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.daemon.alive = alive;
        self.daemon.socket_path = socket_path.into();
        self.daemon.last_refresh_ts = refreshed_at;
        self.invalidate_cache();
    }

    pub fn load() -> Result<Self> {
        crate::scoped_log!(
            debug,
            "runtime_state:load",
            "start: path={}",
            defs::STATE_FILE
        );
        if !std::path::Path::new(defs::STATE_FILE).exists() {
            crate::scoped_log!(
                debug,
                "runtime_state:load",
                "fallback: reason=state_file_missing, path={}",
                defs::STATE_FILE
            );
            return Ok(Self::default());
        }
        let content = fs::read_to_string(defs::STATE_FILE)?;
        let state = serde_json::from_str(&content)?;
        crate::scoped_log!(
            debug,
            "runtime_state:load",
            "complete: path={}, bytes={}",
            defs::STATE_FILE,
            content.len()
        );
        Ok(state)
    }
}

fn collect_mode_stats(result: &ExecutionResult) -> ModuleModeStats {
    ModuleModeStats {
        overlayfs: result.overlay_module_ids.len(),
        magicmount: result.magic_module_ids.len(),
        kasumi: result.kasumi_count(),
        blacklisted: 0usize,
    }
}

fn collect_active_mounts(result: &ExecutionResult) -> Vec<String> {
    let mut active_mounts = result.overlay_partitions.clone();

    if result.kasumi_runtime_enabled {
        active_mounts.push("kasumi".to_string());
    }

    active_mounts.sort();
    active_mounts.dedup();

    crate::scoped_log!(
        debug,
        "runtime_state:active_mounts",
        "complete: overlay_partitions={}, kasumi_runtime_enabled={}, active_mounts={}",
        result.overlay_partitions.len(),
        result.kasumi_runtime_enabled,
        active_mounts.len()
    );

    active_mounts
}

fn collect_skip_mount_modules(config: &Config) -> Vec<String> {
    let mut modules = Vec::new();
    let Ok(entries) = fs::read_dir(&config.moduledir) else {
        crate::scoped_log!(
            debug,
            "runtime_state:skip_modules",
            "skip: reason=moduledir_unreadable, path={}",
            config.moduledir.display()
        );
        return modules;
    };

    for entry in entries.flatten() {
        let module_dir = entry.path();
        if !module_dir.is_dir() {
            continue;
        }
        let id = entry.file_name().to_string_lossy().into_owned();
        if crate::core::inventory::is_reserved_module_dir(&id) {
            continue;
        }
        if utils::dir_contains_entry_case_insensitive(&module_dir, defs::SKIP_MOUNT_FILE_NAME) {
            modules.push(id);
        }
    }

    modules.sort();

    crate::scoped_log!(
        debug,
        "runtime_state:skip_modules",
        "complete: moduledir={}, modules={}",
        config.moduledir.display(),
        modules.len()
    );

    modules
}

fn collect_blacklisted_modules(config: &Config) -> Vec<String> {
    let mut modules: Vec<String> = config
        .module_blacklist
        .iter()
        .filter(|id| {
            let module_dir = config.moduledir.join(id);
            module_dir.is_dir()
        })
        .cloned()
        .collect();
    modules.sort();

    crate::scoped_log!(
        debug,
        "runtime_state:blacklisted",
        "complete: moduledir={}, modules={}",
        config.moduledir.display(),
        modules.len()
    );

    modules
}

fn clear_recovered_mount_errors(state: &mut RuntimeState) {
    let mounted: HashSet<String> = state
        .mounted_module_ids()
        .into_iter()
        .map(|s| s.to_owned())
        .collect();
    state
        .mount_error_modules
        .retain(|module_id| !mounted.contains(module_id));
    state
        .mount_error_reasons
        .retain(|module_id, _| !mounted.contains(module_id));
}
