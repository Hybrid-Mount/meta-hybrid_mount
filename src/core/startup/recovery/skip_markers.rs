// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::{
    collections::{HashMap, HashSet},
    fs::OpenOptions,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

use crate::{defs, utils};

pub(super) struct MarkOutcome {
    pub(super) newly_marked: Vec<String>,
    pub(super) already_marked: Vec<String>,
    pub(super) unknown_modules: Vec<String>,
}

pub(super) fn mark_failed_modules(
    module_ids: &[String],
    module_dirs: &HashMap<String, PathBuf>,
    auto_skipped: &mut HashSet<String>,
) -> Result<MarkOutcome> {
    let mut newly_marked = Vec::new();
    let mut already_marked = Vec::new();
    let mut unknown_modules = Vec::new();

    for module_id in module_ids {
        if auto_skipped.contains(module_id) {
            already_marked.push(module_id.clone());
            continue;
        }
        if let Some(module_dir) = module_dirs.get(module_id) {
            create_mount_error_marker(module_dir)?;
            auto_skipped.insert(module_id.clone());
            newly_marked.push(module_id.clone());
        } else {
            unknown_modules.push(module_id.clone());
        }
    }

    Ok(MarkOutcome {
        newly_marked,
        already_marked,
        unknown_modules,
    })
}

pub(super) fn list_module_dirs(base: &Path) -> Result<HashMap<String, PathBuf>> {
    let mut modules = HashMap::new();
    if !base.exists() {
        return Ok(modules);
    }

    for entry in std::fs::read_dir(base)?.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let id = entry.file_name().to_string_lossy().into_owned();
        if crate::core::inventory::is_reserved_module_dir(&id) {
            continue;
        }
        modules.insert(id, path);
    }

    Ok(modules)
}

fn create_mount_error_marker(module_dir: &Path) -> Result<()> {
    let marker = module_dir.join(defs::MOUNT_ERROR_FILE_NAME);
    utils::remove_dir_entries_case_insensitive(module_dir, defs::MOUNT_ERROR_FILE_NAME)
        .with_context(|| {
            format!(
                "Failed to clear existing markers in {}",
                module_dir.display()
            )
        })?;
    crate::scoped_log!(
        info,
        "recovery:markers",
        "create mount error marker: path={}",
        marker.display()
    );
    OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&marker)
        .with_context(|| format!("Failed to create {}", marker.display()))?;
    crate::scoped_log!(
        debug,
        "recovery:markers",
        "mount error marker ready: module_dir={}",
        module_dir.display()
    );
    Ok(())
}
