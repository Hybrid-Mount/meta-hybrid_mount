// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::{fs, path::Path};

use anyhow::{Context, Result, bail};

use super::{ModuleApplyEntry, ModulesApplyPayload};
use crate::{conf::config::Config, defs, utils};

pub fn apply_modules_payload(
    config_path: &Path,
    modules: &[ModuleApplyEntry],
) -> Result<ModulesApplyPayload> {
    let mut config = Config::load_optional_from_file(config_path)?;
    let canonical_moduledir = config
        .moduledir
        .canonicalize()
        .unwrap_or_else(|_| config.moduledir.clone());

    for module in modules {
        utils::validation::validate_module_id(&module.id)?;
        if let Some(ref sp) = module.source_path {
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
        }

        let module_path = module
            .source_path
            .clone()
            .unwrap_or_else(|| config.moduledir.join(&module.id));
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

        config.rules.insert(module.id.clone(), module.rules.clone());
    }

    config.save_to_file(config_path)?;
    Ok(ModulesApplyPayload {
        updated: modules.len(),
    })
}
