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
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    fs,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

use crate::{
    conf::config,
    core::{
        backend_capabilities::BackendCapabilities,
        inventory::Module,
        ops::plan::{MountPlan, OverlayOperation},
        recovery::ModuleStageFailure,
    },
    defs,
    domain::MountMode,
    partitions,
    sys::fs::{
        PreparedDir, copy_non_dir_entry, ensure_dir_like, finalize_copied_tree,
        prune_orphaned_children, remove_path,
    },
    utils,
};

const SHALLOW_OVERLAY_DIR: &str = ".hybrid_overlay";

#[derive(Debug, Default)]
struct ModulePlanOutcome {
    overlay_groups: BTreeMap<PathBuf, (String, Vec<PathBuf>)>,
    magic: bool,
    kasumi: bool,
}

impl ModulePlanOutcome {
    fn has_mount_result(&self) -> bool {
        !self.overlay_groups.is_empty() || self.magic || self.kasumi
    }
}

#[derive(Debug, Default)]
struct ModulePrepareOutcome {
    has_mount_content: bool,
    opaque_dirs: Vec<PathBuf>,
    plan: ModulePlanOutcome,
}

struct ProcessingItem {
    source_dir: PathBuf,
    copy_dir: PathBuf,
    final_dir: PathBuf,
    shallow_copy_dir: PathBuf,
    shallow_final_dir: PathBuf,
    system_target: PathBuf,
    relative_path: PathBuf,
    partition_label: String,
    plan_active: bool,
    count_mount_content: bool,
}

struct EntryState {
    direct_non_dir_entries: bool,
    has_child_dirs: bool,
    has_replace_marker: bool,
}

struct ModeDecision {
    requested_mode: MountMode,
    effective_mode: MountMode,
    has_descendant_rules: bool,
}

struct PrepareContext {
    use_kasumi: bool,
    managed_partitions: HashSet<String>,
    target_cache: HashMap<PathBuf, PathBuf>,
}

impl PrepareContext {
    fn new(capabilities: &BackendCapabilities, managed_partitions: HashSet<String>) -> Self {
        Self {
            use_kasumi: capabilities.can_use_kasumi(),
            managed_partitions,
            target_cache: HashMap::new(),
        }
    }

    fn resolve_target_cached(&mut self, system_target: &Path) -> PathBuf {
        if let Some(cached) = self.target_cache.get(system_target) {
            return cached.clone();
        }

        let resolved = utils::resolve_link_path(system_target);
        self.target_cache
            .insert(system_target.to_path_buf(), resolved.clone());
        resolved
    }

    fn should_split_overlay_target(&self, resolved_target: &Path) -> bool {
        let target_name = resolved_target
            .file_name()
            .map(|value| value.to_string_lossy())
            .unwrap_or_default();

        target_name == "system" || self.managed_partitions.contains(target_name.as_ref())
    }

    fn process_dir(
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
            return Ok(());
        }
        if !visited_dirs.insert((metadata.dev(), metadata.ino())) {
            return Ok(());
        }

        ensure_dir_like(&item.source_dir, &item.copy_dir)?;
        let relative_key = item.relative_path.to_string_lossy();
        let requested_mode = module.rules.get_mode(relative_key.as_ref());
        let effective_mode = if matches!(requested_mode, MountMode::Kasumi) && !self.use_kasumi {
            MountMode::Ignore
        } else {
            requested_mode
        };
        let mode_decision = ModeDecision {
            requested_mode,
            effective_mode,
            has_descendant_rules: descendant_rule_prefixes.contains(relative_key.as_ref()),
        };
        let needs_shallow_overlay = matches!(mode_decision.effective_mode, MountMode::Overlay)
            && mode_decision.has_descendant_rules;
        drop(relative_key);

        let current_target = if item.plan_active {
            if item.system_target.exists() {
                self.resolve_target_cached(&item.system_target)
            } else {
                item.system_target.clone()
            }
        } else {
            item.system_target.clone()
        };
        let next_partition_label = if item.plan_active {
            current_target
                .file_name()
                .map(|value| value.to_string_lossy().into_owned())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| item.partition_label.clone())
        } else {
            item.partition_label.clone()
        };

        let mut child_dirs = Vec::new();
        let mut direct_non_dir_entries = false;
        let mut has_replace_marker = false;

        for entry_result in fs::read_dir(&item.source_dir)
            .with_context(|| format!("failed to read {}", item.source_dir.display()))?
        {
            let entry = entry_result
                .with_context(|| format!("failed to enumerate {}", item.source_dir.display()))?;
            let file_name = entry.file_name();
            let source_path = entry.path();
            if utils::path_file_name_eq_ignore_ascii_case(&source_path, defs::REPLACE_DIR_FILE_NAME)
            {
                has_replace_marker = true;
                if item.count_mount_content {
                    outcome.has_mount_content = true;
                }
                outcome.opaque_dirs.push(item.copy_dir.clone());
                if needs_shallow_overlay {
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

            let file_type = entry
                .file_type()
                .with_context(|| format!("failed to inspect {}", source_path.display()))?;
            let metadata = fs::symlink_metadata(&source_path).with_context(|| {
                format!("failed to read metadata for {}", source_path.display())
            })?;

            if file_type.is_dir() {
                ensure_dir_like(&source_path, &copy_path)?;
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
                    count_mount_content: item.count_mount_content,
                });
            } else {
                direct_non_dir_entries = true;
                if item.count_mount_content {
                    outcome.has_mount_content = true;
                }
                copy_non_dir_entry(&source_path, &copy_path, &metadata, &file_type)?;
                if needs_shallow_overlay {
                    ensure_dir_like(&item.source_dir, &item.shallow_copy_dir)?;
                    copy_non_dir_entry(&source_path, &shallow_copy_path, &metadata, &file_type)?;
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
            )
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
    ) -> bool {
        log_mode_decision(
            module,
            &item.relative_path,
            &mode_decision.requested_mode,
            &mode_decision.effective_mode,
        );

        let has_any_entries = entry_state.direct_non_dir_entries
            || entry_state.has_child_dirs
            || entry_state.has_replace_marker;
        #[cfg(feature = "control-plane")]
        let has_magic_entries = has_any_entries;
        #[cfg(not(feature = "control-plane"))]
        let has_magic_entries = entry_state.direct_non_dir_entries
            || entry_state.has_replace_marker
            || (entry_state.has_child_dirs && !mode_decision.has_descendant_rules);

        if matches!(mode_decision.effective_mode, MountMode::Magic) && has_magic_entries {
            outcome.plan.magic = true;
        }
        if matches!(mode_decision.effective_mode, MountMode::Kasumi) && has_any_entries {
            outcome.plan.kasumi = true;
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
            return true;
        }

        match mode_decision.effective_mode {
            MountMode::Magic | MountMode::Ignore | MountMode::Kasumi => false,
            MountMode::Overlay => {
                if !item.system_target.exists() {
                    crate::scoped_log!(
                        debug,
                        "prepare",
                        "target skip: module={}, reason=missing_target, path={}",
                        module.id,
                        item.system_target.display()
                    );
                    return false;
                }

                if self.should_split_overlay_target(resolved_target) {
                    return true;
                }

                queue_overlay(
                    &mut outcome.plan,
                    resolved_target.to_path_buf(),
                    &item.partition_label,
                    item.final_dir.clone(),
                );
                false
            }
        }
    }
}

pub fn prepare_mount_plan(
    config: &config::Config,
    modules: &[Module],
    target_base: &Path,
    capabilities: &BackendCapabilities,
) -> Result<MountPlan> {
    prepare_mount_plan_with_root(
        config,
        modules,
        target_base,
        Path::new("/"),
        capabilities,
        partitions::managed_partition_names(),
    )
}

fn prepare_mount_plan_with_root(
    config: &config::Config,
    modules: &[Module],
    target_base: &Path,
    system_root: &Path,
    capabilities: &BackendCapabilities,
    managed_partitions: Vec<String>,
) -> Result<MountPlan> {
    crate::scoped_log!(
        info,
        "prepare",
        "start: modules={}, storage_root={}",
        modules.len(),
        target_base.display()
    );

    if modules.iter().any(module_requests_kasumi) && !capabilities.can_use_kasumi() {
        if config.kasumi.enabled {
            crate::scoped_log!(
                warn,
                "prepare",
                "kasumi fallback: enabled=true, status={}, action=ignore",
                capabilities.kasumi_status()
            );
        } else {
            crate::scoped_log!(
                warn,
                "prepare",
                "kasumi fallback: enabled=false, action=ignore"
            );
        }
    }

    fs::create_dir_all(target_base)
        .with_context(|| format!("failed to create storage root {}", target_base.display()))?;
    prune_orphaned_children(
        target_base,
        modules.iter().map(|module| module.id.as_str()),
        &["lost+found", "hybrid_mount"],
        "prepare",
    )?;

    let module_rank: HashMap<&str, usize> = modules
        .iter()
        .enumerate()
        .map(|(idx, module)| (module.id.as_str(), idx))
        .collect();
    let managed_set = managed_partitions.into_iter().collect::<HashSet<_>>();
    let mut context = PrepareContext::new(capabilities, managed_set);
    let mut overlay_groups: BTreeMap<PathBuf, (String, Vec<PathBuf>)> = BTreeMap::new();
    let mut magic_ids = HashSet::new();
    #[cfg(feature = "kasumi")]
    let mut kasumi_ids = HashSet::new();

    for module in modules {
        crate::scoped_log!(debug, "prepare", "module inspect: id={}", module.id);
        let prepared = PreparedDir::new(target_base, &module.id)
            .map_err(|err| module_sync_error(module, err))?;
        let outcome = prepare_module(
            module,
            prepared.tmp_path(),
            prepared.final_path(),
            system_root,
            &mut context,
        )
        .map_err(|err| module_sync_error(module, err))?;

        let keep_module = outcome.has_mount_content && outcome.plan.has_mount_result();
        if !keep_module {
            crate::scoped_log!(
                debug,
                "prepare",
                "module skip: id={}, reason={}",
                module.id,
                if outcome.has_mount_content {
                    "no_mount_plan"
                } else {
                    "no_mount_content"
                }
            );
            if let Err(err) = remove_path(prepared.final_path()) {
                crate::scoped_log!(
                    warn,
                    "prepare",
                    "cleanup stale module failed: id={}, path={}, error={:#}",
                    module.id,
                    prepared.final_path().display(),
                    err
                );
            }
            continue;
        }

        finalize_copied_tree(&module.id, prepared.tmp_path(), &outcome.opaque_dirs);
        prepared
            .commit()
            .map_err(|err| module_sync_error(module, err))?;

        merge_overlay_groups(&mut overlay_groups, outcome.plan.overlay_groups);
        if outcome.plan.magic {
            magic_ids.insert(module.id.clone());
        }
        #[cfg(feature = "kasumi")]
        if outcome.plan.kasumi {
            kasumi_ids.insert(module.id.clone());
        }
    }

    let mut overlay_module_ids = HashSet::new();
    let mut overlay_ops = Vec::with_capacity(overlay_groups.len());
    for (target_path, (partition_name, mut layers)) in overlay_groups {
        layers.sort_by_cached_key(|path| {
            let module_id = utils::extract_module_id(path).filter(|id| !id.is_empty());
            (
                module_id
                    .as_deref()
                    .and_then(|id| module_rank.get(id))
                    .copied()
                    .unwrap_or(usize::MAX),
                path.clone(),
            )
        });

        for layer in &layers {
            if let Some(module_id) = utils::extract_module_id(layer) {
                overlay_module_ids.insert(module_id);
            }
        }

        crate::scoped_log!(
            info,
            "prepare",
            "overlay op: partition={}, target={}, layers={}",
            partition_name,
            target_path.display(),
            layers.len()
        );

        overlay_ops.push(OverlayOperation {
            partition_name,
            target: target_path.display().to_string(),
            lowerdirs: layers,
        });
    }

    let plan = MountPlan {
        overlay_ops,
        #[cfg(feature = "kasumi")]
        kasumi_add_rules: Vec::new(),
        #[cfg(feature = "kasumi")]
        kasumi_merge_rules: Vec::new(),
        #[cfg(feature = "kasumi")]
        kasumi_hide_rules: Vec::new(),
        overlay_module_ids: sorted_ids(overlay_module_ids),
        magic_module_ids: sorted_ids(magic_ids),
        #[cfg(feature = "kasumi")]
        kasumi_module_ids: sorted_ids(kasumi_ids),
    };

    crate::scoped_log!(
        info,
        "prepare",
        "complete: overlay_ops={}, overlay_modules={}, magic_modules={}, kasumi_modules={}, kasumi_rule_compile=deferred",
        plan.overlay_ops.len(),
        plan.overlay_module_ids.len(),
        plan.magic_module_ids.len(),
        {
            #[cfg(feature = "kasumi")]
            {
                plan.kasumi_module_ids.len()
            }
            #[cfg(not(feature = "kasumi"))]
            {
                0usize
            }
        }
    );

    Ok(plan)
}

fn prepare_module(
    module: &Module,
    tmp_dst: &Path,
    final_dst: &Path,
    system_root: &Path,
    context: &mut PrepareContext,
) -> Result<ModulePrepareOutcome> {
    if !module.source_path.exists() {
        return Ok(ModulePrepareOutcome::default());
    }

    ensure_dir_like(&module.source_path, tmp_dst)?;

    let mut outcome = ModulePrepareOutcome::default();
    let mut queue = VecDeque::new();
    let mut visited_dirs = HashSet::new();
    let descendant_rule_prefixes = module.rules.descendant_rule_prefixes();

    for entry_result in fs::read_dir(&module.source_path)
        .with_context(|| format!("failed to read {}", module.source_path.display()))?
    {
        let entry = entry_result
            .with_context(|| format!("failed to enumerate {}", module.source_path.display()))?;
        let file_name = entry.file_name();
        if utils::path_file_name_eq_ignore_ascii_case(&entry.path(), defs::REPLACE_DIR_FILE_NAME) {
            outcome.opaque_dirs.push(tmp_dst.to_path_buf());
            continue;
        }

        let source_path = entry.path();
        let copy_path = tmp_dst.join(&file_name);
        let final_path = final_dst.join(&file_name);
        let relative_path = PathBuf::from(&file_name);
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect {}", source_path.display()))?;
        let metadata = fs::symlink_metadata(&source_path)
            .with_context(|| format!("failed to read metadata for {}", source_path.display()))?;

        let top_level_partition = file_name
            .to_str()
            .is_some_and(|name| context.managed_partitions.contains(name));

        if file_type.is_dir() {
            ensure_dir_like(&source_path, &copy_path)?;
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
                count_mount_content: top_level_partition,
            });
        } else {
            if top_level_partition {
                outcome.has_mount_content = true;
            }
            copy_non_dir_entry(&source_path, &copy_path, &metadata, &file_type)?;
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

fn module_requests_kasumi(module: &Module) -> bool {
    matches!(module.rules.default_mode, MountMode::Kasumi)
        || module
            .rules
            .paths
            .values()
            .any(|mode| matches!(mode, MountMode::Kasumi))
}

fn module_sync_error(module: &Module, err: anyhow::Error) -> anyhow::Error {
    ModuleStageFailure::sync_one(&module.id, err).into()
}

fn queue_overlay(
    plan: &mut ModulePlanOutcome,
    resolved_target: PathBuf,
    partition_label: &str,
    source: PathBuf,
) {
    crate::scoped_log!(
        debug,
        "prepare",
        "queue overlay: partition={}, layer={}, target={}",
        partition_label,
        source.display(),
        resolved_target.display()
    );
    let (_, layers) = plan
        .overlay_groups
        .entry(resolved_target)
        .or_insert_with(|| (partition_label.to_string(), Vec::new()));
    layers.push(source);
}

fn merge_overlay_groups(
    target: &mut BTreeMap<PathBuf, (String, Vec<PathBuf>)>,
    source: BTreeMap<PathBuf, (String, Vec<PathBuf>)>,
) {
    for (target_path, (partition_name, mut layers)) in source {
        let (_, target_layers) = target
            .entry(target_path)
            .or_insert_with(|| (partition_name, Vec::new()));
        target_layers.append(&mut layers);
    }
}

fn sorted_ids(ids: HashSet<String>) -> Vec<String> {
    let mut out: Vec<String> = ids.into_iter().collect();
    out.sort();
    out
}

fn log_mode_decision(
    module: &Module,
    relative_path: &Path,
    requested_mode: &MountMode,
    effective_mode: &MountMode,
) {
    let relative_display = relative_path.display();
    if requested_mode != effective_mode {
        crate::scoped_log!(
            info,
            "prepare",
            "mode decision: module={}, relative={}, requested={}, effective={}",
            module.id,
            relative_display,
            requested_mode.as_strategy(),
            effective_mode.as_strategy()
        );
    } else {
        crate::scoped_log!(
            debug,
            "prepare",
            "mode decision: module={}, relative={}, requested={}, effective={}",
            module.id,
            relative_display,
            requested_mode.as_strategy(),
            effective_mode.as_strategy()
        );
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use crate::domain::ModuleRules;

    fn write_file(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    fn make_module(
        id: &str,
        source_path: &Path,
        default_mode: MountMode,
        rules: &[(&str, MountMode)],
    ) -> Module {
        Module {
            id: id.to_string(),
            source_path: source_path.to_path_buf(),
            rules: ModuleRules {
                default_mode,
                paths: rules
                    .iter()
                    .map(|(path, mode)| ((*path).to_string(), *mode))
                    .collect(),
            },
        }
    }

    fn test_config() -> config::Config {
        config::Config {
            mountsource: "test".to_string(),
            ..config::Config::default()
        }
    }

    fn prepare_with_root(
        config: &config::Config,
        modules: &[Module],
        target_base: &Path,
        system_root: &Path,
        capabilities: &BackendCapabilities,
    ) -> MountPlan {
        prepare_mount_plan_with_root(
            config,
            modules,
            target_base,
            system_root,
            capabilities,
            vec!["system".to_string()],
        )
        .unwrap()
    }

    #[test]
    fn prepare_mount_plan_builds_overlay_op_from_prepared_storage() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("module");
        write_file(&source.join("system/bin/sh"), "shell");

        let system_root = temp.path().join("sysroot");
        fs::create_dir_all(system_root.join("system/bin")).unwrap();

        let storage = temp.path().join("storage");
        let module = make_module("foo", &source, MountMode::Overlay, &[]);

        let plan = prepare_with_root(
            &test_config(),
            &[module],
            &storage,
            &system_root,
            &BackendCapabilities::default(),
        );

        assert_eq!(plan.overlay_ops.len(), 1);
        assert_eq!(plan.magic_module_ids, Vec::<String>::new());
        assert_eq!(plan.kasumi_module_ids, Vec::<String>::new());
        assert!(storage.join("foo/system/bin/sh").exists());
        assert_eq!(
            plan.overlay_ops[0].target,
            system_root.join("system/bin").display().to_string()
        );
        assert_eq!(
            plan.overlay_ops[0].lowerdirs,
            vec![storage.join("foo/system/bin")]
        );
    }

    #[test]
    fn prepare_mount_plan_marks_magic_modules_without_overlay() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("module");
        write_file(&source.join("system/bin/sh"), "shell");

        let system_root = temp.path().join("sysroot");
        fs::create_dir_all(system_root.join("system/bin")).unwrap();

        let storage = temp.path().join("storage");
        let module = make_module(
            "foo",
            &source,
            MountMode::Overlay,
            &[("system/bin", MountMode::Magic)],
        );

        let plan = prepare_with_root(
            &test_config(),
            &[module],
            &storage,
            &system_root,
            &BackendCapabilities::default(),
        );

        assert!(plan.overlay_ops.is_empty());
        assert_eq!(plan.magic_module_ids, vec!["foo".to_string()]);
        assert_eq!(plan.kasumi_module_ids, Vec::<String>::new());
        assert!(storage.join("foo/system/bin/sh").exists());
    }

    #[test]
    fn prepare_mount_plan_ignores_kasumi_when_unavailable() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("module");
        write_file(&source.join("system/bin/sh"), "shell");

        let system_root = temp.path().join("sysroot");
        fs::create_dir_all(system_root.join("system/bin")).unwrap();

        let storage = temp.path().join("storage");
        let mut config = test_config();
        config.kasumi.enabled = true;
        let module = make_module("foo", &source, MountMode::Kasumi, &[]);

        let plan = prepare_with_root(
            &config,
            &[module],
            &storage,
            &system_root,
            &BackendCapabilities::default(),
        );

        assert!(plan.overlay_ops.is_empty());
        assert!(plan.magic_module_ids.is_empty());
        assert!(plan.kasumi_module_ids.is_empty());
        assert!(!storage.join("foo").exists());
    }

    #[test]
    fn prepare_mount_plan_drops_modules_without_plan_results() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("module");
        write_file(&source.join("system/bin/sh"), "shell");

        let system_root = temp.path().join("sysroot");
        fs::create_dir_all(system_root.join("system/bin")).unwrap();

        let storage = temp.path().join("storage");
        let module = make_module("foo", &source, MountMode::Ignore, &[]);

        let plan = prepare_with_root(
            &test_config(),
            &[module],
            &storage,
            &system_root,
            &BackendCapabilities::default(),
        );

        assert!(plan.overlay_ops.is_empty());
        assert!(plan.magic_module_ids.is_empty());
        assert!(plan.kasumi_module_ids.is_empty());
        assert!(!storage.join("foo").exists());
    }

    #[test]
    fn prepare_mount_plan_preserves_overlay_direct_files_when_subtree_is_split() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("module");
        write_file(&source.join("module.prop"), "id=foo");
        write_file(&source.join("system/etc/permissions.xml"), "permissions");
        write_file(&source.join("system/etc/init/ignored.rc"), "ignored");

        let system_root = temp.path().join("sysroot");
        fs::create_dir_all(system_root.join("system/etc/init")).unwrap();

        let storage = temp.path().join("storage");
        let module = make_module(
            "foo",
            &source,
            MountMode::Overlay,
            &[("system/etc/init", MountMode::Ignore)],
        );

        let plan = prepare_with_root(
            &test_config(),
            &[module],
            &storage,
            &system_root,
            &BackendCapabilities::default(),
        );

        let shallow_etc = storage
            .join("foo")
            .join(SHALLOW_OVERLAY_DIR)
            .join("system/etc");
        assert_eq!(plan.overlay_ops.len(), 1);
        assert_eq!(plan.overlay_module_ids, vec!["foo".to_string()]);
        assert!(plan.magic_module_ids.is_empty());
        assert_eq!(
            plan.overlay_ops[0].target,
            system_root.join("system/etc").display().to_string()
        );
        assert_eq!(plan.overlay_ops[0].lowerdirs, vec![shallow_etc.clone()]);
        assert!(storage.join("foo/system/etc/permissions.xml").exists());
        assert!(shallow_etc.join("permissions.xml").exists());
        assert!(!shallow_etc.join("init").exists());
    }

    #[test]
    fn prepare_mount_plan_preserves_overlay_replace_marker_when_subtree_is_split() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("module");
        write_file(&source.join("module.prop"), "id=foo");
        write_file(&source.join("system/etc/.REPLACE"), "");
        write_file(&source.join("system/etc/init/ignored.rc"), "ignored");

        let system_root = temp.path().join("sysroot");
        fs::create_dir_all(system_root.join("system/etc/init")).unwrap();

        let storage = temp.path().join("storage");
        let module = make_module(
            "foo",
            &source,
            MountMode::Overlay,
            &[("system/etc/init", MountMode::Ignore)],
        );

        let plan = prepare_with_root(
            &test_config(),
            &[module],
            &storage,
            &system_root,
            &BackendCapabilities::default(),
        );

        let shallow_etc = storage
            .join("foo")
            .join(SHALLOW_OVERLAY_DIR)
            .join("system/etc");
        assert_eq!(plan.overlay_ops.len(), 1);
        assert_eq!(
            plan.overlay_ops[0].target,
            system_root.join("system/etc").display().to_string()
        );
        assert_eq!(plan.overlay_ops[0].lowerdirs, vec![shallow_etc.clone()]);
        assert!(shallow_etc.is_dir());
        assert!(!shallow_etc.join(".REPLACE").exists());
        assert!(!shallow_etc.join("init").exists());
    }

    #[test]
    fn prepare_mount_plan_skips_replace_marker_entries() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("module");
        fs::create_dir_all(source.join("system")).unwrap();
        write_file(&source.join("system/.REPLACE"), "");
        write_file(&source.join("system/bin/sh"), "shell");

        let system_root = temp.path().join("sysroot");
        fs::create_dir_all(system_root.join("system/bin")).unwrap();

        let storage = temp.path().join("storage");
        let module = make_module("foo", &source, MountMode::Overlay, &[]);

        let plan = prepare_with_root(
            &test_config(),
            &[module],
            &storage,
            &system_root,
            &BackendCapabilities::default(),
        );

        assert!(!plan.overlay_ops.is_empty());
        assert!(!storage.join("foo/system/.REPLACE").exists());
        assert!(storage.join("foo/system/bin/sh").exists());
    }

    #[test]
    fn prepare_mount_plan_keeps_replace_only_overlay_dir() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("module");
        write_file(&source.join("system/app/.RePlAcE"), "");

        let system_root = temp.path().join("sysroot");
        fs::create_dir_all(system_root.join("system/app")).unwrap();

        let storage = temp.path().join("storage");
        let module = make_module("foo", &source, MountMode::Overlay, &[]);

        let plan = prepare_with_root(
            &test_config(),
            &[module],
            &storage,
            &system_root,
            &BackendCapabilities::default(),
        );

        assert_eq!(plan.overlay_ops.len(), 1);
        assert_eq!(
            plan.overlay_ops[0].target,
            system_root.join("system/app").display().to_string()
        );
        assert!(storage.join("foo/system/app").is_dir());
        assert!(!storage.join("foo/system/app/.RePlAcE").exists());
    }

    #[test]
    fn prepare_mount_plan_marks_magic_for_replace_only_dir() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("module");
        write_file(&source.join("system/.rEpLaCe"), "");

        let system_root = temp.path().join("sysroot");
        fs::create_dir_all(system_root.join("system")).unwrap();

        let storage = temp.path().join("storage");
        let module = make_module("foo", &source, MountMode::Magic, &[]);

        let plan = prepare_with_root(
            &test_config(),
            &[module],
            &storage,
            &system_root,
            &BackendCapabilities::default(),
        );

        assert!(plan.overlay_ops.is_empty());
        assert_eq!(plan.magic_module_ids, vec!["foo".to_string()]);
        assert!(storage.join("foo/system").is_dir());
        assert!(!storage.join("foo/system/.rEpLaCe").exists());
    }
}
