// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    fs,
    path::{Path, PathBuf},
};

use anyhow::Result;

use crate::{
    conf::config,
    core::inventory::{Module, MountMode},
    defs,
    sys::hymofs,
    utils,
};

#[derive(Debug, Clone)]
pub struct OverlayOperation {
    pub partition_name: String,
    pub target: String,
    pub lowerdirs: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct HymofsAddRule {
    pub target: String,
    pub source: PathBuf,
    pub file_type: i32,
}

#[derive(Debug, Clone)]
pub struct HymofsMergeRule {
    pub target: String,
    pub source: PathBuf,
}

#[derive(Debug, Default)]
pub struct MountPlan {
    pub overlay_ops: Vec<OverlayOperation>,
    pub hymofs_add_rules: Vec<HymofsAddRule>,
    pub hymofs_merge_rules: Vec<HymofsMergeRule>,
    pub hymofs_hide_rules: Vec<String>,
    pub overlay_module_ids: Vec<String>,
    pub magic_module_ids: Vec<String>,
    pub hymofs_module_ids: Vec<String>,
}

struct ProcessingItem {
    module_source: PathBuf,
    system_target: PathBuf,
    partition_label: String,
}

fn build_managed_partitions(config: &config::Config) -> HashSet<String> {
    let mut managed_partitions: HashSet<String> = defs::BUILTIN_PARTITIONS
        .iter()
        .map(|s| s.to_string())
        .collect();
    managed_partitions.insert("system".to_string());
    managed_partitions.extend(config.partitions.iter().cloned());
    managed_partitions
}

fn mode_name(mode: &MountMode) -> &'static str {
    match mode {
        MountMode::Overlay => "overlay",
        MountMode::Magic => "magic",
        MountMode::Hymofs => "hymofs",
        MountMode::Ignore => "ignore",
    }
}

fn effective_mount_mode(requested: &MountMode, use_hymofs: bool) -> MountMode {
    if matches!(requested, MountMode::Hymofs) && !use_hymofs {
        MountMode::Ignore
    } else {
        requested.clone()
    }
}

fn sorted_ids(ids: HashSet<String>) -> Vec<String> {
    let mut out: Vec<String> = ids.into_iter().collect();
    out.sort();
    out
}

pub fn module_requests_hymofs(module: &Module) -> bool {
    matches!(module.rules.default_mode, MountMode::Hymofs)
        || module
            .rules
            .paths
            .values()
            .any(|mode| matches!(mode, MountMode::Hymofs))
}

pub fn hymofs_backend_requested(config: &config::Config, modules: &[Module]) -> bool {
    config.hymofs.enabled
        && hymofs::can_operate(config.hymofs.ignore_protocol_mismatch)
        && modules.iter().any(module_requests_hymofs)
}

fn module_content_path(storage_root: &Path, module: &Module) -> Option<PathBuf> {
    let mut content_path = storage_root.join(&module.id);
    if !content_path.exists() {
        content_path = module.source_path.clone();
    }
    content_path.exists().then_some(content_path)
}

fn resolve_target(system_target: &Path) -> PathBuf {
    let resolved_target = match fs::read_link(system_target) {
        Ok(target) => {
            if target.is_absolute() {
                target
            } else {
                system_target
                    .parent()
                    .unwrap_or(Path::new("/"))
                    .join(target)
            }
        }
        Err(_) => system_target.to_path_buf(),
    };

    if resolved_target.exists() {
        return resolved_target
            .canonicalize()
            .unwrap_or_else(|_| resolved_target.clone());
    }

    if let Some(parent) = resolved_target.parent()
        && parent.exists()
    {
        return parent
            .canonicalize()
            .map(|p| {
                resolved_target
                    .file_name()
                    .map_or_else(|| p.clone(), |name| p.join(name))
            })
            .unwrap_or_else(|_| resolved_target.clone());
    }

    resolved_target
}

fn resolve_target_cached(cache: &mut HashMap<PathBuf, PathBuf>, system_target: &Path) -> PathBuf {
    if let Some(cached) = cache.get(system_target) {
        return cached.clone();
    }

    let resolved = resolve_target(system_target);
    cache.insert(system_target.to_path_buf(), resolved.clone());
    resolved
}

pub fn generate(
    config: &config::Config,
    modules: &[Module],
    storage_root: &Path,
) -> Result<MountPlan> {
    generate_with_root(config, modules, storage_root, Path::new("/"))
}

fn generate_with_root(
    config: &config::Config,
    modules: &[Module],
    storage_root: &Path,
    system_root: &Path,
) -> Result<MountPlan> {
    crate::scoped_log!(
        info,
        "planner",
        "start: modules={}, storage_root={}",
        modules.len(),
        storage_root.display()
    );

    let mut plan = MountPlan::default();

    let mut overlay_groups: BTreeMap<PathBuf, (String, Vec<PathBuf>)> = BTreeMap::new();
    let mut target_cache: HashMap<PathBuf, PathBuf> = HashMap::new();
    let module_rank: HashMap<&str, usize> = modules
        .iter()
        .enumerate()
        .map(|(idx, m)| (m.id.as_str(), idx))
        .collect();

    let mut overlay_ids = HashSet::new();
    let mut magic_ids = HashSet::new();
    let mut hymofs_ids = HashSet::new();

    let sensitive_partitions: HashSet<&str> = defs::SENSITIVE_PARTITIONS.iter().cloned().collect();
    let extra_partitions: HashSet<&str> = config.partitions.iter().map(String::as_str).collect();
    let managed_partitions = build_managed_partitions(config);
    let hymofs_status = hymofs::check_status();
    let use_hymofs =
        config.hymofs.enabled && hymofs::can_operate(config.hymofs.ignore_protocol_mismatch);
    let hymofs_requested = modules.iter().any(module_requests_hymofs);

    if hymofs_requested && !use_hymofs {
        crate::scoped_log!(
            warn,
            "planner",
            "hymofs fallback: enabled={}, status={:?}, action=ignore",
            config.hymofs.enabled,
            hymofs_status
        );
    }

    for module in modules {
        crate::scoped_log!(debug, "planner", "module inspect: id={}", module.id);
        let Some(content_path) = module_content_path(storage_root, module) else {
            crate::scoped_log!(
                debug,
                "planner",
                "module skip: id={}, reason=content_path_missing",
                module.id,
            );
            continue;
        };

        match fs::read_dir(&content_path) {
            Ok(entries) => {
                for entry_result in entries {
                    let entry = match entry_result {
                        Ok(entry) => entry,
                        Err(err) => {
                            crate::scoped_log!(
                                warn,
                                "planner",
                                "enumerate content failed: module={}, path={}, error={}",
                                module.id,
                                content_path.display(),
                                err
                            );
                            continue;
                        }
                    };

                    let path = entry.path();
                    if !path.is_dir() {
                        continue;
                    }

                    match entry.file_type() {
                        Ok(file_type) if file_type.is_symlink() => continue,
                        Ok(_) => {}
                        Err(err) => {
                            crate::scoped_log!(
                                warn,
                                "planner",
                                "file type failed: module={}, path={}, error={}",
                                module.id,
                                path.display(),
                                err
                            );
                            continue;
                        }
                    }

                    let dir_name = entry.file_name();
                    let Some(dir_name) = dir_name.to_str() else {
                        crate::scoped_log!(
                            warn,
                            "planner",
                            "skip: module={}, reason=non_utf8_partition_dir, path={:?}",
                            module.id,
                            path
                        );
                        continue;
                    };

                    if !managed_partitions.contains(dir_name) {
                        continue;
                    }

                    let requested_mode = module.rules.get_mode(dir_name);
                    let effective_mode = effective_mount_mode(&requested_mode, use_hymofs);
                    if requested_mode != effective_mode {
                        crate::scoped_log!(
                            info,
                            "planner",
                            "mode decision: module={}, partition={}, requested={}, effective={}",
                            module.id,
                            dir_name,
                            mode_name(&requested_mode),
                            mode_name(&effective_mode)
                        );
                    } else {
                        crate::scoped_log!(
                            debug,
                            "planner",
                            "mode decision: module={}, partition={}, requested={}, effective={}",
                            module.id,
                            dir_name,
                            mode_name(&requested_mode),
                            mode_name(&effective_mode)
                        );
                    }
                    match effective_mode {
                        MountMode::Magic => {
                            magic_ids.insert(module.id.clone());
                            continue;
                        }
                        MountMode::Ignore => continue,
                        MountMode::Hymofs => {
                            hymofs_ids.insert(module.id.clone());
                            continue;
                        }
                        MountMode::Overlay => {}
                    }

                    overlay_ids.insert(module.id.clone());

                    let mut queue = VecDeque::new();
                    queue.push_back(ProcessingItem {
                        module_source: path.clone(),
                        system_target: system_root.join(dir_name),
                        partition_label: dir_name.to_string(),
                    });

                    while let Some(item) = queue.pop_front() {
                        let ProcessingItem {
                            module_source,
                            system_target,
                            partition_label,
                        } = item;

                        if !system_target.exists() {
                            crate::scoped_log!(
                                debug,
                                "planner",
                                "target skip: module={}, reason=missing_target, path={}",
                                module.id,
                                system_target.display()
                            );
                            continue;
                        }

                        let canonical_target =
                            resolve_target_cached(&mut target_cache, &system_target);

                        let target_name = canonical_target
                            .file_name()
                            .map(|s| s.to_string_lossy())
                            .unwrap_or_default();

                        let should_split = sensitive_partitions.contains(target_name.as_ref())
                            || extra_partitions.contains(target_name.as_ref())
                            || target_name == "system";

                        if should_split {
                            let next_partition_label = if target_name.is_empty() {
                                partition_label.clone()
                            } else {
                                target_name.to_string()
                            };

                            match fs::read_dir(&module_source) {
                                Ok(sub_entries) => {
                                    for sub_entry_result in sub_entries {
                                        let sub_entry = match sub_entry_result {
                                            Ok(sub_entry) => sub_entry,
                                            Err(err) => {
                                                crate::scoped_log!(
                                                    warn,
                                                    "planner",
                                                    "enumerate subtree failed: module={}, path={}, error={}",
                                                    module.id,
                                                    module_source.display(),
                                                    err
                                                );
                                                continue;
                                            }
                                        };
                                        let sub_path = sub_entry.path();
                                        match sub_entry.file_type() {
                                            Ok(file_type) if file_type.is_symlink() => continue,
                                            Ok(file_type) if !file_type.is_dir() => continue,
                                            Ok(_) => {}
                                            Err(err) => {
                                                crate::scoped_log!(
                                                    warn,
                                                    "planner",
                                                    "subtree file type failed: module={}, path={}, error={}",
                                                    module.id,
                                                    sub_path.display(),
                                                    err
                                                );
                                                continue;
                                            }
                                        }
                                        let sub_name = sub_entry.file_name();

                                        queue.push_back(ProcessingItem {
                                            module_source: sub_path,
                                            system_target: canonical_target.join(sub_name),
                                            partition_label: next_partition_label.clone(),
                                        });
                                    }
                                }
                                Err(err) => {
                                    crate::scoped_log!(
                                        warn,
                                        "planner",
                                        "read subtree failed: module={}, path={}, error={}",
                                        module.id,
                                        module_source.display(),
                                        err
                                    );
                                }
                            }
                        } else {
                            crate::scoped_log!(
                                debug,
                                "planner",
                                "queue overlay: module={}, partition={}, layer={}, target={}",
                                module.id,
                                partition_label,
                                module_source.display(),
                                canonical_target.display()
                            );
                            let (_, layers) = overlay_groups
                                .entry(canonical_target)
                                .or_insert_with(|| (partition_label.clone(), Vec::new()));
                            layers.push(module_source);
                        }
                    }
                }
            }
            Err(err) => {
                crate::scoped_log!(
                    warn,
                    "planner",
                    "read content root failed: module={}, path={}, error={}",
                    module.id,
                    content_path.display(),
                    err
                );
            }
        }
    }

    for (target_path, (partition_name, mut layers)) in overlay_groups {
        let target_str = target_path.to_string_lossy().to_string();

        if !target_path.is_dir() {
            continue;
        }

        layers.sort_by_cached_key(|path| {
            let module_id = utils::extract_module_id(path).unwrap_or_default();
            (
                module_rank
                    .get(module_id.as_str())
                    .copied()
                    .unwrap_or(usize::MAX),
                path.clone(),
            )
        });

        crate::scoped_log!(
            info,
            "planner",
            "overlay op: partition={}, target={}, layers={}",
            partition_name,
            target_str,
            layers.len()
        );

        plan.overlay_ops.push(OverlayOperation {
            partition_name,
            target: target_str,
            lowerdirs: layers,
        });
    }

    plan.overlay_module_ids = sorted_ids(overlay_ids);
    plan.magic_module_ids = sorted_ids(magic_ids);
    plan.hymofs_module_ids = sorted_ids(hymofs_ids);

    crate::scoped_log!(
        info,
        "planner",
        "complete: overlay_ops={}, overlay_modules={}, magic_modules={}, hymofs_modules={}, hymofs_rule_compile=deferred",
        plan.overlay_ops.len(),
        plan.overlay_module_ids.len(),
        plan.magic_module_ids.len(),
        plan.hymofs_module_ids.len()
    );

    Ok(plan)
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    use std::{collections::HashMap, fs, path::Path};

    use tempfile::tempdir;

    use super::generate_with_root;
    use crate::{
        conf::config::{Config, ModuleRules, MountMode},
        core::inventory::Module,
    };

    fn module_with_layout(base: &Path, id: &str, dirs: &[&str], rules: ModuleRules) -> Module {
        let module_root = base.join(id);
        fs::create_dir_all(&module_root).expect("failed to create module root");
        fs::write(module_root.join("module.prop"), "name=Test Module\n")
            .expect("failed to write module.prop");

        for dir in dirs {
            fs::create_dir_all(module_root.join(dir)).expect("failed to create module content");
        }

        Module {
            id: id.to_string(),
            source_path: module_root,
            rules,
        }
    }

    #[test]
    fn planner_respects_partition_level_magic_and_ignore_rules() {
        let temp = tempdir().expect("failed to create temp dir");
        let system_root = temp.path().join("rootfs");
        let storage_root = temp.path().join("storage");

        fs::create_dir_all(system_root.join("system/bin")).expect("failed to create system/bin");

        let overlay = module_with_layout(
            &storage_root,
            "mod_overlay",
            &["system/bin"],
            ModuleRules::default(),
        );
        let magic = module_with_layout(
            &storage_root,
            "mod_magic",
            &["system/bin"],
            ModuleRules {
                default_mode: MountMode::Overlay,
                paths: HashMap::from([("system".to_string(), MountMode::Magic)]),
            },
        );
        let ignored = module_with_layout(
            &storage_root,
            "mod_ignore",
            &["system/bin"],
            ModuleRules {
                default_mode: MountMode::Overlay,
                paths: HashMap::from([("system".to_string(), MountMode::Ignore)]),
            },
        );

        let plan = generate_with_root(
            &Config::default(),
            &[overlay, magic, ignored],
            &storage_root,
            &system_root,
        )
        .expect("planner should succeed");

        assert_eq!(plan.overlay_module_ids, vec!["mod_overlay"]);
        assert_eq!(plan.magic_module_ids, vec!["mod_magic"]);
        assert_eq!(plan.overlay_ops.len(), 1);
        assert_eq!(plan.overlay_ops[0].partition_name, "system");
        assert_eq!(
            plan.overlay_ops[0].target,
            system_root.join("system/bin").to_string_lossy()
        );
        assert_eq!(
            plan.overlay_ops[0].lowerdirs,
            vec![storage_root.join("mod_overlay/system/bin")]
        );
    }

    #[test]
    fn planner_splits_sensitive_partitions_and_preserves_module_order_in_layers() {
        let temp = tempdir().expect("failed to create temp dir");
        let system_root = temp.path().join("rootfs");
        let storage_root = temp.path().join("storage");

        fs::create_dir_all(system_root.join("vendor/lib64"))
            .expect("failed to create vendor/lib64");

        let beta = module_with_layout(
            &storage_root,
            "beta_module",
            &["vendor/lib64"],
            ModuleRules::default(),
        );
        let alpha = module_with_layout(
            &storage_root,
            "alpha_module",
            &["vendor/lib64"],
            ModuleRules::default(),
        );

        let plan = generate_with_root(
            &Config::default(),
            &[beta, alpha],
            &storage_root,
            &system_root,
        )
        .expect("planner should succeed");

        assert_eq!(
            plan.overlay_module_ids,
            vec!["alpha_module".to_string(), "beta_module".to_string()]
        );
        assert!(plan.magic_module_ids.is_empty());
        assert_eq!(plan.overlay_ops.len(), 1);
        assert_eq!(plan.overlay_ops[0].partition_name, "vendor");
        assert_eq!(
            plan.overlay_ops[0].target,
            system_root.join("vendor/lib64").to_string_lossy()
        );
        assert_eq!(
            plan.overlay_ops[0].lowerdirs,
            vec![
                storage_root.join("beta_module/vendor/lib64"),
                storage_root.join("alpha_module/vendor/lib64"),
            ]
        );
    }

    #[test]
    fn planner_prefers_runtime_storage_copy_over_module_source_tree() {
        let temp = tempdir().expect("failed to create temp dir");
        let system_root = temp.path().join("rootfs");
        let storage_root = temp.path().join("storage");
        let source_root = temp.path().join("source");

        fs::create_dir_all(system_root.join("system/bin")).expect("failed to create system/bin");
        fs::create_dir_all(system_root.join("system/xbin")).expect("failed to create system/xbin");

        let source_module = module_with_layout(
            &source_root,
            "mod_storage",
            &["system/xbin"],
            ModuleRules::default(),
        );
        let _runtime_copy = module_with_layout(
            &storage_root,
            "mod_storage",
            &["system/bin"],
            ModuleRules::default(),
        );

        let plan = generate_with_root(
            &Config::default(),
            &[Module {
                id: source_module.id.clone(),
                source_path: source_module.source_path.clone(),
                rules: source_module.rules.clone(),
            }],
            &storage_root,
            &system_root,
        )
        .expect("planner should succeed");

        assert_eq!(plan.overlay_ops.len(), 1);
        assert_eq!(
            plan.overlay_ops[0].target,
            system_root.join("system/bin").to_string_lossy()
        );
        assert_eq!(
            plan.overlay_ops[0].lowerdirs,
            vec![storage_root.join("mod_storage/system/bin")]
        );
    }

    #[test]
    fn planner_falls_back_to_ignore_when_hymofs_is_disabled() {
        let temp = tempdir().expect("failed to create temp dir");
        let system_root = temp.path().join("rootfs");
        let storage_root = temp.path().join("storage");

        fs::create_dir_all(system_root.join("system/bin")).expect("failed to create system/bin");

        let module = module_with_layout(
            &storage_root,
            "mod_hymofs",
            &["system/bin"],
            ModuleRules {
                default_mode: MountMode::Hymofs,
                ..ModuleRules::default()
            },
        );

        let plan = generate_with_root(&Config::default(), &[module], &storage_root, &system_root)
            .expect("planner should succeed");

        assert!(plan.overlay_ops.is_empty());
        assert!(plan.overlay_module_ids.is_empty());
        assert!(plan.magic_module_ids.is_empty());
        assert!(plan.hymofs_module_ids.is_empty());
    }

    #[test]
    fn planner_includes_configured_extra_partitions() {
        let temp = tempdir().expect("failed to create temp dir");
        let system_root = temp.path().join("rootfs");
        let storage_root = temp.path().join("storage");

        fs::create_dir_all(system_root.join("my_custom/app"))
            .expect("failed to create custom partition");

        let module = module_with_layout(
            &storage_root,
            "mod_custom",
            &["my_custom/app"],
            ModuleRules::default(),
        );

        let config = Config {
            partitions: vec!["my_custom".to_string()],
            ..Config::default()
        };

        let plan = generate_with_root(&config, &[module], &storage_root, &system_root)
            .expect("planner should succeed");

        assert_eq!(plan.overlay_ops.len(), 1);
        assert_eq!(plan.overlay_ops[0].partition_name, "my_custom");
        assert_eq!(
            plan.overlay_ops[0].target,
            system_root.join("my_custom/app").to_string_lossy()
        );
    }

    #[test]
    #[cfg(unix)]
    fn planner_tracks_symlinked_system_partitions_under_real_partition_names() {
        let temp = tempdir().expect("failed to create temp dir");
        let system_root = temp.path().join("rootfs");
        let storage_root = temp.path().join("storage");

        fs::create_dir_all(system_root.join("system")).expect("failed to create system root");
        fs::create_dir_all(system_root.join("vendor/lib64"))
            .expect("failed to create vendor/lib64");
        symlink("../vendor", system_root.join("system/vendor"))
            .expect("failed to create /system/vendor symlink");

        let module = module_with_layout(
            &storage_root,
            "mod_vendor",
            &["system/vendor/lib64"],
            ModuleRules::default(),
        );

        let plan = generate_with_root(&Config::default(), &[module], &storage_root, &system_root)
            .expect("planner should succeed");

        assert_eq!(plan.overlay_ops.len(), 1);
        assert_eq!(plan.overlay_ops[0].partition_name, "vendor");
        assert_eq!(
            plan.overlay_ops[0].target,
            system_root.join("vendor/lib64").to_string_lossy()
        );
        assert_eq!(
            plan.overlay_ops[0].lowerdirs,
            vec![storage_root.join("mod_vendor/system/vendor/lib64")]
        );
    }

    #[test]
    #[cfg(unix)]
    fn planner_skips_symlinked_partition_aliases_when_real_partition_exists() {
        let temp = tempdir().expect("failed to create temp dir");
        let system_root = temp.path().join("rootfs");
        let storage_root = temp.path().join("storage");
        let module_root = storage_root.join("mod_product");

        fs::create_dir_all(system_root.join("system")).expect("failed to create system root");
        fs::create_dir_all(system_root.join("product/overlay"))
            .expect("failed to create product/overlay");
        symlink("../product", system_root.join("system/product"))
            .expect("failed to create /system/product symlink");
        fs::create_dir_all(module_root.join("product/overlay"))
            .expect("failed to create module product/overlay");
        fs::create_dir_all(module_root.join("system")).expect("failed to create module system dir");
        fs::write(module_root.join("module.prop"), "name=Test Module\n")
            .expect("failed to write module.prop");
        symlink("../product", module_root.join("system/product"))
            .expect("failed to create module /system/product symlink");

        let module = Module {
            id: "mod_product".to_string(),
            source_path: module_root,
            rules: ModuleRules::default(),
        };

        let plan = generate_with_root(&Config::default(), &[module], &storage_root, &system_root)
            .expect("planner should succeed");

        assert_eq!(plan.overlay_ops.len(), 1);
        assert_eq!(plan.overlay_ops[0].partition_name, "product");
        assert_eq!(
            plan.overlay_ops[0].target,
            system_root.join("product/overlay").to_string_lossy()
        );
        assert_eq!(
            plan.overlay_ops[0].lowerdirs,
            vec![storage_root.join("mod_product/product/overlay")]
        );
    }
}
