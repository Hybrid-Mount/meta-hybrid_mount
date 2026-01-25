// Copyright 2025 Meta-Hybrid Mount Authors
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    collections::{HashMap, HashSet, VecDeque},
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
    #[allow(dead_code)]
    Info,
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

#[allow(clippy::collapsible_if)]
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

                    // Check strictly for dead symlinks or other issues
                    for entry in WalkDir::new(layer_path).min_depth(1).into_iter().flatten() {
                        if entry.path_is_symlink() {
                            if let Ok(target) = std::fs::read_link(entry.path()) {
                                if target.is_absolute() && !target.exists() {
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
                            }
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
    let mut plan = MountPlan::default();

    let mut overlay_groups: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();

    let mut overlay_ids = HashSet::new();
    let mut magic_ids = HashSet::new();

    let sensitive_partitions: HashSet<&str> = defs::SENSITIVE_PARTITIONS.iter().cloned().collect();

    for module in modules {
        let mut content_path = storage_root.join(&module.id);
        if !content_path.exists() {
            content_path = module.source_path.clone();
        }
        if !content_path.exists() {
            continue;
        }

        if let Ok(entries) = fs::read_dir(&content_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }

                let dir_name = entry.file_name().to_string_lossy().to_string();

                if !defs::BUILTIN_PARTITIONS.contains(&dir_name.as_str())
                    && !config.partitions.contains(&dir_name)
                {
                    continue;
                }

                let mode = module.rules.get_mode(&dir_name);
                if matches!(mode, MountMode::Magic) {
                    magic_ids.insert(module.id.clone());
                    // Magic mount logic would go here separately or fallback
                    continue;
                }
                if matches!(mode, MountMode::Ignore) {
                    continue;
                }

                overlay_ids.insert(module.id.clone());

                let mut queue = VecDeque::new();
                queue.push_back(ProcessingItem {
                    module_source: path.clone(),
                    system_target: PathBuf::from("/").join(&dir_name),
                    partition_label: dir_name.clone(),
                });

                while let Some(item) = queue.pop_front() {
                    let ProcessingItem {
                        module_source,
                        system_target,
                        partition_label,
                    } = item;

                    if !system_target.exists() {
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
                        match resolved_target.canonicalize() {
                            Ok(p) => p,
                            Err(_) => resolved_target,
                        }
                    } else {
                        resolved_target
                    };

                    let target_name = canonical_target
                        .file_name()
                        .map(|s| s.to_string_lossy())
                        .unwrap_or_default();

                    let should_split = sensitive_partitions.contains(target_name.as_ref())
                        || target_name == "system"; // 总是尝试拆解 /system 以发现内部的软链接

                    if should_split {
                        // 遍历模块内的该目录，将子项加入队列
                        if let Ok(sub_entries) = fs::read_dir(&module_source) {
                            for sub_entry in sub_entries.flatten() {
                                let sub_path = sub_entry.path();
                                if !sub_path.is_dir() {
                                    // OverlayFS 无法在根目录挂载文件，忽略文件
                                    continue;
                                }
                                let sub_name = sub_entry.file_name();

                                queue.push_back(ProcessingItem {
                                    module_source: sub_path,
                                    system_target: canonical_target.join(sub_name), // 下钻一层
                                    partition_label: partition_label.clone(),
                                });
                            }
                        }
                    } else {
                        // 不需要拆解，直接作为挂载点
                        overlay_groups
                            .entry(canonical_target)
                            .or_default()
                            .push(module_source);
                    }
                }
            }
        }
    }

    for (target_path, layers) in overlay_groups {
        let target_str = target_path.to_string_lossy().to_string();

        // 最终安全检查：不要挂载在非目录上
        if !target_path.is_dir() {
            continue;
        }

        let partition_name = target_path
            .iter()
            .nth(1)
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

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

    Ok(plan)
}
