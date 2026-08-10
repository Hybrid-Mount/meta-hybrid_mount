// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::{
    collections::{HashSet, VecDeque},
    fs,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};

use super::{
    plan_builder::{log_mode_decision, queue_overlay},
    types::{EntryState, ModeDecision, ModulePrepareOutcome, PrepareContext, ProcessingItem},
};
use crate::{
    core::inventory::Module,
    defs,
    domain::MountMode,
    sys::fs::{copy_non_dir_entry, ensure_dir_like},
    utils,
};

impl PrepareContext {
    pub(super) fn resolve_target_cached(&mut self, system_target: &Path) -> Result<PathBuf> {
        if let Some(cached) = self.target_cache.get(system_target) {
            return Ok(cached.clone());
        }

        let resolved = utils::resolve_link_path(system_target)
            .with_context(|| format!("failed to resolve target {}", system_target.display()))?;
        self.target_cache
            .insert(system_target.to_path_buf(), resolved.clone());
        Ok(resolved)
    }

    pub(super) fn should_split_overlay_target(&self, resolved_target: &Path) -> Result<bool> {
        let target_name = resolved_target
            .file_name()
            .and_then(|value| value.to_str())
            .with_context(|| {
                format!(
                    "overlay target has no UTF-8 filename: {}",
                    resolved_target.display()
                )
            })?;

        Ok(target_name == "system" || self.managed_partitions.contains(target_name))
    }

    pub(super) fn process_dir(
        &mut self,
        module: &Module,
        item: ProcessingItem,
        outcome: &mut ModulePrepareOutcome,
        queue: &mut VecDeque<ProcessingItem>,
        visited_dirs: &mut HashSet<(u64, u64)>,
        descendant_rule_prefixes: &HashSet<String>,
    ) -> Result<()> {
        let metadata = fs::symlink_metadata(&item.source_dir)
            .with_context(|| format!("failed to inspect {}", item.source_dir.display()))?;
        if !metadata.file_type().is_dir() {
            bail!(
                "queued source path is not a directory: {}",
                item.source_dir.display()
            );
        }
        if !visited_dirs.insert((metadata.dev(), metadata.ino())) {
            return Ok(());
        }
        self.metrics.directories_scanned += 1;

        let relative_key = item.relative_path.to_string_lossy();
        let requested_mode = module.rules.get_mode(relative_key.as_ref());
        let mode_decision = ModeDecision {
            requested_mode,
            effective_mode: requested_mode,
            has_descendant_rules: descendant_rule_prefixes.contains(relative_key.as_ref()),
        };
        let needs_shallow_overlay = matches!(mode_decision.effective_mode, MountMode::Overlay)
            && mode_decision.has_descendant_rules;
        let materialize_full_overlay = item.materialize_tree
            || (item.plan_active
                && matches!(mode_decision.effective_mode, MountMode::Overlay)
                && !mode_decision.has_descendant_rules);
        let materialize_shallow_overlay =
            item.plan_active && !item.materialize_tree && needs_shallow_overlay;
        drop(relative_key);

        if materialize_full_overlay {
            ensure_dir_like(&item.source_dir, &item.copy_dir)?;
        }

        let current_target = if item.plan_active {
            if item.system_target.exists() {
                self.resolve_target_cached(&item.system_target)?
            } else {
                item.system_target.clone()
            }
        } else {
            item.system_target.clone()
        };
        let next_partition_label = if item.plan_active {
            current_target
                .file_name()
                .and_then(|value| value.to_str())
                .filter(|value| !value.is_empty())
                .with_context(|| {
                    format!(
                        "resolved target has no UTF-8 partition label: {}",
                        current_target.display()
                    )
                })?
                .to_owned()
        } else {
            item.partition_label.clone()
        };

        let mut child_dirs = Vec::new();
        let mut direct_non_dir_entries = false;
        let mut has_replace_marker = false;

        for entry_result in fs::read_dir(&item.source_dir)
            .with_context(|| format!("failed to read {}", item.source_dir.display()))?
        {
            self.metrics.entries_scanned += 1;
            let entry = entry_result
                .with_context(|| format!("failed to enumerate {}", item.source_dir.display()))?;
            let file_name = entry.file_name();
            let source_path = entry.path();
            if utils::path_file_name_eq(&source_path, defs::REPLACE_DIR_FILE_NAME) {
                has_replace_marker = true;
                if item.count_mount_content {
                    outcome.has_mount_content = true;
                }
                if materialize_full_overlay {
                    outcome.opaque_dirs.push(item.copy_dir.clone());
                }
                if materialize_shallow_overlay {
                    ensure_dir_like(&item.source_dir, &item.shallow_copy_dir)?;
                    outcome.opaque_dirs.push(item.shallow_copy_dir.clone());
                }
                continue;
            }

            let copy_path = item.copy_dir.join(&file_name);
            let final_path = item.final_dir.join(&file_name);
            let shallow_copy_path = item.shallow_copy_dir.join(&file_name);
            let shallow_final_path = item.shallow_final_dir.join(&file_name);
            let next_relative = item.relative_path.join(&file_name);

            let metadata = fs::symlink_metadata(&source_path).with_context(|| {
                format!("failed to read metadata for {}", source_path.display())
            })?;
            let file_type = metadata.file_type();

            if file_type.is_dir() {
                child_dirs.push(ProcessingItem {
                    source_dir: source_path,
                    copy_dir: copy_path,
                    final_dir: final_path,
                    shallow_copy_dir: shallow_copy_path,
                    shallow_final_dir: shallow_final_path,
                    system_target: current_target.join(&file_name),
                    relative_path: next_relative,
                    partition_label: next_partition_label.clone(),
                    plan_active: false,
                    materialize_tree: materialize_full_overlay,
                    count_mount_content: item.count_mount_content,
                });
            } else {
                direct_non_dir_entries = true;
                if item.count_mount_content {
                    outcome.has_mount_content = true;
                }
                if materialize_full_overlay {
                    let copied_bytes =
                        copy_non_dir_entry(&source_path, &copy_path, &metadata, &file_type)?;
                    self.record_copy(copied_bytes);
                }
                if materialize_shallow_overlay {
                    ensure_dir_like(&item.source_dir, &item.shallow_copy_dir)?;
                    let copied_bytes = copy_non_dir_entry(
                        &source_path,
                        &shallow_copy_path,
                        &metadata,
                        &file_type,
                    )?;
                    self.record_copy(copied_bytes);
                }
            }
        }

        let child_plan_active = if item.plan_active {
            self.apply_plan_decision(
                module,
                &item,
                &current_target,
                mode_decision,
                EntryState {
                    direct_non_dir_entries,
                    has_child_dirs: !child_dirs.is_empty(),
                    has_replace_marker,
                },
                outcome,
            )?
        } else {
            false
        };

        for mut child in child_dirs {
            child.plan_active = child_plan_active;
            queue.push_back(child);
        }

        Ok(())
    }

    fn apply_plan_decision(
        &mut self,
        module: &Module,
        item: &ProcessingItem,
        resolved_target: &Path,
        mode_decision: ModeDecision,
        entry_state: EntryState,
        outcome: &mut ModulePrepareOutcome,
    ) -> Result<bool> {
        log_mode_decision(
            module,
            &item.relative_path,
            &mode_decision.requested_mode,
            &mode_decision.effective_mode,
        );

        #[cfg(feature = "control-plane")]
        let has_magic_entries = entry_state.direct_non_dir_entries
            || entry_state.has_child_dirs
            || entry_state.has_replace_marker;
        #[cfg(not(feature = "control-plane"))]
        let has_magic_entries = entry_state.direct_non_dir_entries
            || entry_state.has_replace_marker
            || (entry_state.has_child_dirs && !mode_decision.has_descendant_rules);

        if matches!(mode_decision.effective_mode, MountMode::Magic) && has_magic_entries {
            outcome.plan.magic = true;
        }
        let needs_shallow_overlay = matches!(mode_decision.effective_mode, MountMode::Overlay)
            && mode_decision.has_descendant_rules
            && (entry_state.direct_non_dir_entries || entry_state.has_replace_marker);
        if needs_shallow_overlay {
            crate::scoped_log!(
                debug,
                "prepare",
                "mixed overlay subtree split: module={}, relative={}, behavior=shallow_overlay",
                module.id,
                item.relative_path.display()
            );

            if item.system_target.exists() {
                queue_overlay(
                    &mut outcome.plan,
                    resolved_target.to_path_buf(),
                    &item.partition_label,
                    item.shallow_final_dir.clone(),
                );
            } else {
                crate::scoped_log!(
                    debug,
                    "prepare",
                    "target skip: module={}, reason=missing_target, path={}",
                    module.id,
                    item.system_target.display()
                );
            }
        }

        if mode_decision.has_descendant_rules {
            return Ok(true);
        }

        match mode_decision.effective_mode {
            MountMode::Magic | MountMode::Ignore => Ok(false),
            MountMode::Overlay => {
                if !item.system_target.exists() {
                    crate::scoped_log!(
                        debug,
                        "prepare",
                        "target skip: module={}, reason=missing_target, path={}",
                        module.id,
                        item.system_target.display()
                    );
                    return Ok(false);
                }

                if self.should_split_overlay_target(resolved_target)? {
                    return Ok(true);
                }

                queue_overlay(
                    &mut outcome.plan,
                    resolved_target.to_path_buf(),
                    &item.partition_label,
                    item.final_dir.clone(),
                );
                Ok(false)
            }
        }
    }
}
