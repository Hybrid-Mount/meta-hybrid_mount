// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

use super::{
    ModuleListEntry,
    runtime_index::RuntimeModuleIndex,
    scan_cache::{ModuleScanInfo, cached_module_scan_info, cached_suspicious_shell_commands},
};
use crate::{
    conf::config::Config,
    core::{inventory, runtime_state::RuntimeState},
    domain::MountMode,
};

pub fn build_modules_payload(
    config: &Config,
    state: &RuntimeState,
    path: Option<&Path>,
) -> Result<Vec<ModuleListEntry>> {
    if let Some(source_dir) = path {
        return build_scanned_modules_payload(config, state, source_dir);
    }

    Ok(build_runtime_modules_payload(config, state))
}

pub(super) fn build_scanned_modules_payload(
    config: &Config,
    state: &RuntimeState,
    source_dir: &Path,
) -> Result<Vec<ModuleListEntry>> {
    if !source_dir.exists() {
        return Ok(Vec::new());
    }

    let runtime_index = RuntimeModuleIndex::new(state);
    let mut modules = Vec::new();
    for entry in fs::read_dir(source_dir)
        .with_context(|| format!("failed to read module directory {}", source_dir.display()))?
    {
        let entry = entry.with_context(|| {
            format!(
                "failed to enumerate module directory {}",
                source_dir.display()
            )
        })?;
        let file_type = entry.file_type().with_context(|| {
            format!(
                "failed to read module entry type {}",
                entry.path().display()
            )
        })?;
        if !file_type.is_dir() {
            continue;
        }

        let module_path = entry.path();
        let id = entry.file_name().to_string_lossy().into_owned();
        if inventory::is_reserved_module_dir(&id) {
            continue;
        }

        modules.push(build_module_entry(
            config,
            &runtime_index,
            id,
            module_path,
            true,
        ));
    }

    modules.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(modules)
}

pub(super) fn build_runtime_modules_payload(
    config: &Config,
    state: &RuntimeState,
) -> Vec<ModuleListEntry> {
    let runtime_index = RuntimeModuleIndex::new(state);
    let mut ids = BTreeSet::new();
    ids.extend(state.overlay_modules.iter().cloned());
    ids.extend(state.magic_modules.iter().cloned());
    ids.extend(state.kasumi_modules.iter().cloned());
    ids.extend(state.skip_mount_modules.iter().cloned());
    ids.extend(state.blacklisted_modules.iter().cloned());
    ids.extend(state.mount_error_modules.iter().cloned());
    ids.extend(collect_mount_error_marker_modules(&config.moduledir));
    ids.extend(config.rules.keys().cloned());

    let mut modules = Vec::new();
    for id in ids {
        if inventory::is_reserved_module_dir(&id) {
            continue;
        }

        let source_path = config.moduledir.join(&id);
        modules.push(build_module_entry(
            config,
            &runtime_index,
            id,
            source_path,
            false,
        ));
    }

    modules
}

fn build_module_entry(
    config: &Config,
    runtime_index: &RuntimeModuleIndex<'_>,
    id: String,
    source_path: PathBuf,
    include_config_blacklist: bool,
) -> ModuleListEntry {
    let rules = inventory::load_module_rules(config, &id);
    let scan_info = cached_module_scan_info(&source_path, &id);
    let is_blacklisted = runtime_index.is_blacklisted(&id)
        || (include_config_blacklist && config.module_blacklist.contains(&id));
    let runtime_mode = if is_blacklisted {
        None
    } else {
        runtime_index.mode(&id)
    };
    let mode = if is_blacklisted {
        MountMode::Ignore
    } else {
        runtime_mode.unwrap_or(rules.default_mode)
    };
    let enabled =
        !is_blacklisted && runtime_index.enabled(&id) && !scan_info.markers.blocks_mount();
    let mount_error = if is_blacklisted {
        Some("blacklisted".to_string())
    } else {
        mount_error_reason(runtime_index, &id, &scan_info)
    };
    let suggest_ignore =
        mount_error.is_some() && cached_suspicious_shell_commands(&source_path, &id);
    let metadata = scan_info.metadata;

    ModuleListEntry {
        id,
        name: metadata.name,
        version: metadata.version,
        author: metadata.author,
        description: metadata.description,
        mode,
        is_mounted: runtime_mode.is_some(),
        enabled,
        source_path,
        rules,
        mount_error,
        suggest_ignore,
    }
}

fn collect_mount_error_marker_modules(moduledir: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(moduledir) else {
        return Vec::new();
    };

    let mut ids = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                crate::scoped_log!(
                    warn,
                    "api:modules",
                    "skip unreadable module entry: path={}, error={:#}",
                    moduledir.display(),
                    err
                );
                continue;
            }
        };
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(err) => {
                crate::scoped_log!(
                    warn,
                    "api:modules",
                    "skip module with unreadable type: path={}, error={:#}",
                    entry.path().display(),
                    err
                );
                continue;
            }
        };
        if !file_type.is_dir() {
            continue;
        }

        let id = entry.file_name().to_string_lossy().into_owned();
        if inventory::is_reserved_module_dir(&id) {
            continue;
        }

        let module_path = entry.path();
        let scan_info = cached_module_scan_info(&module_path, &id);
        if scan_info.markers.mount_error {
            ids.push(id);
        }
    }

    ids
}

fn mount_error_reason(
    runtime_index: &RuntimeModuleIndex<'_>,
    module_id: &str,
    scan_info: &ModuleScanInfo,
) -> Option<String> {
    runtime_index.mount_error_reason(module_id).or_else(|| {
        scan_info
            .markers
            .mount_error
            .then(|| "mount_error marker present".to_string())
    })
}
