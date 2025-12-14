use std::path::{Path, PathBuf};
use std::collections::{HashMap, HashSet};
use anyhow::Result;
use rayon::prelude::*;
use walkdir::WalkDir;
use rustix::mount::UnmountFlags;

use crate::{
    conf::config, 
    mount::{magic, overlay, hymofs::{HymoFs, HymoFsStatus}}, 
    utils,
    core::planner::MountPlan
};

pub struct ExecutionResult {
    pub overlay_module_ids: Vec<String>,
    pub hymo_module_ids: Vec<String>,
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

fn extract_id(path: &Path) -> Option<String> {
    path.parent()
        .and_then(|p| p.file_name())
        .map(|s| s.to_string_lossy().to_string())
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
    let all_layers: Vec<(String, &PathBuf)> = plan.overlay_ops.iter()
        .flat_map(|op| {
            op.lowerdirs.iter().map(move |path| {
                let mod_id = extract_id(path).unwrap_or_else(|| "unknown".into());
                (mod_id, path)
            })
        })
        .collect();
    for (mod_id, layer_path) in all_layers {
        if !layer_path.exists() { continue; }
        for entry in WalkDir::new(layer_path) {
            if let Ok(entry) = entry {
                if entry.path_is_symlink() {
                    if let Ok(target) = std::fs::read_link(entry.path()) {
                        if target.is_absolute() {
                            if !target.exists() {
                                issues.push(DiagnosticIssue {
                                    level: DiagnosticLevel::Warning,
                                    context: mod_id.clone(),
                                    message: format!("Dead absolute symlink: {} -> {}", 
                                        entry.path().display(), target.display()),
                                });
                            }
                        }
                    }
                }
            }
        }
    }
    issues
}

pub fn execute(plan: &MountPlan, config: &config::Config) -> Result<ExecutionResult> {
    let mut magic_queue = plan.magic_module_paths.clone();
    let mut global_success_map: HashMap<PathBuf, HashSet<String>> = HashMap::new();
    let mut final_overlay_ids = HashSet::new();
    let mut final_hymo_ids = HashSet::new();
    
    plan.overlay_module_ids.iter().for_each(|id| { final_overlay_ids.insert(id.clone()); });
    plan.hymo_module_ids.iter().for_each(|id| { final_hymo_ids.insert(id.clone()); });

    if !plan.hymo_ops.is_empty() {
        match HymoFs::check_status() {
            HymoFsStatus::Available => {
                log::info!(">> Phase 1: HymoFS Injection (Protocol v{})...", crate::defs::HYMO_PROTOCOL_VERSION);
                if let Err(e) = HymoFs::clear() {
                    log::warn!("Failed to reset HymoFS rules: {}", e);
                }
                for op in &plan.hymo_ops {
                    let part_name = op.target.file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    log::debug!("Injecting {} -> {}", op.source.display(), op.target.display());
                    match HymoFs::inject_directory(&op.target, &op.source) {
                        Ok(_) => {
                            if let Some(root) = extract_module_root(&op.source) {
                                global_success_map.entry(root).or_default().insert(part_name);
                            }
                        },
                        Err(e) => {
                            log::error!("HymoFS failed for {}: {}. Fallback to Magic Mount.", op.module_id, e);
                            if let Some(root) = extract_module_root(&op.source) {
                                magic_queue.push(root);
                            }
                            final_hymo_ids.remove(&op.module_id);
                        }
                    }
                }
            },
            status => {
                let reason = match status {
                    HymoFsStatus::NotPresent => "Kernel module not loaded",
                    HymoFsStatus::ProtocolMismatch => "Protocol version mismatch",
                    _ => "Unavailable",
                };
                log::warn!("!! HymoFS requested but unavailable: {}. Falling back to Magic Mount.", reason);
                for op in &plan.hymo_ops {
                    if let Some(root) = extract_module_root(&op.source) {
                        magic_queue.push(root);
                    }
                    final_hymo_ids.remove(&op.module_id);
                }
            }
        }
    }

    log::info!(">> Phase 2: OverlayFS Execution...");
    let overlay_results: Vec<OverlayResult> = plan.overlay_ops.par_iter()
        .map(|op| {
            let lowerdir_strings: Vec<String> = op.lowerdirs.iter()
                .map(|p: &PathBuf| p.display().to_string())
                .collect();
            log::info!("Mounting {} [OVERLAY] ({} layers)", op.target, lowerdir_strings.len());
            if let Err(e) = overlay::mount_overlay(&op.target, &lowerdir_strings, None, None, config.disable_umount) {
                log::warn!("OverlayFS failed for {}: {}. Triggering fallback.", op.target, e);
                let mut local_magic = Vec::new();
                let mut local_fallback_ids = Vec::new();
                for layer_path in &op.lowerdirs {
                    if let Some(root) = extract_module_root(layer_path) {
                        local_magic.push(root.clone());
                        if let Some(id) = extract_id(layer_path) {
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
            global_success_map.entry(root)
                .or_default()
                .insert(partition);
        }
    }

    magic_queue.sort();
    magic_queue.dedup();
    let mut final_magic_ids = Vec::new();

    if !magic_queue.is_empty() {
        let (tempdir, needs_mount) = if let Some(t) = &config.tempdir { 
            (t.clone(), false)
        } else { 
            (utils::select_temp_dir()?, true)
        };

        for path in &magic_queue {
            if let Some(name) = path.file_name() {
                final_magic_ids.push(name.to_string_lossy().to_string());
            }
        }
        
        log::info!(">> Phase 3: Magic Mount (Fallback) using {} (tmpfs_overlay={})", tempdir.display(), needs_mount);
        
        if needs_mount {
            if !tempdir.exists() {
                std::fs::create_dir_all(&tempdir)?;
            }
            utils::mount_tmpfs(&tempdir, "tmpfs")?;
        } else {
            utils::ensure_temp_dir(&tempdir)?;
        }

        if let Err(e) = magic::mount_partitions(
            &tempdir, 
            &magic_queue, 
            &config.mountsource, 
            &config.partitions, 
            global_success_map, 
            config.disable_umount
        ) {
            log::error!("Magic Mount critical failure: {:#}", e);
            final_magic_ids.clear();
        }

        if needs_mount {
            let _ = rustix::mount::unmount(&tempdir, UnmountFlags::DETACH);
        } else {
            utils::cleanup_temp_dir(&tempdir);
        }
    }

    let mut result_overlay = final_overlay_ids.into_iter().collect::<Vec<_>>();
    let mut result_hymo = final_hymo_ids.into_iter().collect::<Vec<_>>();
    let mut result_magic = final_magic_ids;
    
    result_overlay.sort();
    result_hymo.sort();
    result_magic.sort();
    result_magic.dedup();

    Ok(ExecutionResult {
        overlay_module_ids: result_overlay,
        hymo_module_ids: result_hymo,
        magic_module_ids: result_magic,
    })
}
