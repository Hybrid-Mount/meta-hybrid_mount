// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::{collections::HashSet, fs, path::Path};

use anyhow::{Context, Result, bail};

use super::{ModuleApplyEntry, ModulesApplyPayload};
use crate::{conf::config::Config, defs, utils};

struct ValidatedModuleApply<'a> {
    entry: &'a ModuleApplyEntry,
    marker_change: Option<MarkerChange>,
}

#[derive(Clone)]
enum MarkerChange {
    Create(std::path::PathBuf),
    Remove(std::path::PathBuf),
}

impl MarkerChange {
    fn apply(&self) -> Result<()> {
        match self {
            Self::Create(path) => fs::write(path, b"")
                .with_context(|| format!("failed to create disable marker {}", path.display())),
            Self::Remove(path) => fs::remove_file(path)
                .with_context(|| format!("failed to remove disable marker {}", path.display())),
        }
    }

    fn rollback(&self) -> Result<()> {
        match self {
            Self::Create(path) => fs::remove_file(path)
                .with_context(|| format!("failed to roll back disable marker {}", path.display())),
            Self::Remove(path) => fs::write(path, b"")
                .with_context(|| format!("failed to restore disable marker {}", path.display())),
        }
    }
}

fn plan_marker_change(module_path: &Path, enabled: Option<bool>) -> Result<Option<MarkerChange>> {
    let Some(enabled) = enabled else {
        return Ok(None);
    };
    let disable_path = module_path.join(defs::DISABLE_FILE_NAME);
    let marker_exists = match fs::symlink_metadata(&disable_path) {
        Ok(metadata) if metadata.file_type().is_file() => true,
        Ok(_) => bail!(
            "disable marker '{}' is not a regular file",
            disable_path.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "failed to inspect disable marker {}",
                    disable_path.display()
                )
            });
        }
    };

    Ok(match (enabled, marker_exists) {
        (false, false) => Some(MarkerChange::Create(disable_path)),
        (true, true) => Some(MarkerChange::Remove(disable_path)),
        _ => None,
    })
}

fn rollback_marker_changes(changes: &[MarkerChange]) -> Result<()> {
    for change in changes.iter().rev() {
        change.rollback()?;
    }
    Ok(())
}

fn fail_after_marker_changes(error: anyhow::Error, changes: &[MarkerChange]) -> anyhow::Error {
    match rollback_marker_changes(changes) {
        Ok(()) => error,
        Err(rollback_error) => error.context(format!(
            "additionally failed to roll back module markers: {rollback_error:#}"
        )),
    }
}

pub fn apply_modules_payload(
    config_path: &Path,
    modules: &[ModuleApplyEntry],
) -> Result<ModulesApplyPayload> {
    let mut config = Config::load_from_file(config_path)?;
    let mut validated = Vec::with_capacity(modules.len());
    let mut seen_ids = HashSet::with_capacity(modules.len());
    for module in modules {
        utils::validation::validate_module_id(&module.id)?;
        if !seen_ids.insert(module.id.as_str()) {
            bail!("duplicate module id '{}' in apply request", module.id);
        }
        let module_path = config.moduledir.join(&module.id);
        let metadata = fs::symlink_metadata(&module_path)
            .with_context(|| format!("failed to inspect module path {}", module_path.display()))?;
        if !metadata.file_type().is_dir() {
            bail!("module path '{}' is not a directory", module_path.display());
        }

        validated.push(ValidatedModuleApply {
            entry: module,
            marker_change: plan_marker_change(&module_path, module.enabled)?,
        });
    }

    let mut applied_marker_changes = Vec::new();
    for item in &validated {
        let Some(change) = &item.marker_change else {
            continue;
        };
        if let Err(error) = change.apply() {
            return Err(fail_after_marker_changes(error, &applied_marker_changes));
        }
        applied_marker_changes.push(change.clone());
    }

    for item in &validated {
        config
            .rules
            .insert(item.entry.id.clone(), item.entry.rules.clone());
    }
    if let Err(error) = config.save_to_file(config_path) {
        return Err(fail_after_marker_changes(error, &applied_marker_changes));
    }

    Ok(ModulesApplyPayload {
        updated: modules.len(),
    })
}
