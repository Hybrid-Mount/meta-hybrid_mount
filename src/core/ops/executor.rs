// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use anyhow::Result;

use crate::{
    conf::config,
    core::ops::planner::MountPlan,
    defs,
    mount::{
        magic_mount,
        overlayfs::{self, utils::umount_dir},
        umount_mgr,
    },
    utils,
};

pub struct ExecutionResult {
    pub overlay_module_ids: Vec<String>,
    pub magic_module_ids: Vec<String>,
}

pub fn execute(plan: &MountPlan, config: &config::Config) -> Result<ExecutionResult> {
    let mut final_magic_ids: HashSet<String> = plan.magic_module_ids.iter().cloned().collect();
    let mut final_overlay_ids: HashSet<String> = HashSet::new();

    log::info!(">> Phase 1: OverlayFS Execution...");

    for op in &plan.overlay_ops {
        let involved_modules: Vec<String> = op
            .lowerdirs
            .iter()
            .filter_map(|p| utils::extract_module_id(p))
            .collect();

        let lowerdir_strings: Vec<String> = op
            .lowerdirs
            .iter()
            .map(|p| p.display().to_string())
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

        match overlayfs::overlayfs::mount_overlay(
            &op.target,
            &lowerdir_strings,
            work_opt,
            upper_opt,
            &config.mountsource,
        ) {
            Ok(_) => {
                for id in involved_modules {
                    final_overlay_ids.insert(id);
                }

                #[cfg(any(target_os = "linux", target_os = "android"))]
                if !config.disable_umount
                    && let Err(e) = umount_mgr::send_umountable(&op.target)
                {
                    log::warn!(
                        "Failed to schedule unmount for {}(kernel): {}",
                        op.target,
                        e
                    );
                }
            }
            Err(e) => {
                log::warn!(
                    "OverlayFS failed for {}: {}. Fallback to Magic Mount.",
                    op.target,
                    e
                );
                for id in involved_modules {
                    final_magic_ids.insert(id);
                }
            }
        }
    }

    final_overlay_ids.retain(|id| !final_magic_ids.contains(id));

    let mut magic_queue: Vec<String> = final_magic_ids.iter().cloned().collect();
    magic_queue.sort();

    if !magic_queue.is_empty() {
        let tempdir = PathBuf::from(&config.hybrid_mnt_dir).join("magic_workspace");
        let _ = umount_mgr::TMPFS.set(tempdir.to_string_lossy().to_string());

        log::info!(
            ">> Phase 2: Magic Mount (Fallback/Native) using {}",
            tempdir.display()
        );

        if matches!(config.overlay_mode, config::OverlayMode::Erofs) {
            if tempdir.exists() {
                crate::sys::mount::mount_tmpfs(&tempdir, "magic_ws")?;
                #[cfg(any(target_os = "linux", target_os = "android"))]
                if let Err(e) = umount_mgr::send_umountable(&tempdir) {
                    log::warn!("Failed to schedule unmount for magic_ws: {}", e);
                }
            } else {
                log::error!("Magic Mount anchor missing in EROFS image!");
            }
        } else if !tempdir.exists() {
            std::fs::create_dir_all(&tempdir)?;
        }

        let module_dir = Path::new(&config.hybrid_mnt_dir);
        let magic_need_ids: HashSet<String> = magic_queue.iter().cloned().collect();

        if let Err(e) = magic_mount::magic_mount(
            &tempdir,
            module_dir,
            &config.mountsource,
            &config.partitions,
            magic_need_ids,
            !config.disable_umount,
        ) {
            log::error!("Magic Mount critical failure: {:#}", e);
            final_magic_ids.clear();
        }
    }

    if let Err(e) = umount_dir(&config.hybrid_mnt_dir) {
        log::warn!(
            "Failed to schedule unmount for {}: {}",
            config.hybrid_mnt_dir,
            e
        );
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        if !config.disable_umount {
            let _ = umount_mgr::send_umountable(&config.hybrid_mnt_dir);
            if let Err(e) = umount_mgr::commit() {
                log::warn!("Final try_umount commit failed: {}", e);
            }
        }
    }

    let mut result_overlay: Vec<String> = final_overlay_ids.into_iter().collect();
    let mut result_magic: Vec<String> = final_magic_ids.into_iter().collect();

    result_overlay.sort();
    result_magic.sort();

    Ok(ExecutionResult {
        overlay_module_ids: result_overlay,
        magic_module_ids: result_magic,
    })
}
