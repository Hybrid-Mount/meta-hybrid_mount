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
    collections::{BTreeMap, HashMap, HashSet},
    path::PathBuf,
};

use crate::domain::MountMode;

pub(super) const SHALLOW_OVERLAY_DIR: &str = ".hybrid_overlay";

#[derive(Debug, Default)]
pub(super) struct ModulePlanOutcome {
    pub(super) overlay_groups: BTreeMap<PathBuf, (String, Vec<PathBuf>)>,
    pub(super) magic: bool,
}

impl ModulePlanOutcome {
    pub(super) fn has_mount_result(&self) -> bool {
        !self.overlay_groups.is_empty() || self.magic
    }
}

#[derive(Debug, Default)]
pub(super) struct ModulePrepareOutcome {
    pub(super) has_mount_content: bool,
    pub(super) opaque_dirs: Vec<PathBuf>,
    pub(super) plan: ModulePlanOutcome,
}

pub(super) struct ProcessingItem {
    pub(super) source_dir: PathBuf,
    pub(super) copy_dir: PathBuf,
    pub(super) final_dir: PathBuf,
    pub(super) shallow_copy_dir: PathBuf,
    pub(super) shallow_final_dir: PathBuf,
    pub(super) system_target: PathBuf,
    pub(super) relative_path: PathBuf,
    pub(super) partition_label: String,
    pub(super) plan_active: bool,
    pub(super) count_mount_content: bool,
}

pub(super) struct EntryState {
    pub(super) direct_non_dir_entries: bool,
    pub(super) has_child_dirs: bool,
    pub(super) has_replace_marker: bool,
}

pub(super) struct ModeDecision {
    pub(super) requested_mode: MountMode,
    pub(super) effective_mode: MountMode,
    pub(super) has_descendant_rules: bool,
}

pub(super) struct PrepareContext {
    pub(super) managed_partitions: HashSet<String>,
    pub(super) target_cache: HashMap<PathBuf, PathBuf>,
}

impl PrepareContext {
    pub(super) fn new(managed_partitions: HashSet<String>) -> Self {
        Self {
            managed_partitions,
            target_cache: HashMap::new(),
        }
    }
}
