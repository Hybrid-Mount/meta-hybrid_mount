// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{collections::HashSet, path::Path};

use anyhow::Result;

use crate::{
    conf::config,
    core::{
        backend::MountDriver,
        ops::planner::{MountPlan, OverlayOperation},
    },
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

pub fn execute<P, D>(
    plan: &MountPlan,
    config: &config::Config,
    tempdir: P,
    driver: &D,
) -> Result<ExecutionResult>
where
    P: AsRef<Path>,
    D: MountDriver + ?Sized,
{
    let mut final_magic_ids: HashSet<String> = plan.magic_module_ids.iter().cloned().collect();
    let mut final_overlay_ids: HashSet<String> = HashSet::new();

    if driver.is_supported()? {
        for op in &plan.overlay_ops {
            match driver.mount_overlay(op, config) {
                Ok(ids) => {
                    final_overlay_ids.extend(ids);
                }
                Err(_) => {
                    let involved_modules: Vec<String> = op
                        .lowerdirs
                        .iter()
                        .filter_map(|p| utils::extract_module_id(p))
                        .collect();
                    final_magic_ids.extend(involved_modules);
                }
            }
        }
        final_overlay_ids.retain(|id| !final_magic_ids.contains(id));
    } else {
        final_magic_ids.extend(plan.overlay_module_ids.clone());
    }

    let mut magic_queue: Vec<String> = final_magic_ids.iter().cloned().collect();
    magic_queue.sort();

    if !magic_queue.is_empty() {
        let magic_need_ids: HashSet<String> = magic_queue.into_iter().collect();
        if let Ok(mounted_ids) = driver.mount_magic(&magic_need_ids, config, tempdir.as_ref()) {
            final_magic_ids.retain(|id| mounted_ids.contains(id));
        } else {
            final_magic_ids.clear();
        }
    }

    let _ = umount_dir(tempdir.as_ref());

    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        if !config.disable_umount {
            let _ = umount_mgr::send_umountable(tempdir.as_ref());
            let _ = umount_mgr::commit();
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

pub struct NativeMount;

impl MountDriver for NativeMount {
    fn is_supported(&self) -> Result<bool> {
        overlayfs::utils::is_overlay_supported()
    }

    fn mount_overlay(&self, op: &OverlayOperation, config: &config::Config) -> Result<Vec<String>> {
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

        let mut mount_source = config.mountsource.clone();

        if defs::IGNORE_UNOUNT_PARTITIONS
            .iter()
            .any(|s| s.trim() == op.target.trim())
        {
            mount_source = "overlay".to_string();
        }

        overlayfs::overlayfs::mount_overlay(
            &op.target,
            &lowerdir_strings,
            work_opt,
            upper_opt,
            &mount_source,
        )?;

        #[cfg(any(target_os = "linux", target_os = "android"))]
        if !config.disable_umount {
            let _ = umount_mgr::send_umountable(&op.target);
        }

        Ok(involved_modules)
    }

    fn mount_magic(
        &self,
        ids: &HashSet<String>,
        config: &config::Config,
        tempdir: &Path,
    ) -> Result<Vec<String>> {
        let magic_ws_path = tempdir.join("magic_workspace");

        if matches!(config.overlay_mode, config::OverlayMode::Erofs) {
            if magic_ws_path.exists() {
                crate::sys::mount::mount_tmpfs(&magic_ws_path, "magic_ws")?;
            }
        } else if !magic_ws_path.exists() {
            std::fs::create_dir_all(&magic_ws_path)?;
        }

        magic_mount::magic_mount(
            &magic_ws_path,
            tempdir,
            &config.mountsource,
            &config.partitions,
            ids.clone(),
            !config.disable_umount,
        )?;

        Ok(ids.iter().cloned().collect())
    }
}
