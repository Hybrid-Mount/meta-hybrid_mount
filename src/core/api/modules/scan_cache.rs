// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::{
    collections::HashMap,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::SystemTime,
};

use super::metadata::{ModuleMetadata, read_module_metadata};
use crate::defs;

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileStamp {
    len: u64,
    modified: Option<SystemTime>,
}

impl FileStamp {
    fn from_metadata(metadata: &fs::Metadata) -> Self {
        Self {
            len: metadata.len(),
            modified: metadata.modified().ok(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct ModuleMarkers {
    blocked: bool,
    pub(super) mount_error: bool,
}

impl ModuleMarkers {
    pub(super) fn blocks_mount(self) -> bool {
        self.blocked
    }
}

#[derive(Debug, Clone)]
pub(super) struct ModuleScanInfo {
    pub(super) metadata: ModuleMetadata,
    pub(super) markers: ModuleMarkers,
    shell_paths: Vec<PathBuf>,
    suspicious_shell_commands: Option<bool>,
}

#[derive(Debug, Clone)]
struct CachedModuleScanInfo {
    dir_stamp: Option<FileStamp>,
    prop_stamp: Option<FileStamp>,
    shell_stamps: Vec<(PathBuf, Option<FileStamp>)>,
    info: ModuleScanInfo,
}

type ModuleScanCacheKey = (PathBuf, String);

static MODULE_SCAN_CACHE: OnceLock<Mutex<HashMap<ModuleScanCacheKey, CachedModuleScanInfo>>> =
    OnceLock::new();

pub(super) fn cached_module_scan_info(module_path: &Path, module_id: &str) -> ModuleScanInfo {
    let key = (module_path.to_path_buf(), module_id.to_string());
    let dir_stamp = file_stamp(module_path);
    let prop_stamp = file_stamp(&module_path.join("module.prop"));

    let cached_candidate = {
        let cache = lock_module_scan_cache();
        if let Some(cached) = cache.get(&key)
            && cached.dir_stamp == dir_stamp
            && cached.prop_stamp == prop_stamp
        {
            Some((cached.shell_stamps.clone(), cached.info.clone()))
        } else {
            None
        }
    };
    if let Some((shell_stamps, info)) = cached_candidate
        && current_shell_stamps(&shell_stamps) == shell_stamps
    {
        return info;
    }

    let scanned = scan_module_info(module_path, module_id);
    let info = scanned.info.clone();
    let mut cache = lock_module_scan_cache();
    if cache.len() > 512 {
        cache.clear();
    }
    cache.insert(key, scanned);
    info
}

pub(super) fn cached_suspicious_shell_commands(module_path: &Path, module_id: &str) -> bool {
    let _ = cached_module_scan_info(module_path, module_id);
    let key = (module_path.to_path_buf(), module_id.to_string());
    let shell_paths = {
        let cache = lock_module_scan_cache();
        let Some(cached) = cache.get(&key) else {
            return false;
        };
        if let Some(suspicious) = cached.info.suspicious_shell_commands {
            return suspicious;
        }
        cached.info.shell_paths.clone()
    };

    let suspicious = has_suspicious_shell_commands(&shell_paths);
    let mut cache = lock_module_scan_cache();
    if let Some(cached) = cache.get_mut(&key) {
        cached.info.suspicious_shell_commands = Some(suspicious);
    }
    suspicious
}

fn module_scan_cache() -> &'static Mutex<HashMap<ModuleScanCacheKey, CachedModuleScanInfo>> {
    MODULE_SCAN_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lock_module_scan_cache()
-> std::sync::MutexGuard<'static, HashMap<ModuleScanCacheKey, CachedModuleScanInfo>> {
    module_scan_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn scan_module_info(module_path: &Path, module_id: &str) -> CachedModuleScanInfo {
    let dir_stamp = file_stamp(module_path);
    let prop_path = module_path.join("module.prop");
    let prop_stamp = file_stamp(&prop_path);
    let metadata = read_module_metadata(module_path, module_id);
    let mut markers = ModuleMarkers::default();
    let mut shell_paths = Vec::new();

    if let Ok(entries) = fs::read_dir(module_path) {
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            if os_str_eq_ignore_ascii_case(&file_name, defs::MOUNT_ERROR_FILE_NAME) {
                markers.blocked = true;
                markers.mount_error = true;
            } else if [
                defs::DISABLE_FILE_NAME,
                defs::REMOVE_FILE_NAME,
                defs::SKIP_MOUNT_FILE_NAME,
            ]
            .into_iter()
            .any(|marker| os_str_eq_ignore_ascii_case(&file_name, marker))
            {
                markers.blocked = true;
            }

            let path = entry.path();
            if os_str_ext_eq_ignore_ascii_case(&path, "sh") {
                shell_paths.push(path);
            }
        }
    }

    shell_paths.sort();
    let shell_stamps = shell_paths
        .iter()
        .map(|path| (path.clone(), followed_file_stamp(path)))
        .collect();

    CachedModuleScanInfo {
        dir_stamp,
        prop_stamp,
        shell_stamps,
        info: ModuleScanInfo {
            metadata,
            markers,
            shell_paths,
            suspicious_shell_commands: None,
        },
    }
}

fn file_stamp(path: &Path) -> Option<FileStamp> {
    fs::symlink_metadata(path)
        .ok()
        .map(|metadata| FileStamp::from_metadata(&metadata))
}

fn followed_file_stamp(path: &Path) -> Option<FileStamp> {
    fs::metadata(path)
        .ok()
        .map(|metadata| FileStamp::from_metadata(&metadata))
}

fn os_str_eq_ignore_ascii_case(value: &OsStr, expected: &str) -> bool {
    value
        .as_encoded_bytes()
        .eq_ignore_ascii_case(expected.as_bytes())
}

fn os_str_ext_eq_ignore_ascii_case(path: &Path, expected: &str) -> bool {
    path.extension()
        .is_some_and(|ext| os_str_eq_ignore_ascii_case(ext, expected))
}

fn current_shell_stamps(
    shell_stamps: &[(PathBuf, Option<FileStamp>)],
) -> Vec<(PathBuf, Option<FileStamp>)> {
    shell_stamps
        .iter()
        .map(|(path, _)| (path.clone(), followed_file_stamp(path)))
        .collect()
}

/// Scans .sh files in the module directory for shell commands that suggest the
/// module performs its own mount operations (mount, bind mount, mkdir, touch).
/// When true, the user should consider setting the module to "ignore" mode
/// because Hybrid Mount cannot manage modules that do their own mounting.
const MAX_SH_SCAN_BYTES: u64 = 256 * 1024;

fn has_suspicious_shell_commands(shell_paths: &[PathBuf]) -> bool {
    for path in shell_paths {
        let Ok(meta) = path.metadata() else {
            continue;
        };
        if !meta.is_file() || meta.len() > MAX_SH_SCAN_BYTES {
            continue;
        }

        let Ok(content) = fs::read_to_string(path) else {
            continue;
        };

        if contains_mount_commands(&content) {
            return true;
        }
    }

    false
}

fn contains_mount_commands(content: &str) -> bool {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let first_word = trimmed
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_start_matches(['\\', '`']);

        match first_word {
            "mount" | "mkdir" | "touch" => return true,
            "busybox" => {
                let rest = &trimmed[first_word.len()..].trim_start();
                let sub_cmd = rest.split_whitespace().next().unwrap_or("");
                if matches!(sub_cmd, "mount" | "mkdir" | "touch") {
                    return true;
                }
            }
            _ => {}
        }

        if first_word.contains("mount") || first_word.contains("bind") {
            return true;
        }
    }
    false
}
