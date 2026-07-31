// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};

use super::{ModuleListEntry, runtime_index::RuntimeModuleIndex, scan_info::module_scan_info};
use crate::{
    conf::config::Config,
    core::{inventory, runtime_state::RuntimeState},
    domain::MountMode,
};

pub fn build_modules_payload(
    config: &Config,
    state: &RuntimeState,
) -> Result<Vec<ModuleListEntry>> {
    build_scanned_modules_payload(config, state, &config.moduledir)
}

pub(super) fn build_scanned_modules_payload(
    config: &Config,
    state: &RuntimeState,
    source_dir: &Path,
) -> Result<Vec<ModuleListEntry>> {
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
            bail!(
                "module directory contains a non-directory entry: {}",
                entry.path().display()
            );
        }

        let module_path = entry.path();
        let id = entry
            .file_name()
            .into_string()
            .map_err(|_| anyhow::anyhow!("module directory name is not valid UTF-8"))?;
        if inventory::is_reserved_module_dir(&id) {
            continue;
        }
        crate::utils::validation::validate_module_id(&id)?;
        inventory::discovery::validate_module_prop_id(&module_path.join("module.prop"), &id)?;

        modules.push(build_module_entry(config, &runtime_index, id, module_path)?);
    }

    modules.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(modules)
}

fn build_module_entry(
    config: &Config,
    runtime_index: &RuntimeModuleIndex<'_>,
    id: String,
    source_path: PathBuf,
) -> Result<ModuleListEntry> {
    let rules = inventory::load_module_rules(config, &id)?;
    let scan_info = module_scan_info(&source_path, &id)?;
    let is_blacklisted = runtime_index.is_blacklisted(&id) || config.module_blacklist.contains(&id);
    let runtime_mode = if is_blacklisted {
        None
    } else {
        runtime_index.mode(&id)
    };
    let mode = if is_blacklisted {
        MountMode::Ignore
    } else {
        match runtime_mode {
            Some(mode) => mode,
            None => rules.default_mode,
        }
    };
    let enabled =
        !is_blacklisted && runtime_index.enabled(&id) && !scan_info.markers.blocks_mount();
    let metadata = scan_info.metadata;

    Ok(ModuleListEntry {
        id,
        name: metadata.name,
        version: metadata.version,
        author: metadata.author,
        description: metadata.description,
        mode,
        is_mounted: runtime_mode.is_some(),
        enabled,
        is_blacklisted,
        rules,
    })
}
