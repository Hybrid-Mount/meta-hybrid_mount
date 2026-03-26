// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    fs,
    path::{Path, PathBuf},
};

use anyhow::Result;
use rayon::prelude::*;
use serde::Serialize;
use walkdir::WalkDir;

use crate::{
    conf::config,
    core::inventory::{Module, MountMode},
    defs, utils,
};

#[derive(Debug, Clone)]
pub struct OverlayOperation {
    pub partition_name: String,
    pub target: String,
    pub lowerdirs: Vec<PathBuf>,
}

#[derive(Debug, Default)]
pub struct MountPlan {
    pub overlay_ops: Vec<OverlayOperation>,
    pub overlay_module_ids: Vec<String>,
    pub magic_module_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConflictEntry {
    pub partition: String,
    pub relative_path: String,
    pub contending_modules: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub enum DiagnosticLevel {
    Warning,
    Critical,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticIssue {
    pub level: DiagnosticLevel,
    pub context: String,
    pub message: String,
}

#[derive(Debug, Default)]
pub struct AnalysisReport {
    pub conflicts: Vec<ConflictEntry>,
    pub diagnostics: Vec<DiagnosticIssue>,
}

impl MountPlan {
    pub fn analyze(&self) -> AnalysisReport {
        let results: Vec<(Vec<ConflictEntry>, Vec<DiagnosticIssue>)> = self
            .overlay_ops
            .par_iter()
            .map(|op| {
                let mut local_conflicts = Vec::new();
                let mut local_diagnostics = Vec::new();
                let mut file_map: HashMap<String, Vec<String>> = HashMap::new();

                if !Path::new(&op.target).exists() {
                    local_diagnostics.push(DiagnosticIssue {
                        level: DiagnosticLevel::Critical,
                        context: op.partition_name.clone(),
                        message: format!("Target mount point does not exist: {}", op.target),
                    });
                }

                for layer_path in &op.lowerdirs {
                    if !layer_path.exists() {
                        continue;
                    }

                    let module_id =
                        utils::extract_module_id(layer_path).unwrap_or_else(|| "UNKNOWN".into());

                    for entry in WalkDir::new(layer_path).min_depth(1).into_iter().flatten() {
                        if entry.path_is_symlink()
                            && let Ok(target) = std::fs::read_link(entry.path())
                            && target.is_absolute()
                            && !target.exists()
                        {
                            local_diagnostics.push(DiagnosticIssue {
                                level: DiagnosticLevel::Warning,
                                context: module_id.clone(),
                                message: format!(
                                    "Dead absolute symlink: {} -> {}",
                                    entry.path().display(),
                                    target.display()
                                ),
                            });
                        }

                        if !entry.file_type().is_file() {
                            continue;
                        }

                        if let Ok(rel) = entry.path().strip_prefix(layer_path) {
                            let rel_str = rel.to_string_lossy().to_string();
                            file_map.entry(rel_str).or_default().push(module_id.clone());
                        }
                    }
                }

                for (rel_path, modules) in file_map {
                    if modules.len() > 1 {
                        local_conflicts.push(ConflictEntry {
                            partition: op.partition_name.clone(),
                            relative_path: rel_path,
                            contending_modules: modules,
                        });
                    }
                }

                (local_conflicts, local_diagnostics)
            })
            .collect();

        let mut report = AnalysisReport::default();
        for (c, d) in results {
            report.conflicts.extend(c);
            report.diagnostics.extend(d);
        }

        report.conflicts.sort_by(|a, b| {
            a.partition
                .cmp(&b.partition)
                .then_with(|| a.relative_path.cmp(&b.relative_path))
        });

        report
    }
}

struct ProcessingItem {
    module_source: PathBuf,
    system_target: PathBuf,
    partition_label: String,
}

pub fn generate(
    config: &config::Config,
    modules: &[Module],
    storage_root: &Path,
) -> Result<MountPlan> {
    log::info!(
        "[planner] start generating mount plan: modules={}, storage_root={}",
        modules.len(),
        storage_root.display()
    );

    let mut plan = MountPlan::default();

    let mut overlay_groups: BTreeMap<PathBuf, Vec<PathBuf>> = BTreeMap::new();
    let module_rank: HashMap<&str, usize> = modules
        .iter()
        .enumerate()
        .map(|(idx, m)| (m.id.as_str(), idx))
        .collect();

    let mut overlay_ids = HashSet::new();
    let mut magic_ids = HashSet::new();

    let sensitive_partitions: HashSet<&str> = defs::SENSITIVE_PARTITIONS.iter().cloned().collect();
    let mut managed_partitions: HashSet<String> = defs::BUILTIN_PARTITIONS
        .iter()
        .map(|s| s.to_string())
        .collect();
    managed_partitions.insert("system".to_string());
    managed_partitions.extend(config.partitions.iter().cloned());

    for module in modules {
        log::debug!("[planner] evaluating module={}", module.id);
        let mut content_path = storage_root.join(&module.id);
        if !content_path.exists() {
            content_path = module.source_path.clone();
        }
        if !content_path.exists() {
            log::debug!(
                "[planner] skip module={} because content path not found: {}",
                module.id,
                content_path.display()
            );
            continue;
        }

        if let Ok(entries) = fs::read_dir(&content_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }

                if let Ok(file_type) = entry.file_type()
                    && file_type.is_symlink()
                {
                    continue;
                }

                let dir_name = entry.file_name();
                let Some(dir_name) = dir_name.to_str() else {
                    continue;
                };

                if !managed_partitions.contains(dir_name) {
                    continue;
                }

                let mode = module.rules.get_mode(dir_name);
                if matches!(mode, MountMode::Magic) {
                    magic_ids.insert(module.id.clone());
                    log::info!(
                        "[planner] module={} partition={} forced to magic mount",
                        module.id,
                        dir_name
                    );
                    continue;
                }
                if matches!(mode, MountMode::Ignore) {
                    log::debug!(
                        "[planner] module={} partition={} ignored by rule",
                        module.id,
                        dir_name
                    );
                    continue;
                }

                overlay_ids.insert(module.id.clone());

                let mut queue = VecDeque::new();
                queue.push_back(ProcessingItem {
                    module_source: path.clone(),
                    system_target: Path::new("/").join(dir_name),
                    partition_label: dir_name.to_string(),
                });

                while let Some(item) = queue.pop_front() {
                    let ProcessingItem {
                        module_source,
                        system_target,
                        partition_label,
                    } = item;

                    if !system_target.exists() {
                        log::debug!(
                            "[planner] skip missing target for module={}: {}",
                            module.id,
                            system_target.display()
                        );
                        continue;
                    }

                    let resolved_target = match fs::read_link(&system_target) {
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
                        Err(_) => system_target.clone(),
                    };
                    let canonical_target = if resolved_target.exists() {
                        resolved_target
                            .canonicalize()
                            .unwrap_or_else(|_| resolved_target.clone())
                    } else if let Some(parent) = resolved_target.parent() {
                        if parent.exists() {
                            parent
                                .canonicalize()
                                .map(|p| {
                                    resolved_target
                                        .file_name()
                                        .map_or_else(|| p.clone(), |name| p.join(name))
                                })
                                .unwrap_or_else(|_| resolved_target.clone())
                        } else {
                            resolved_target.clone()
                        }
                    } else {
                        resolved_target.clone()
                    };

                    let target_name = canonical_target
                        .file_name()
                        .map(|s| s.to_string_lossy())
                        .unwrap_or_default();

                    let should_split = sensitive_partitions.contains(target_name.as_ref())
                        || target_name == "system";

                    if should_split {
                        if let Ok(sub_entries) = fs::read_dir(&module_source) {
                            for sub_entry in sub_entries.flatten() {
                                let sub_path = sub_entry.path();
                                if !sub_path.is_dir() {
                                    continue;
                                }
                                let sub_name = sub_entry.file_name();

                                queue.push_back(ProcessingItem {
                                    module_source: sub_path,
                                    system_target: canonical_target.join(sub_name),
                                    partition_label: partition_label.clone(),
                                });
                            }
                        }
                    } else {
                        log::debug!(
                            "[planner] queue overlay layer: module={}, partition={}, layer={}, target={}",
                            module.id,
                            partition_label,
                            module_source.display(),
                            canonical_target.display()
                        );
                        overlay_groups
                            .entry(canonical_target)
                            .or_default()
                            .push(module_source);
                    }
                }
            }
        }
    }

    for (target_path, mut layers) in overlay_groups {
        let target_str = target_path.to_string_lossy().to_string();

        if !target_path.is_dir() {
            continue;
        }

        layers.sort_by(|a, b| {
            let aid = utils::extract_module_id(a).unwrap_or_default();
            let bid = utils::extract_module_id(b).unwrap_or_default();
            let ar = module_rank.get(aid.as_str()).copied().unwrap_or(usize::MAX);
            let br = module_rank.get(bid.as_str()).copied().unwrap_or(usize::MAX);

            ar.cmp(&br).then_with(|| a.cmp(b))
        });

        let partition_name = target_path
            .iter()
            .nth(1)
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        log::info!(
            "[planner] add overlay op: partition={}, target={}, layers={}",
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

    plan.overlay_module_ids = overlay_ids.into_iter().collect();
    plan.magic_module_ids = magic_ids.into_iter().collect();
    plan.overlay_module_ids.sort();
    plan.magic_module_ids.sort();

    log::info!(
        "[planner] plan generated: overlay_ops={}, overlay_modules={}, magic_modules={}",
        plan.overlay_ops.len(),
        plan.overlay_module_ids.len(),
        plan.magic_module_ids.len()
    );

    Ok(plan)
}
