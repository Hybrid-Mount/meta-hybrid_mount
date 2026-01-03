// Copyright 2025 Meta-Hybrid Mount Authors
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use anyhow::Result;
use rustix::mount::UnmountFlags;
use walkdir::WalkDir;

use crate::{
    conf::config,
    core::planner::MountPlan,
    defs,
    mount::{magic, overlay},
    utils,
};

pub struct ExecutionResult {
    pub overlay_module_ids: Vec<String>,
    pub magic_module_ids: Vec<String>,
}

pub enum DiagnosticLevel {
    #[allow(dead_code)]
    Info,
    Warning,
    Critical,
}

pub struct DiagnosticIssue {
    pub level: DiagnosticLevel,
    pub context: String,
    pub message: String,
}

fn extract_module_root(partition_path: &Path) -> Option<PathBuf> {
    partition_path.parent().map(|p| p.to_path_buf())
}

struct OverlayResult {
    magic_roots: Vec<PathBuf>,
    fallback_ids: Vec<String>,
    success_records: Vec<(PathBuf, String)>,
}

pub fn diagnose_plan(plan: &MountPlan) -> Vec<DiagnosticIssue> {
    let mut issues = Vec::new();

    for op in &plan.overlay_ops {
        let target = Path::new(&op.target);

        if !target.exists() {
            issues.push(DiagnosticIssue {
                level: DiagnosticLevel::Critical,
                context: op.partition_name.clone(),
                message: format!("Target mount point does not exist: {}", op.target),
            });
        }
    }

    let all_layers: Vec<(String, &PathBuf)> = plan
        .overlay_ops
        .iter()
        .flat_map(|op| {
            op.lowerdirs.iter().map(move |path| {
                let mod_id = utils::extract_module_id(path).unwrap_or_else(|| "unknown".into());

                (mod_id, path)
            })
        })
        .collect();

    for (mod_id, layer_path) in all_layers {
        if !layer_path.exists() {
            continue;
        }

        for entry in WalkDir::new(layer_path).into_iter().flatten() {
            if entry.path_is_symlink()
                && let Ok(target) = std::fs::read_link(entry.path())
                && target.is_absolute()
                && !target.exists()
            {
                issues.push(DiagnosticIssue {
                    level: DiagnosticLevel::Warning,
                    context: mod_id.clone(),
                    message: format!(
                        "Dead absolute symlink: {} -> {}",
                        entry.path().display(),
                        target.display()
                    ),
                });
            }
        }
    }

    issues
}

pub fn execute(plan: &MountPlan, config: &config::Config) -> Result<ExecutionResult> {
    let mut magic_queue = plan.magic_module_paths.clone();

    let mut global_success_map: HashMap<PathBuf, HashSet<String>> = HashMap::new();

    let mut final_overlay_ids = HashSet::new();

    plan.overlay_module_ids.iter().for_each(|id| {
        final_overlay_ids.insert(id.clone());
    });

    log::info!(">> Phase 1: OverlayFS Execution...");

    // Changed from par_iter() to iter() to ensure thread safety when modifying CWD
    let overlay_results: Vec<OverlayResult> = plan
        .overlay_ops
        .iter()
        .map(|op| {
            let lowerdir_strings: Vec<String> = op
                .lowerdirs
                .iter()
                .map(|p: &PathBuf| p.display().to_string())
                .collect();

            let rw_root = Path::new(defs::SYSTEM_RW_DIR);

            let part_rw = rw_root.join(&op.partition_name);

            let upper = part_rw.join("upperdir");

            let work = part_rw.join("workdir");

            let (upper_opt, work_opt) = if upper.exists() && work.exists() {
                (Some(upper), Some(work))
            } else {
                (None, None)
            };

            log::info!(
                "Mounting {} [OVERLAY] (Layers: {})",
                op.target,
                lowerdir_strings.len()
            );

            if let Err(e) = overlay::mount_overlay(
                &op.target,
                &lowerdir_strings,
                work_opt,
                upper_opt,
                config.disable_umount,
            ) {
                log::warn!(
                    "OverlayFS failed for {}: {}. Triggering fallback.",
                    op.target,
                    e
                );

                let mut local_magic = Vec::new();

                let mut local_fallback_ids = Vec::new();

                for layer_path in &op.lowerdirs {
                    if let Some(root) = extract_module_root(layer_path) {
                        local_magic.push(root.clone());

                        if let Some(id) = utils::extract_module_id(layer_path) {
                            local_fallback_ids.push(id);
                        }
                    }
                }

                return OverlayResult {
                    magic_roots: local_magic,
                    fallback_ids: local_fallback_ids,
                    success_records: Vec::new(),
                };
            }

            let mut successes = Vec::new();

            for layer_path in &op.lowerdirs {
                if let Some(root) = extract_module_root(layer_path) {
                    successes.push((root, op.partition_name.clone()));
                }
            }

            OverlayResult {
                magic_roots: Vec::new(),
                fallback_ids: Vec::new(),
                success_records: successes,
            }
        })
        .collect();

    for res in overlay_results {
        magic_queue.extend(res.magic_roots);

        for id in res.fallback_ids {
            final_overlay_ids.remove(&id);
        }

        for (root, partition) in res.success_records {
            global_success_map
                .entry(root)
                .or_default()
                .insert(partition);
        }
    }

    magic_queue.sort();

    magic_queue.dedup();

    let mut final_magic_ids = Vec::new();

    if !magic_queue.is_empty() {
        let tempdir = utils::select_temp_dir()?;
        let _ = crate::try_umount::TMPFS.set(tempdir.to_string_lossy().to_string());

        for path in &magic_queue {
            if let Some(name) = path.file_name() {
                final_magic_ids.push(name.to_string_lossy().to_string());
            }
        }

        log::info!(
            ">> Phase 2: Magic Mount (Fallback) using {}",
            tempdir.display()
        );

        if !tempdir.exists() {
            std::fs::create_dir_all(&tempdir)?;
        }

        utils::mount_tmpfs(&tempdir, "tmpfs")?;

        if let Err(e) = magic::mount_partitions(
            &tempdir,
            &magic_queue,
            &config.mountsource,
            &config.partitions,
            global_success_map,
            config.disable_umount,
        ) {
            log::error!("Magic Mount critical failure: {:#}", e);

            final_magic_ids.clear();
        }

        let _ = rustix::mount::unmount(&tempdir, UnmountFlags::DETACH);
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    if !config.disable_umount
        && let Err(e) = crate::try_umount::commit()
    {
        log::warn!("Final try_umount commit failed: {}", e);
    }

    let mut result_overlay = final_overlay_ids.into_iter().collect::<Vec<_>>();

    let mut result_magic = final_magic_ids;

    result_overlay.sort();

    result_magic.sort();

    result_magic.dedup();

    Ok(ExecutionResult {
        overlay_module_ids: result_overlay,
        magic_module_ids: result_magic,
    })
}
