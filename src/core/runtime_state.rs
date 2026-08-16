// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[cfg(feature = "control-plane")]
use crate::sys::fs::xattr;
use crate::{
    conf::config::Config,
    core::{inventory::InventorySummary, ops::executor::ExecutionResult},
    defs,
    sys::fs::atomic_write,
};

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
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
pub struct ModuleModeStats {
    pub overlayfs: usize,
    pub magicmount: usize,
    pub blacklisted: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
    /// Observable storage base after finalization. A temporary mount workspace
    /// is only retained here when cleanup is explicitly disabled.
    pub mount_point: PathBuf,
    /// True once a mount run has completed. Daemons that start without
    /// persisted state serve an idle state with this set to false.
    #[serde(default = "default_mounted")]
    pub mounted: bool,
    pub overlay_modules: Vec<String>,
    pub magic_modules: Vec<String>,
    /// Exact mount targets created by Magic Mount during the successful run.
    /// Late-load cleanup uses these instead of inferring ownership from a
    /// shared KernelSU/APatch mount source.
    #[serde(default)]
    pub magic_mount_targets: Vec<String>,
    #[serde(default)]
    pub custom_mounts: Vec<String>,
    pub skip_mount_modules: Vec<String>,
    pub blacklisted_modules: Vec<String>,
    pub active_mounts: Vec<String>,
    #[cfg(feature = "control-plane")]
    pub tmpfs_xattr_supported: bool,
    pub mount_stats: MountStatistics,
    pub mode_stats: ModuleModeStats,
    pub daemon: DaemonRuntimeInfo,
    #[serde(skip)]
    cached_status_value: Option<serde_json::Value>,
}

fn default_mounted() -> bool {
    true
}

fn runtime_mount_point_after_finalize(config: &Config, staging_path: &Path) -> PathBuf {
    if config.disable_umount || !crate::utils::is_mount_workspace_path(staging_path) {
        staging_path.to_path_buf()
    } else {
        Path::new(defs::CONFIG_FILE)
            .parent()
            .expect("config file must live below the persistent data root")
            .to_path_buf()
    }
}

#[cfg(feature = "control-plane")]
fn tmpfs_xattr_support_for_status(probe: Result<bool>) -> bool {
    match probe {
        Ok(supported) => supported,
        Err(error) => {
            crate::scoped_log!(
                warn,
                "runtime_state:xattr",
                "probe failed, reporting unsupported: error={:#}",
                error
            );
            false
        }
    }
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
        magic_mount_targets: Vec<String>,
        custom_mounts: Vec<String>,
        active_mounts: Vec<String>,
        mount_stats: MountStatistics,
        mode_stats: ModuleModeStats,
    ) -> Result<Self> {
        let start = SystemTime::now();

        let timestamp = start
            .duration_since(UNIX_EPOCH)
            .map_err(|err| anyhow::anyhow!("system clock is before the Unix epoch: {err}"))?
            .as_secs();

        let pid = std::process::id();

        #[cfg(feature = "control-plane")]
        let tmpfs_xattr_supported =
            tmpfs_xattr_support_for_status(xattr::is_overlay_xattr_supported());

        let state = Self {
            timestamp,
            pid,
            storage_mode,
            mount_point,
            mounted: true,
            overlay_modules,
            magic_modules,
            magic_mount_targets,
            custom_mounts,
            skip_mount_modules: Vec::new(),
            blacklisted_modules: Vec::new(),
            active_mounts,
            #[cfg(feature = "control-plane")]
            tmpfs_xattr_supported,
            mount_stats,
            mode_stats,
            daemon: DaemonRuntimeInfo::default(),
            cached_status_value: None,
        };

        #[cfg(feature = "control-plane")]
        crate::scoped_log!(
            debug,
            "runtime_state:new",
            "complete: storage_mode={}, mount_point={}, overlay_modules={}, magic_modules={}, active_mounts={}, tmpfs_xattr_supported={}",
            state.storage_mode,
            state.mount_point.display(),
            state.overlay_modules.len(),
            state.magic_modules.len(),
            state.active_mounts.len(),
            state.tmpfs_xattr_supported
        );
        #[cfg(not(feature = "control-plane"))]
        crate::scoped_log!(
            debug,
            "runtime_state:new",
            "complete: storage_mode={}, mount_point={}, overlay_modules={}, magic_modules={}, active_mounts={}",
            state.storage_mode,
            state.mount_point.display(),
            state.overlay_modules.len(),
            state.magic_modules.len(),
            state.active_mounts.len()
        );

        Ok(state)
    }

    /// Idle state for a daemon that started without a persisted mount run.
    /// Uses the configured overlay mode and the data directory as mount base
    /// so the WebUI can show "not mounted" instead of failing validation.
    pub fn idle(storage_mode: &str, mount_point: PathBuf) -> Self {
        Self {
            storage_mode: storage_mode.to_owned(),
            mount_point,
            mounted: false,
            ..Self::default()
        }
    }

    /// True when the state can be served to the WebUI: a supported storage
    /// mode and a non-empty mount base.
    pub fn has_valid_mount_identity(&self) -> bool {
        matches!(self.storage_mode.as_str(), "tmpfs" | "ext4")
            && !self.mount_point.as_os_str().is_empty()
    }

    pub fn save(&self) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        match std::fs::read_to_string(defs::STATE_FILE) {
            Ok(existing) if existing == json => return Ok(()),
            Ok(_) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(err.into()),
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
        crate::scoped_log!(
            info,
            "runtime_state:summary",
            "saved: storage_mode={}, active_mounts={}, daemon_alive={}",
            self.storage_mode,
            self.active_mounts.join(","),
            self.daemon.alive
        );
        Ok(())
    }

    pub fn build_from_execution(
        config: &Config,
        storage_mode: crate::core::storage::StorageMode,
        mount_point: &Path,
        result: &ExecutionResult,
        inventory: &InventorySummary,
    ) -> Result<Self> {
        crate::scoped_log!(
            debug,
            "runtime_state:build",
            "start: storage_mode={}, mount_point={}, overlay_modules={}, magic_modules={}",
            storage_mode.as_str(),
            mount_point.display(),
            result.overlay_module_ids.len(),
            result.magic_module_ids.len()
        );

        let runtime_mount_point = runtime_mount_point_after_finalize(config, mount_point);
        let mut state = Self::new(
            storage_mode.as_str().to_owned(),
            runtime_mount_point,
            result.overlay_module_ids.clone(),
            result.magic_module_ids.clone(),
            result.magic_mount_targets.clone(),
            result.custom_mount_targets.clone(),
            collect_active_mounts(result),
            result.mount_stats.clone(),
            collect_mode_stats(result),
        )?;
        state.skip_mount_modules = inventory.skip_mount_modules.clone();
        state.blacklisted_modules = inventory.blacklisted_modules.clone();
        state.mode_stats.blacklisted = state.blacklisted_modules.len();
        state.invalidate_cache();

        crate::scoped_log!(
            debug,
            "runtime_state:build",
            "complete: skip_mount_modules={}, active_mounts={}",
            state.skip_mount_modules.len(),
            state.active_mounts.len()
        );

        Ok(state)
    }

    pub fn mounted_module_ids(&self) -> HashSet<&str> {
        self.overlay_modules
            .iter()
            .chain(self.magic_modules.iter())
            .map(|s| s.as_str())
            .collect()
    }

    #[cfg(feature = "control-plane")]
    pub fn set_daemon_state(&mut self, alive: bool, socket_path: impl Into<String>) -> Result<()> {
        let refreshed_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| anyhow::anyhow!("system clock is before the Unix epoch: {err}"))?
            .as_secs();
        self.daemon.alive = alive;
        self.daemon.socket_path = socket_path.into();
        self.daemon.last_refresh_ts = refreshed_at;
        self.invalidate_cache();
        Ok(())
    }

    pub fn load() -> Result<Self> {
        crate::scoped_log!(
            debug,
            "runtime_state:load",
            "start: path={}",
            defs::STATE_FILE
        );
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
        blacklisted: 0usize,
    }
}

fn collect_active_mounts(result: &ExecutionResult) -> Vec<String> {
    let mut active_mounts = result.overlay_partitions.clone();

    if !result.custom_mount_targets.is_empty() {
        active_mounts.push("custom-bind".to_string());
    }

    active_mounts.sort();
    active_mounts.dedup();

    crate::scoped_log!(
        debug,
        "runtime_state:active_mounts",
        "complete: overlay_partitions={}, custom_mounts={}, active_mounts={}",
        result.overlay_partitions.len(),
        result.custom_mount_targets.len(),
        active_mounts.len()
    );

    active_mounts
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "control-plane")]
    use super::tmpfs_xattr_support_for_status;
    use super::{MountStatistics, RuntimeState};

    fn empty_execution_result() -> crate::core::ops::executor::ExecutionResult {
        crate::core::ops::executor::ExecutionResult {
            overlay_module_ids: Vec::new(),
            overlay_partitions: Vec::new(),
            magic_module_ids: Vec::new(),
            magic_mount_targets: Vec::new(),
            custom_mount_targets: Vec::new(),
            mount_stats: MountStatistics::default(),
            rollback_targets: Vec::new(),
        }
    }

    #[cfg(feature = "control-plane")]
    #[test]
    fn xattr_probe_failure_is_reported_as_unsupported() {
        assert!(tmpfs_xattr_support_for_status(Ok(true)));
        assert!(!tmpfs_xattr_support_for_status(Ok(false)));
        assert!(!tmpfs_xattr_support_for_status(Err(anyhow::anyhow!(
            "probe failed"
        ))));
    }

    #[test]
    fn serialized_runtime_state_round_trips() {
        let value = serde_json::to_value(RuntimeState::default()).unwrap();
        let object = value.as_object().unwrap();

        assert!(object.contains_key("mounted"));
        serde_json::from_value::<RuntimeState>(value).unwrap();
    }

    #[test]
    fn idle_state_meets_status_contract() {
        let state = RuntimeState::idle("ext4", std::path::PathBuf::from("/data/adb/hybrid-mount"));

        assert!(!state.mounted);
        assert!(state.has_valid_mount_identity());
        assert_eq!(state.storage_mode, "ext4");
        assert_eq!(
            state.mount_point.display().to_string(),
            "/data/adb/hybrid-mount"
        );
    }

    #[test]
    fn default_state_is_not_a_valid_mount_identity() {
        assert!(!RuntimeState::default().has_valid_mount_identity());
    }

    #[test]
    fn legacy_state_without_mounted_defaults_to_mounted() {
        let mut value = serde_json::to_value(RuntimeState::default()).unwrap();
        value.as_object_mut().unwrap().remove("mounted");

        let state = serde_json::from_value::<RuntimeState>(value).unwrap();

        assert!(state.mounted);
    }

    #[test]
    fn legacy_state_without_mount_target_fields_defaults_to_empty() {
        let mut value = serde_json::to_value(RuntimeState::default()).unwrap();
        let object = value.as_object_mut().unwrap();
        object.remove("magic_mount_targets");
        object.remove("custom_mounts");

        let state = serde_json::from_value::<RuntimeState>(value).unwrap();

        assert!(state.magic_mount_targets.is_empty());
        assert!(state.custom_mounts.is_empty());
    }

    #[test]
    fn finalized_state_persists_exact_magic_mount_targets() {
        let mut result = empty_execution_result();
        result.magic_mount_targets = vec![
            "/system/etc/hosts".to_string(),
            "/vendor/etc/example.conf".to_string(),
        ];

        let state = RuntimeState::build_from_execution(
            &crate::conf::config::Config::default(),
            crate::core::storage::StorageMode::Ext4,
            std::path::Path::new("/mnt/hm_a1B2c3D4e5"),
            &result,
            &crate::core::inventory::InventorySummary::default(),
        )
        .unwrap();

        assert_eq!(state.magic_mount_targets, result.magic_mount_targets);
    }

    #[test]
    fn finalized_state_uses_stable_data_root_after_staging_cleanup() {
        let config = crate::conf::config::Config::default();
        let staging = std::path::Path::new("/mnt/hm_a1B2c3D4e5");

        let state = RuntimeState::build_from_execution(
            &config,
            crate::core::storage::StorageMode::Ext4,
            staging,
            &empty_execution_result(),
            &crate::core::inventory::InventorySummary::default(),
        )
        .unwrap();

        assert_eq!(
            state.mount_point,
            std::path::Path::new(crate::defs::CONFIG_FILE)
                .parent()
                .unwrap()
                .to_path_buf()
        );
        assert_ne!(state.mount_point, staging);
    }

    #[test]
    fn finalized_state_keeps_staging_path_when_cleanup_is_disabled() {
        let config = crate::conf::config::Config {
            disable_umount: true,
            ..Default::default()
        };
        let staging = std::path::Path::new("/debug_ramdisk/hm_a1B2c3D4e5");

        let state = RuntimeState::build_from_execution(
            &config,
            crate::core::storage::StorageMode::Ext4,
            staging,
            &empty_execution_result(),
            &crate::core::inventory::InventorySummary::default(),
        )
        .unwrap();

        assert_eq!(state.mount_point, staging);
    }

    #[test]
    fn finalized_state_keeps_non_temporary_mount_paths() {
        let config = crate::conf::config::Config::default();
        let retained = std::path::Path::new("/data/adb/hybrid-mount/custom-storage");

        let state = RuntimeState::build_from_execution(
            &config,
            crate::core::storage::StorageMode::Ext4,
            retained,
            &empty_execution_result(),
            &crate::core::inventory::InventorySummary::default(),
        )
        .unwrap();

        assert_eq!(state.mount_point, retained);
    }
}
