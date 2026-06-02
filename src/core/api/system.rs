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

use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
#[cfg(any(target_os = "linux", target_os = "android"))]
use procfs::process::Process;
use rustix::fs::statvfs;
use serde::Serialize;

use crate::{conf::config::Config, core::runtime_state::RuntimeState, partitions};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PartitionInfo {
    pub name: String,
    pub mount_point: String,
    pub fs_type: String,
    pub is_read_only: bool,
    pub exists_as_symlink: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct StorageInfo {
    pub path: String,
    pub pid: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub used: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percent: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MountStatsPayload {
    pub total_mounts: usize,
    pub successful_mounts: usize,
    pub failed_mounts: usize,
    pub tmpfs_created: usize,
    pub files_mounted: usize,
    pub dirs_mounted: usize,
    pub symlinks_created: usize,
    pub overlayfs_mounts: usize,
    pub success_rate: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SystemInfoPayload {
    pub kernel: String,
    pub selinux: String,
    pub mount_base: String,
    pub active_mounts: Vec<String>,
    #[cfg(feature = "control-plane")]
    pub tmpfs_xattr_supported: bool,
    pub supported_overlay_modes: Vec<String>,
}

impl From<&crate::core::runtime_state::MountStatistics> for MountStatsPayload {
    fn from(stats: &crate::core::runtime_state::MountStatistics) -> Self {
        Self {
            total_mounts: stats.total_mounts,
            successful_mounts: stats.successful_mounts,
            failed_mounts: stats.failed_mounts,
            tmpfs_created: stats.tmpfs_created,
            files_mounted: stats.files_mounted,
            dirs_mounted: stats.dirs_mounted,
            symlinks_created: stats.symlinks_created,
            overlayfs_mounts: stats.overlayfs_mounts,
            success_rate: stats.success_rate(),
        }
    }
}

#[derive(Debug)]
struct MountEntry {
    mount_point: PathBuf,
    fs_type: String,
    is_read_only: bool,
}

fn storage_mode_label(storage_mode: &str) -> String {
    if storage_mode.is_empty() {
        "unknown".to_string()
    } else {
        storage_mode.to_string()
    }
}

fn format_windows_size(bytes: u64) -> String {
    const UNITS: [&str; 7] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"];

    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }

    if value.fract() == 0.0 {
        format!("{value:.0} {}", UNITS[unit])
    } else {
        format!("{value:.2} {}", UNITS[unit])
    }
}

pub fn build_storage_payload(state: &RuntimeState) -> StorageInfo {
    let mount_path = state.mount_point.clone();
    let path_str = mount_path.display().to_string();

    if mount_path.as_os_str().is_empty() || !mount_path.exists() {
        return StorageInfo {
            path: path_str,
            pid: state.pid,
            error: Some("Not mounted".to_string()),
            warning: None,
            size: None,
            used: None,
            avail: None,
            percent: None,
            mode: Some(storage_mode_label(&state.storage_mode)),
        };
    }

    match statvfs_usage(&mount_path) {
        Ok((total_bytes, used_bytes, free_bytes, percent)) => StorageInfo {
            path: path_str,
            pid: state.pid,
            error: None,
            warning: (total_bytes == 0).then_some("Zero size detected".to_string()),
            size: Some(format_windows_size(total_bytes)),
            used: Some(format_windows_size(used_bytes)),
            avail: Some(format_windows_size(free_bytes)),
            percent: Some(percent),
            mode: Some(storage_mode_label(&state.storage_mode)),
        },
        Err(err) => StorageInfo {
            path: path_str,
            pid: state.pid,
            error: Some(format!("statvfs failed: {err:#}")),
            warning: None,
            size: None,
            used: None,
            avail: None,
            percent: None,
            mode: Some(storage_mode_label(&state.storage_mode)),
        },
    }
}

pub fn build_mount_stats_payload(state: &RuntimeState) -> MountStatsPayload {
    MountStatsPayload::from(&state.mount_stats)
}

pub fn build_partitions_payload(config: &Config) -> Vec<PartitionInfo> {
    detect_partitions(config).unwrap_or_default()
}

pub fn build_system_info_payload(state: &RuntimeState) -> SystemInfoPayload {
    SystemInfoPayload {
        kernel: read_kernel_release().unwrap_or_else(|_| "Unknown".to_string()),
        selinux: read_selinux_status().unwrap_or_else(|_| "Unknown".to_string()),
        mount_base: state.mount_point.display().to_string(),
        active_mounts: state.active_mounts.clone(),
        #[cfg(feature = "control-plane")]
        tmpfs_xattr_supported: state.tmpfs_xattr_supported,
        supported_overlay_modes: vec!["tmpfs".to_string(), "ext4".to_string()],
    }
}

fn statvfs_usage(path: &std::path::Path) -> Result<(u64, u64, u64, f64)> {
    let stats = statvfs(path).with_context(|| format!("statvfs failed for {}", path.display()))?;
    let block_size = if stats.f_frsize > 0 {
        stats.f_frsize
    } else {
        stats.f_bsize
    };
    let total_bytes = stats.f_blocks.saturating_mul(block_size);
    let free_bytes = stats.f_bavail.saturating_mul(block_size);
    let used_bytes = total_bytes.saturating_sub(stats.f_bfree.saturating_mul(block_size));
    let percent = if total_bytes > 0 {
        used_bytes as f64 * 100.0 / total_bytes as f64
    } else {
        0.0
    };

    Ok((total_bytes, used_bytes, free_bytes, percent))
}

fn detect_partitions(_config: &Config) -> Result<Vec<PartitionInfo>> {
    let mount_entries = read_mount_entries()?;
    let mut partitions = Vec::new();

    for name in partitions::managed_partition_names() {
        let mount_point = PathBuf::from("/").join(&name);
        let metadata = match fs::symlink_metadata(&mount_point) {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };
        let exists_as_symlink = metadata.file_type().is_symlink();
        let resolved = if exists_as_symlink {
            fs::canonicalize(&mount_point).unwrap_or_else(|_| mount_point.clone())
        } else {
            mount_point.clone()
        };

        let match_entry = mount_entries
            .iter()
            .find(|entry| entry.mount_point == mount_point || entry.mount_point == resolved);

        partitions.push(PartitionInfo {
            name,
            mount_point: mount_point.display().to_string(),
            fs_type: match_entry
                .map(|entry| entry.fs_type.clone())
                .unwrap_or_default(),
            is_read_only: match_entry.is_some_and(|entry| entry.is_read_only),
            exists_as_symlink,
        });
    }

    Ok(partitions)
}

fn read_kernel_release() -> Result<String> {
    let release = fs::read_to_string("/proc/sys/kernel/osrelease")
        .context("failed to read /proc/sys/kernel/osrelease")?;
    let trimmed = release.trim();
    if !trimmed.is_empty() {
        return Ok(trimmed.to_string());
    }

    let proc_version =
        fs::read_to_string("/proc/version").context("failed to read /proc/version")?;
    let trimmed = proc_version.trim();
    if let Some(rest) = trimmed.strip_prefix("Linux version ")
        && let Some(version) = rest.split_whitespace().next()
    {
        return Ok(version.to_string());
    }
    Ok("Unknown".to_string())
}

fn read_selinux_status() -> Result<String> {
    if let Ok(enforce) = fs::read_to_string("/sys/fs/selinux/enforce") {
        match enforce.trim() {
            "1" => return Ok("Enforcing".to_string()),
            "0" => return Ok("Permissive".to_string()),
            _ => {}
        }
    }

    Ok("Unknown".to_string())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn read_mount_entries() -> Result<Vec<MountEntry>> {
    Ok(Process::myself()
        .context("failed to open self procfs handle")?
        .mountinfo()
        .context("failed to read mountinfo")?
        .into_iter()
        .map(|entry| MountEntry {
            mount_point: entry.mount_point,
            fs_type: entry.fs_type,
            is_read_only: entry.mount_options.contains_key("ro"),
        })
        .collect())
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn read_mount_entries() -> Result<Vec<MountEntry>> {
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::format_windows_size;

    #[test]
    fn formats_windows_style_sizes() {
        assert_eq!(format_windows_size(0), "0 B");
        assert_eq!(format_windows_size(999), "999 B");
        assert_eq!(format_windows_size(1024), "1 KiB");
        assert_eq!(format_windows_size(1536), "1.50 KiB");
        assert_eq!(format_windows_size(1024 * 1024), "1 MiB");
    }
}
