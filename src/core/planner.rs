// src/core/planner.rs
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

// 辅助结构：用于在队列中传递处理任务
struct ProcessingItem {
    module_source: PathBuf,    // 模块内的源路径 (e.g. /data/adb/modules/mod1/system/vendor)
    system_target: PathBuf,    // 系统上的目标路径 (e.g. /system/vendor)
    partition_label: String,   // 归属的分区名 (e.g. "vendor")
}

pub fn generate(
    config: &config::Config,
    modules: &[Module],
    storage_root: &Path,
) -> Result<MountPlan> {
    let mut plan = MountPlan::default();

    // 1. 收集所有 overlay 任务 (按最终的目标路径聚合)
    // Key: 最终的系统绝对路径 (e.g. "/vendor/bin")
    // Value: 模块内的源路径列表
    let mut overlay_groups: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
    
    let mut overlay_ids = HashSet::new();
    let mut magic_ids = HashSet::new();

    // 预处理敏感分区列表，方便查询
    let sensitive_partitions: HashSet<&str> = defs::SENSITIVE_PARTITIONS.iter().cloned().collect();

    for module in modules {
        let mut content_path = storage_root.join(&module.id);
        if !content_path.exists() {
            content_path = module.source_path.clone();
        }
        if !content_path.exists() { continue; }

        // 检查 Magic Mount 模式
        // 如果模块规则强制某些目录使用 Magic Mount，这里简化处理，假设混合模式下主要处理 Overlay
        // 实际实现中，应该先过滤掉 Magic Mount 的目录
        // 这里为了简化，我们先只处理 Overlay 逻辑，Magic Mount 逻辑保留在原处或需单独收集
        
        // 遍历模块根目录
        if let Ok(entries) = fs::read_dir(&content_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() { continue; }

                let dir_name = entry.file_name().to_string_lossy().to_string();
                
                // 检查是否在处理范围内
                if !defs::BUILTIN_PARTITIONS.contains(&dir_name.as_str()) 
                   && !config.partitions.contains(&dir_name) {
                    continue;
                }
                
                // 检查挂载模式
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

                // 使用队列进行路径解析和拆解
                let mut queue = VecDeque::new();
                queue.push_back(ProcessingItem {
                    module_source: path.clone(),
                    system_target: PathBuf::from("/").join(&dir_name),
                    partition_label: dir_name.clone(),
                });

                while let Some(item) = queue.pop_front() {
                    let ProcessingItem { module_source, system_target, partition_label } = item;

                    // 1. 检查系统目标是否存在
                    if !system_target.exists() {
                        // 如果目标不存在，无法挂载 Overlay (除非父目录支持，这里保守跳过)
                        continue;
                    }

                    // 2. 软链接解析 (核心逻辑：操作粒度优化)
                    // 如果 /system/vendor 是软链接，我们需要获取它指向的真实路径 /vendor
                    let resolved_target = match fs::read_link(&system_target) {
                        Ok(target) => {
                            if target.is_absolute() {
                                target
                            } else {
                                // 处理相对软链接
                                system_target.parent().unwrap_or(Path::new("/")).join(target)
                            }
                        },
                        Err(_) => system_target.clone(), // 不是软链接，保持原样
                    };
                    
                    // 规范化路径 (去除 .. 和 .)
                    let canonical_target = if resolved_target.exists() {
                         // 使用 canonicalize 获取最真实的物理路径
                         match resolved_target.canonicalize() {
                             Ok(p) => p,
                             Err(_) => resolved_target,
                         }
                    } else {
                        resolved_target
                    };

                    // 3. 检查是否需要拆解 (Controlled Depth)
                    // 条件：是敏感分区 (vendor, odm...) 或者 是 /system (为了防止遮盖 /system 下的软链接)
                    // 注意：如果 canonical_target 变成了 /vendor，而 /vendor 在敏感列表中，则会触发拆解
                    
                    let target_name = canonical_target.file_name()
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
                        overlay_groups.entry(canonical_target).or_default().push(module_source);
                    }
                }
            }
        }
    }

    // 2. 生成 MountPlan
    for (target_path, layers) in overlay_groups {
        let target_str = target_path.to_string_lossy().to_string();
        
        // 最终安全检查：不要挂载在非目录上
        if !target_path.is_dir() {
            continue;
        }

        // 推测 partition_name (仅用于显示或冲突检测)
        let partition_name = target_path.iter().nth(1)
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

fn has_files(path: &Path) -> bool {
    if let Ok(entries) = fs::read_dir(path)
        && entries.flatten().next().is_some()
    {
        return true;
    }

    false
}
