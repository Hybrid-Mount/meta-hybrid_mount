// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::{collections::HashSet, ffi::OsStr, fs, path::Path};

use anyhow::{Context, Result, bail};

use super::{ModuleApplyEntry, ModulesApplyPayload};
use crate::{conf::config::Config, defs, utils};

struct ValidatedModuleApply<'a> {
    entry: &'a ModuleApplyEntry,
    module_path: std::path::PathBuf,
}

pub fn apply_modules_payload(
    config_path: &Path,
    modules: &[ModuleApplyEntry],
) -> Result<ModulesApplyPayload> {
    let mut config = Config::load_optional_from_file(config_path)?;
    let canonical_moduledir = config
        .moduledir
        .canonicalize()
        .unwrap_or_else(|_| config.moduledir.clone());

    let mut validated = Vec::with_capacity(modules.len());
    let mut seen_ids = HashSet::with_capacity(modules.len());
    for module in modules {
        utils::validation::validate_module_id(&module.id)?;
        if !seen_ids.insert(module.id.as_str()) {
            bail!("duplicate module id '{}' in apply request", module.id);
        }
        let module_path = if let Some(ref sp) = module.source_path {
            if sp.file_name() != Some(OsStr::new(&module.id)) {
                bail!(
                    "source_path '{}' does not match module id '{}'",
                    sp.display(),
                    module.id
                );
            }
            let canonical_sp = sp
                .canonicalize()
                .with_context(|| format!("failed to canonicalize source_path {}", sp.display()))?;
            if !canonical_sp.starts_with(&canonical_moduledir) {
                bail!(
                    "source_path '{}' is outside moduledir '{}'",
                    sp.display(),
                    config.moduledir.display()
                );
            }
            canonical_sp
        } else {
            config.moduledir.join(&module.id)
        };

        if !module_path.is_dir() {
            bail!("module path '{}' is not a directory", module_path.display());
        }

        validated.push(ValidatedModuleApply {
            entry: module,
            module_path,
        });
    }

    // Persist all rule changes only after every entry has passed validation.
    // This prevents a bad item late in the batch from leaving earlier marker
    // files modified while the configuration remains unchanged.
    for item in &validated {
        config
            .rules
            .insert(item.entry.id.clone(), item.entry.rules.clone());
    }
    config.save_to_file(config_path)?;

    for item in validated {
        let module = item.entry;
        let module_path = item.module_path;
        let disable_path = module_path.join(defs::DISABLE_FILE_NAME);

        if module.enabled == Some(false) {
            utils::remove_dir_entries_case_insensitive(&module_path, defs::DISABLE_FILE_NAME)?;
            fs::write(&disable_path, b"").with_context(|| {
                format!("failed to create disable marker {}", disable_path.display())
            })?;
        } else if module.enabled == Some(true) {
            utils::remove_dir_entries_case_insensitive(&module_path, defs::DISABLE_FILE_NAME)
                .with_context(|| {
                    format!("failed to remove disable marker {}", disable_path.display())
                })?;
        }
    }
    Ok(ModulesApplyPayload {
        updated: modules.len(),
    })
}
