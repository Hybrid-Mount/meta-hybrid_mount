// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    collections::BTreeSet,
    ffi::CString,
    fs,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::{Value, json};

use crate::{
    conf::config::Config,
    core::runtime_state::RuntimeState,
    defs,
    sys::{
        hymofs::{self, HymoFsStatus},
        lkm,
    },
};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct HymofsRuleEntry {
    #[serde(rename = "type")]
    pub rule_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_type: Option<i32>,
}

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

#[derive(Debug)]
struct MountEntry {
    mount_point: PathBuf,
    fs_type: String,
    is_read_only: bool,
}

pub fn parse_hymofs_rule_listing(listing: &str) -> Vec<HymofsRuleEntry> {
    let mut rules = Vec::new();

    for raw_line in listing.lines() {
        let line = raw_line.trim();
        if line.is_empty()
            || line.starts_with("HymoFS Protocol:")
            || line.starts_with("HymoFS Enabled:")
        {
            continue;
        }

        let mut parts = line.split_whitespace();
        let Some(kind_raw) = parts.next() else {
            continue;
        };
        let rule_type = kind_raw.to_uppercase();

        match rule_type.as_str() {
            "ADD" => {
                let target = parts.next().map(ToString::to_string);
                let source = parts.next().map(ToString::to_string);
                let file_type = parts.next().and_then(|value| value.parse::<i32>().ok());
                rules.push(HymofsRuleEntry {
                    rule_type,
                    target,
                    source,
                    path: None,
                    args: None,
                    file_type,
                });
            }
            "MERGE" => {
                let target = parts.next().map(ToString::to_string);
                let source = parts.next().map(ToString::to_string);
                rules.push(HymofsRuleEntry {
                    rule_type,
                    target,
                    source,
                    path: None,
                    args: None,
                    file_type: None,
                });
            }
            "HIDE" | "INJECT" => {
                rules.push(HymofsRuleEntry {
                    rule_type,
                    target: None,
                    source: None,
                    path: parts.next().map(ToString::to_string),
                    args: None,
                    file_type: None,
                });
            }
            _ => {
                let args = parts.collect::<Vec<_>>().join(" ");
                rules.push(HymofsRuleEntry {
                    rule_type,
                    target: None,
                    source: None,
                    path: None,
                    args: (!args.is_empty()).then_some(args),
                    file_type: None,
                });
            }
        }
    }

    rules
}

pub fn build_features_payload() -> Value {
    let bits = hymofs::get_features().unwrap_or_default();
    json!({
        "bitmask": bits,
        "names": hymofs::feature_names(bits),
    })
}

pub fn build_lkm_payload(config: &Config) -> Value {
    let status = lkm::status(&config.hymofs);
    json!({
        "loaded": status.loaded,
        "module_name": status.module_name,
        "autoload": status.autoload,
        "kmi_override": status.kmi_override,
        "current_kmi": status.current_kmi,
        "search_dir": status.search_dir,
        "module_file": status.module_file,
        "last_error": lkm::last_error(),
    })
}

pub fn build_hymofs_version_payload(config: &Config, state: &RuntimeState) -> Value {
    if !config.hymofs.enabled {
        return json!({
            "protocol_version": hymofs::HYMO_PROTOCOL_VERSION,
            "kernel_version": 0,
            "hymofs_available": false,
            "protocol_mismatch": false,
            "mismatch_message": Value::Null,
            "active_modules": Vec::<String>::new(),
            "mount_base": state.mount_point,
            "mirror_path": config.hymofs.mirror_path,
        });
    }

    let status = hymofs::check_status();
    let kernel_version = hymofs::get_protocol_version().ok();
    let active_rules = hymofs::get_active_rules().unwrap_or_default();
    let parsed_rules = parse_hymofs_rule_listing(&active_rules);
    let active_modules = if !state.hymofs_modules.is_empty() {
        let mut modules = state.hymofs_modules.clone();
        modules.sort();
        modules.dedup();
        modules
    } else {
        extract_active_module_ids(&parsed_rules, &config.hymofs.mirror_path)
    };

    let mismatch = kernel_version.is_some_and(|version| version != hymofs::HYMO_PROTOCOL_VERSION);

    json!({
        "protocol_version": hymofs::HYMO_PROTOCOL_VERSION,
        "kernel_version": kernel_version.unwrap_or_default(),
        "hymofs_available": status == HymoFsStatus::Available,
        "protocol_mismatch": mismatch,
        "mismatch_message": mismatch_message(status, kernel_version),
        "active_modules": active_modules,
        "mount_base": state.mount_point,
        "mirror_path": config.hymofs.mirror_path,
    })
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
            mode: state
                .storage_mode
                .is_empty()
                .then_some("unknown".to_string())
                .or_else(|| Some(state.storage_mode.clone())),
        };
    }

    match statvfs_usage(&mount_path) {
        Ok((total_bytes, used_bytes, free_bytes, percent)) => StorageInfo {
            path: path_str,
            pid: state.pid,
            error: None,
            warning: (total_bytes == 0).then_some("Zero size detected".to_string()),
            size: Some(format_size(total_bytes)),
            used: Some(format_size(used_bytes)),
            avail: Some(format_size(free_bytes)),
            percent: Some(percent),
            mode: Some(if state.storage_mode.is_empty() {
                "unknown".to_string()
            } else {
                state.storage_mode.clone()
            }),
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
            mode: Some(if state.storage_mode.is_empty() {
                "unknown".to_string()
            } else {
                state.storage_mode.clone()
            }),
        },
    }
}

pub fn build_mount_stats_payload(state: &RuntimeState) -> Value {
    let stats = &state.mount_stats;
    json!({
        "total_mounts": stats.total_mounts,
        "successful_mounts": stats.successful_mounts,
        "failed_mounts": stats.failed_mounts,
        "tmpfs_created": stats.tmpfs_created,
        "files_mounted": stats.files_mounted,
        "dirs_mounted": stats.dirs_mounted,
        "symlinks_created": stats.symlinks_created,
        "overlayfs_mounts": stats.overlayfs_mounts,
        "success_rate": stats.success_rate(),
    })
}

pub fn build_partitions_payload(config: &Config) -> Vec<PartitionInfo> {
    detect_partitions(config).unwrap_or_default()
}

pub fn build_system_payload(config: &Config, state: &RuntimeState) -> Value {
    let status = if config.hymofs.enabled {
        hymofs::check_status()
    } else {
        HymoFsStatus::NotPresent
    };
    let features = if config.hymofs.enabled {
        build_features_payload()
    } else {
        json!({
            "bitmask": 0,
            "names": Vec::<String>::new(),
        })
    };
    let hooks = if config.hymofs.enabled {
        hymofs::get_hooks().unwrap_or_default()
    } else {
        String::new()
    };

    json!({
        "kernel": read_kernel_release().unwrap_or_else(|_| "Unknown".to_string()),
        "selinux": read_selinux_status().unwrap_or_else(|_| "Unknown".to_string()),
        "mount_base": state.mount_point,
        "mount_error_modules": state.mount_error_modules,
        "mount_error_reasons": state.mount_error_reasons,
        "skip_mount_modules": state.skip_mount_modules,
        "hymofs_available": status == HymoFsStatus::Available,
        "hymofs_status": status_code(status),
        "lkm": build_lkm_payload(config),
        "mountStats": build_mount_stats_payload(state),
        "detectedPartitions": build_partitions_payload(config),
        "hooks": hooks,
        "features": features,
    })
}

fn status_code(status: HymoFsStatus) -> i32 {
    match status {
        HymoFsStatus::Available => 0,
        HymoFsStatus::NotPresent => 1,
        HymoFsStatus::KernelTooOld => 2,
        HymoFsStatus::ModuleTooOld => 3,
    }
}

fn mismatch_message(status: HymoFsStatus, kernel_version: Option<i32>) -> Option<String> {
    match status {
        HymoFsStatus::KernelTooOld => Some(format!(
            "kernel protocol {} is older than userspace api{}",
            kernel_version.unwrap_or_default(),
            hymofs::HYMO_PROTOCOL_VERSION
        )),
        HymoFsStatus::ModuleTooOld => Some(format!(
            "kernel protocol {} is newer than userspace api{}",
            kernel_version.unwrap_or_default(),
            hymofs::HYMO_PROTOCOL_VERSION
        )),
        HymoFsStatus::Available => kernel_version
            .filter(|version| *version != hymofs::HYMO_PROTOCOL_VERSION)
            .map(|version| {
                format!(
                    "protocol mismatch: userspace api{}, kernel api{}",
                    hymofs::HYMO_PROTOCOL_VERSION,
                    version
                )
            }),
        HymoFsStatus::NotPresent => None,
    }
}

fn extract_active_module_ids(rules: &[HymofsRuleEntry], mirror_path: &Path) -> Vec<String> {
    let mut modules = BTreeSet::new();

    for rule in rules {
        let Some(source) = rule.source.as_deref() else {
            continue;
        };

        if let Some(module_id) = extract_module_id_from_source(source, mirror_path) {
            modules.insert(module_id);
        }
    }

    modules.into_iter().collect()
}

fn extract_module_id_from_source(source: &str, mirror_path: &Path) -> Option<String> {
    let module_root = format!("{}/", defs::MODULES_DIR.trim_end_matches('/'));
    if let Some(rest) = source.strip_prefix(&module_root) {
        return rest
            .split('/')
            .next()
            .filter(|value| !value.is_empty())
            .map(ToString::to_string);
    }

    let mirror_prefix = format!(
        "{}/",
        mirror_path.display().to_string().trim_end_matches('/')
    );
    if let Some(rest) = source.strip_prefix(&mirror_prefix) {
        return rest
            .split('/')
            .next()
            .filter(|value| !value.is_empty())
            .map(ToString::to_string);
    }

    None
}

// libc::statvfs field widths differ between Android/glibc (all u64) and macOS
// (mixed u32/u64); silence the per-platform cast/conversion lints instead of
// carrying target-gated code for a stat-calc helper.
#[allow(clippy::unnecessary_cast, clippy::useless_conversion)]
fn statvfs_usage(path: &Path) -> Result<(u64, u64, u64, f64)> {
    let c_path = CString::new(path.as_os_str().as_bytes())
        .with_context(|| format!("invalid storage path {}", path.display()))?;
    let mut stats = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    let rc = unsafe { libc::statvfs(c_path.as_ptr(), stats.as_mut_ptr()) };
    if rc != 0 {
        return Err(std::io::Error::last_os_error())
            .with_context(|| format!("statvfs failed for {}", path.display()));
    }

    let stats = unsafe { stats.assume_init() };
    let block_size = if stats.f_frsize > 0 {
        stats.f_frsize
    } else {
        stats.f_bsize
    };
    // statvfs field widths differ between glibc/bionic (u64) and macOS (u32);
    // widen through u64::from to stay portable without tripping clippy's
    // unnecessary_cast on the Android targets used in CI.
    let block_size = u64::from(block_size);
    let total_bytes = u64::from(stats.f_blocks).saturating_mul(block_size);
    let free_bytes = u64::from(stats.f_bavail).saturating_mul(block_size);
    let used_bytes =
        total_bytes.saturating_sub(u64::from(stats.f_bfree).saturating_mul(block_size));
    let percent = if total_bytes > 0 {
        used_bytes as f64 * 100.0 / total_bytes as f64
    } else {
        0.0
    };

    Ok((total_bytes, used_bytes, free_bytes, percent))
}

fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "K", "M", "G", "T"];

    if bytes < 1024 {
        return format!("{bytes}B");
    }

    let mut value = bytes as f64;
    let mut unit_idx = 0usize;
    while value >= 1024.0 && unit_idx < UNITS.len() - 1 {
        value /= 1024.0;
        unit_idx += 1;
    }

    if value >= 100.0 || (value - value.round()).abs() < 0.05 {
        format!("{value:.0}{}", UNITS[unit_idx])
    } else {
        format!("{value:.1}{}", UNITS[unit_idx])
    }
}

fn read_kernel_release() -> Result<String> {
    if let Ok(value) = fs::read_to_string("/proc/sys/kernel/osrelease") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }

    let uts = unsafe {
        let mut uts = std::mem::MaybeUninit::<libc::utsname>::uninit();
        if libc::uname(uts.as_mut_ptr()) != 0 {
            return Err(std::io::Error::last_os_error()).context("uname failed");
        }
        uts.assume_init()
    };

    let bytes = unsafe { std::ffi::CStr::from_ptr(uts.release.as_ptr()) }.to_bytes();
    Ok(String::from_utf8_lossy(bytes).trim().to_string())
}

fn read_selinux_status() -> Result<String> {
    if let Ok(value) = fs::read_to_string("/sys/fs/selinux/enforce") {
        return Ok(match value.trim() {
            "0" => "Permissive".to_string(),
            "1" => "Enforcing".to_string(),
            other if !other.is_empty() => other.to_string(),
            _ => "Unknown".to_string(),
        });
    }

    let output = Command::new("getenforce").output();
    if let Ok(output) = output
        && output.status.success()
    {
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !text.is_empty() {
            return Ok(text);
        }
    }

    Ok("Unknown".to_string())
}

fn detect_partitions(config: &Config) -> Result<Vec<PartitionInfo>> {
    let mount_entries = read_mount_entries()?;
    let mut names: BTreeSet<String> = defs::BUILTIN_PARTITIONS
        .iter()
        .map(|value| value.to_string())
        .collect();
    names.insert("system".to_string());
    names.extend(config.partitions.iter().cloned());

    let mut partitions = Vec::new();

    for name in names {
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

fn read_mount_entries() -> Result<Vec<MountEntry>> {
    let content =
        fs::read_to_string("/proc/self/mounts").context("failed to read /proc/self/mounts")?;
    let mut entries = Vec::new();

    for line in content.lines() {
        let mut parts = line.split_whitespace();
        let _device = parts.next();
        let Some(mount_point) = parts.next() else {
            continue;
        };
        let Some(fs_type) = parts.next() else {
            continue;
        };
        let Some(options) = parts.next() else {
            continue;
        };

        entries.push(MountEntry {
            mount_point: PathBuf::from(mount_point),
            fs_type: fs_type.to_string(),
            is_read_only: options.split(',').any(|option| option == "ro"),
        });
    }

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{extract_active_module_ids, parse_hymofs_rule_listing};

    #[test]
    fn parse_hymofs_rules_skips_header_lines_and_parses_known_entries() {
        let raw = "\
HymoFS Protocol: 14\n\
HymoFS Enabled: 1\n\
add /system/bin/sh /dev/hymo_mirror/demo/system/bin/sh 10\n\
hide /data/adb/magisk\n\
merge /system/etc /dev/hymo_mirror/demo/system/etc\n\
mount_hide enabled\n";

        let parsed = parse_hymofs_rule_listing(raw);

        assert_eq!(parsed.len(), 4);
        assert_eq!(parsed[0].rule_type, "ADD");
        assert_eq!(parsed[0].target.as_deref(), Some("/system/bin/sh"));
        assert_eq!(
            parsed[0].source.as_deref(),
            Some("/dev/hymo_mirror/demo/system/bin/sh")
        );
        assert_eq!(parsed[0].file_type, Some(10));
        assert_eq!(parsed[1].rule_type, "HIDE");
        assert_eq!(parsed[1].path.as_deref(), Some("/data/adb/magisk"));
        assert_eq!(parsed[2].rule_type, "MERGE");
        assert_eq!(parsed[3].rule_type, "MOUNT_HIDE");
        assert_eq!(parsed[3].args.as_deref(), Some("enabled"));
    }

    #[test]
    fn extract_active_modules_prefers_module_and_mirror_sources() {
        let raw = "\
add /system/bin/sh /data/adb/modules/mod_a/system/bin/sh 10\n\
merge /system/etc /dev/hymo_mirror/mod_b/system/etc\n";
        let parsed = parse_hymofs_rule_listing(raw);
        let modules = extract_active_module_ids(&parsed, Path::new("/dev/hymo_mirror"));

        assert_eq!(modules, vec!["mod_a".to_string(), "mod_b".to_string()]);
    }
}
