// Copyright 2025 Meta-Hybrid Mount Authors
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::Result;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{conf::config, defs};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]

pub enum MountMode {
    #[default]
    Overlay,
    Magic,
    Ignore,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]

pub struct ModuleRules {
    #[serde(default)]
    pub default_mode: MountMode,
    #[serde(default)]
    pub paths: HashMap<String, MountMode>,
}

impl ModuleRules {
    pub fn load(module_dir: &Path, module_id: &str) -> Self {
        let mut rules = ModuleRules::default();

        let internal_config = module_dir.join("hybrid_rules.json");

        if internal_config.exists() {
            match fs::read_to_string(&internal_config) {
                Ok(content) => match serde_json::from_str::<ModuleRules>(&content) {
                    Ok(r) => rules = r,
                    Err(e) => log::warn!("Failed to parse rules for module '{}': {}", module_id, e),
                },
                Err(e) => log::warn!("Failed to read rule file for '{}': {}", module_id, e),
            }
        }

        let user_rules_dir = Path::new("/data/adb/meta-hybrid/rules");

        let user_config = user_rules_dir.join(format!("{}.json", module_id));

        if user_config.exists() {
            match fs::read_to_string(&user_config) {
                Ok(content) => match serde_json::from_str::<ModuleRules>(&content) {
                    Ok(user_rules) => {
                        rules.default_mode = user_rules.default_mode;

                        rules.paths.extend(user_rules.paths);
                    }
                    Err(e) => log::warn!("Failed to parse user rules for '{}': {}", module_id, e),
                },
                Err(e) => log::warn!("Failed to read user rule file for '{}': {}", module_id, e),
            }
        }

        rules
    }

    pub fn get_mode(&self, relative_path: &str) -> MountMode {
        if let Some(mode) = self.paths.get(relative_path) {
            return mode.clone();
        }

        self.default_mode.clone()
    }
}

#[derive(Debug, Clone)]

pub struct Module {
    pub id: String,
    pub source_path: PathBuf,
    pub rules: ModuleRules,
}

pub fn scan(source_dir: &Path, _config: &config::Config) -> Result<Vec<Module>> {
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

            // Centralized ignore list for system directories
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

            let rules = ModuleRules::load(&path, &id);

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
