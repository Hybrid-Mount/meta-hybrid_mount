// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::Result;
use rayon::prelude::*;
use serde::Deserialize;

use crate::{
    conf::config::{self, ModuleRules, MountMode},
    defs,
};

fn load_module_rules(module_dir: &Path, module_id: &str, cfg: &config::Config) -> ModuleRules {
    let mut rules = ModuleRules {
        default_mode: match cfg.default_mode {
            config::DefaultMode::Overlay => MountMode::Overlay,
            config::DefaultMode::Magic => MountMode::Magic,
        },
        ..Default::default()
    };

    #[derive(Deserialize)]
    struct PartialRules {
        default_mode: Option<MountMode>,
        paths: Option<HashMap<String, MountMode>>,
    }

    let internal_config = module_dir.join("hybrid_rules.json");

    if internal_config.exists() {
        match fs::read_to_string(&internal_config) {
            Ok(content) => match serde_json::from_str::<PartialRules>(&content) {
                Ok(partial) => {
                    if let Some(mode) = partial.default_mode {
                        rules.default_mode = mode;
                    }
                    if let Some(paths) = partial.paths {
                        rules.paths = paths;
                    }
                }
                Err(e) => {
                    log::warn!("Failed to parse rules for module '{}': {}", module_id, e)
                }
            },
            Err(e) => log::warn!("Failed to read rule file for '{}': {}", module_id, e),
        }
    }

    if let Some(global_rules) = cfg.rules.get(module_id) {
        rules.default_mode = global_rules.default_mode.clone();
        rules.paths.extend(global_rules.paths.clone());
    }

    rules
}

#[derive(Debug, Clone)]
pub struct Module {
    pub id: String,
    pub source_path: PathBuf,
    pub rules: ModuleRules,
}

pub fn scan(source_dir: &Path, cfg: &config::Config) -> Result<Vec<Module>> {
    if !source_dir.exists() {
        return Ok(Vec::new());
    }

    let dir_entries = fs::read_dir(source_dir)?.collect::<std::io::Result<Vec<_>>>()?;

    let mut modules: Vec<Module> = dir_entries
        .into_par_iter()
        .filter_map(|entry| {
            let path = entry.path();

            if !path.is_dir() {
                return None;
            }

            let id = entry.file_name().to_string_lossy().to_string();

            if matches!(
                id.as_str(),
                "meta-hybrid" | "lost+found" | ".git" | ".idea" | ".vscode"
            ) {
                return None;
            }

            if path.join(defs::DISABLE_FILE_NAME).exists()
                || path.join(defs::REMOVE_FILE_NAME).exists()
                || path.join(defs::SKIP_MOUNT_FILE_NAME).exists()
            {
                return None;
            }

            let rules = load_module_rules(&path, &id, cfg);

            Some(Module {
                id,
                source_path: path,
                rules,
            })
        })
        .collect();

    modules.sort_by(|a, b| b.id.cmp(&a.id));

    Ok(modules)
}
