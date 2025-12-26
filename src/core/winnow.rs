// Copyright 2025 Meta-Hybrid Mount Authors
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{conf::config::WinnowingTable, core::planner::ConflictEntry};

#[derive(Debug, Serialize, Deserialize)]

pub struct ChaffConflict {
    pub path: PathBuf,
    pub contenders: Vec<String>,
    pub selected: String,
    pub is_forced: bool,
}

pub fn sift_conflicts(conflicts: Vec<ConflictEntry>, table: &WinnowingTable) -> Vec<ChaffConflict> {
    conflicts
        .into_iter()
        .map(|c| {
            let path_str = format!("/{}/{}", c.partition, c.relative_path);

            let forced_module = table.get_preferred_module(Path::new(&path_str));

            let selected = if let Some(forced) = &forced_module {
                if c.contending_modules.contains(forced) {
                    forced.clone()
                } else {
                    c.contending_modules
                        .last()
                        .unwrap_or(&"unknown".to_string())
                        .clone()
                }
            } else {
                c.contending_modules
                    .last()
                    .unwrap_or(&"unknown".to_string())
                    .clone()
            };

            ChaffConflict {
                path: PathBuf::from(path_str),
                contenders: c.contending_modules,
                selected,
                is_forced: forced_module.is_some(),
            }
        })
        .collect()
}
