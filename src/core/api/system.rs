// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::{fs, path::PathBuf};

use anyhow::{Context, Result, bail};
#[cfg(any(target_os = "linux", target_os = "android"))]
use procfs::process::Process;
use rustix::fs::statvfs;
use serde::Serialize;

use crate::{conf::config::Config, core::runtime_state::RuntimeState, partitions};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PartitionInfo {
    pub name: String,
    pub mount_point: String,
    pub fs_type: Option<String>,
    pub is_read_only: bool,
    pub exists_as_symlink: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct StorageInfo {
    pub path: String,
    pub pid: u32,
    pub size: String,
    pub used: String,
    pub avail: String,
    pub percent: f64,
    pub mode: String,
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

pub fn build_storage_payload(state: &RuntimeState) -> Result<StorageInfo> {
    let mount_path = state.mount_point.clone();
    let path_str = mount_path.display().to_string();

    if mount_path.as_os_str().is_empty() || !mount_path.exists() {
        bail!("storage mount is unavailable: {}", mount_path.display());
    }

    let (total_bytes, used_bytes, free_bytes, percent) = statvfs_usage(&mount_path)?;
    if total_bytes == 0 {
        bail!("storage filesystem reports zero total bytes");
    }

    Ok(StorageInfo {
        path: path_str,
        pid: state.pid,
        size: format_windows_size(total_bytes),
        used: format_windows_size(used_bytes),
        avail: format_windows_size(free_bytes),
        percent,
        mode: state.storage_mode.clone(),
    })
}

pub fn build_mount_stats_payload(state: &RuntimeState) -> MountStatsPayload {
    MountStatsPayload::from(&state.mount_stats)
}

pub fn build_partitions_payload(config: &Config) -> Result<Vec<PartitionInfo>> {
    detect_partitions(config)
}

pub fn build_system_info_payload(state: &RuntimeState) -> Result<SystemInfoPayload> {
    Ok(SystemInfoPayload {
        kernel: read_kernel_release()?,
        selinux: read_selinux_status()?,
        mount_base: state.mount_point.display().to_string(),
        active_mounts: state.active_mounts.clone(),
        #[cfg(feature = "control-plane")]
        tmpfs_xattr_supported: state.tmpfs_xattr_supported,
        supported_overlay_modes: vec!["tmpfs".to_string(), "ext4".to_string()],
    })
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
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
            Err(err) => return Err(err).with_context(|| format!("failed to inspect {name}")),
        };
        let exists_as_symlink = metadata.file_type().is_symlink();
        let resolved = if exists_as_symlink {
            fs::canonicalize(&mount_point)
                .with_context(|| format!("failed to resolve {}", mount_point.display()))?
        } else {
            mount_point.clone()
        };

        let match_entry = mount_entries
            .iter()
            .find(|entry| entry.mount_point == mount_point || entry.mount_point == resolved);

        partitions.push(PartitionInfo {
            name,
            mount_point: mount_point.display().to_string(),
            fs_type: match_entry.map(|entry| entry.fs_type.clone()),
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

    anyhow::bail!("/proc/sys/kernel/osrelease is empty")
}

fn read_selinux_status() -> Result<String> {
    let enforce = fs::read_to_string("/sys/fs/selinux/enforce")
        .context("failed to read /sys/fs/selinux/enforce")?;
    match enforce.trim() {
        "1" => Ok("Enforcing".to_string()),
        "0" => Ok("Permissive".to_string()),
        value => anyhow::bail!("invalid SELinux enforcement value: {value}"),
    }
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
