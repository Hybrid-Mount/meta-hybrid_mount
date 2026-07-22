// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::{
    collections::{HashSet, VecDeque},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};

use super::types::{ModulePrepareOutcome, PrepareContext, ProcessingItem, SHALLOW_OVERLAY_DIR};
use crate::{
    core::{failure::ModuleStageFailure, inventory::Module},
    defs,
    domain::MountMode,
    sys::fs::{copy_non_dir_entry, ensure_dir_like},
    utils,
};

pub(super) fn prepare_module(
    module: &Module,
    tmp_dst: &Path,
    final_dst: &Path,
    system_root: &Path,
    context: &mut PrepareContext,
) -> Result<ModulePrepareOutcome> {
    if !module.source_path.is_dir() {
        bail!(
            "module source directory is unavailable: {}",
            module.source_path.display()
        );
    }

    context.metrics.directories_scanned += 1;

    let mut outcome = ModulePrepareOutcome::default();
    let mut queue = VecDeque::new();
    let mut visited_dirs = HashSet::new();
    let descendant_rule_prefixes = module.rules.descendant_rule_prefixes();

    for entry_result in fs::read_dir(&module.source_path)
        .with_context(|| format!("failed to read {}", module.source_path.display()))?
    {
        context.metrics.entries_scanned += 1;
        let entry = entry_result
            .with_context(|| format!("failed to enumerate {}", module.source_path.display()))?;
        let file_name = entry.file_name();
        if utils::path_file_name_eq(&entry.path(), defs::REPLACE_DIR_FILE_NAME) {
            continue;
        }

        let source_path = entry.path();
        let copy_path = tmp_dst.join(&file_name);
        let final_path = final_dst.join(&file_name);
        let relative_path = PathBuf::from(&file_name);
        let metadata = fs::symlink_metadata(&source_path)
            .with_context(|| format!("failed to read metadata for {}", source_path.display()))?;
        let file_type = metadata.file_type();

        let top_level_partition = file_name
            .to_str()
            .is_some_and(|name| context.managed_partitions.contains(name));

        if file_type.is_dir() {
            queue.push_back(ProcessingItem {
                source_dir: source_path,
                copy_dir: copy_path,
                final_dir: final_path,
                shallow_copy_dir: tmp_dst
                    .join(SHALLOW_OVERLAY_DIR)
                    .join(PathBuf::from(&file_name)),
                shallow_final_dir: final_dst
                    .join(SHALLOW_OVERLAY_DIR)
                    .join(PathBuf::from(&file_name)),
                system_target: system_root.join(&file_name),
                relative_path,
                partition_label: file_name.to_string_lossy().into_owned(),
                plan_active: top_level_partition,
                materialize_tree: false,
                count_mount_content: top_level_partition,
            });
        } else {
            if top_level_partition {
                outcome.has_mount_content = true;
            }
        }
    }

    while let Some(item) = queue.pop_front() {
        context.process_dir(
            module,
            item,
            &mut outcome,
            &mut queue,
            &mut visited_dirs,
            &descendant_rule_prefixes,
        )?;
    }

    Ok(outcome)
}

pub(super) fn module_requests_kasumi(module: &Module) -> bool {
    matches!(module.rules.default_mode, MountMode::Kasumi)
        || module
            .rules
            .paths
            .values()
            .any(|mode| matches!(mode, MountMode::Kasumi))
}

pub(super) fn materialize_module_identity(
    module: &Module,
    tmp_dst: &Path,
    context: &mut PrepareContext,
) -> Result<()> {
    let source = module.source_path.join("module.prop");
    let metadata = fs::symlink_metadata(&source)
        .with_context(|| format!("failed to inspect {}", source.display()))?;
    let file_type = metadata.file_type();
    ensure_dir_like(&module.source_path, tmp_dst)?;
    let copied_bytes =
        copy_non_dir_entry(&source, &tmp_dst.join("module.prop"), &metadata, &file_type)?;
    context.record_copy(copied_bytes);
    Ok(())
}

pub(super) fn module_sync_error(module: &Module, err: anyhow::Error) -> anyhow::Error {
    ModuleStageFailure::sync_one(&module.id, err).into()
}
