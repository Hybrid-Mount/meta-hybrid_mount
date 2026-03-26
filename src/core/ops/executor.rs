// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{collections::HashSet, path::Path};

use anyhow::{Context, Result, bail};

use crate::{
    conf::config,
    core::ops::planner::{MountPlan, OverlayOperation},
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

pub struct Executer;

impl Executer {
    fn collect_involved_modules(op: &OverlayOperation) -> Vec<String> {
        let mut involved_modules: Vec<String> = op
            .lowerdirs
            .iter()
            .filter_map(|p| utils::extract_module_id(p))
            .collect();
        involved_modules.sort();
        involved_modules.dedup();
        involved_modules
    }

    pub fn execute<P>(
        plan: &MountPlan,
        config: &config::Config,
        tempdir: P,
    ) -> Result<ExecutionResult>
    where
        P: AsRef<Path>,
    {
        log::info!(
            "[executor] start: overlay_ops={}, preselected_magic_modules={}",
            plan.overlay_ops.len(),
            plan.magic_module_ids.len()
        );
        let mut final_magic_ids: HashSet<String> = plan.magic_module_ids.iter().cloned().collect();
        let mut final_overlay_ids: HashSet<String> = HashSet::new();

        if Self::is_supported()? {
            log::info!("[executor] overlayfs supported, applying overlay operations");
            for op in &plan.overlay_ops {
                log::info!(
                    "[executor] apply overlay op: partition={}, target={}, layers={}",
                    op.partition_name,
                    op.target,
                    op.lowerdirs.len()
                );
                match Self::mount_overlay(op, config) {
                    Ok(ids) => {
                        log::info!(
                            "[executor] overlay op success: target={}, modules={}",
                            op.target,
                            ids.len()
                        );
                        final_overlay_ids.extend(ids);
                    }
                    Err(err) => {
                        let involved_modules = Self::collect_involved_modules(op);
                        bail!(
                            "Overlay mount failed for {} (modules: {}): {:#}",
                            op.target,
                            if involved_modules.is_empty() {
                                "<unknown>".to_string()
                            } else {
                                involved_modules.join(", ")
                            },
                            err
                        );
                    }
                }
            }
        } else {
            if !plan.overlay_ops.is_empty() {
                bail!("[executor] overlayfs unsupported and overlay operations are pending");
            }
            log::info!("[executor] overlayfs unsupported, no overlay operations to apply");
        }

        let mut magic_queue: Vec<String> = final_magic_ids.iter().cloned().collect();
        magic_queue.sort();

        if !magic_queue.is_empty() {
            let magic_need_ids: HashSet<String> = magic_queue.into_iter().collect();
            let mut magic_need_list: Vec<String> = magic_need_ids.iter().cloned().collect();
            magic_need_list.sort();
            log::info!(
                "[executor] applying magic mount for modules: {}",
                magic_need_list.join(", ")
            );
            let mounted_ids = Self::mount_magic(&magic_need_ids, config, tempdir.as_ref())
                .with_context(|| {
                    format!(
                        "Failed to mount Magic Mount modules: {}",
                        magic_need_list.join(", ")
                    )
                })?;
            final_magic_ids.retain(|id| mounted_ids.contains(id));
            log::info!(
                "[executor] magic mount completed: mounted_modules={}",
                mounted_ids.len()
            );
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

        log::info!(
            "[executor] completed: overlay_modules={}, magic_modules={}",
            result_overlay.len(),
            result_magic.len()
        );

        Ok(ExecutionResult {
            overlay_module_ids: result_overlay,
            magic_module_ids: result_magic,
        })
    }

    fn is_supported() -> Result<bool> {
        overlayfs::utils::is_overlay_supported()
    }

    fn mount_overlay(op: &OverlayOperation, config: &config::Config) -> Result<Vec<String>> {
        let involved_modules = Self::collect_involved_modules(op);

        log::debug!(
            "[executor] mount_overlay preparing: target={}, partition={}, modules={}",
            op.target,
            op.partition_name,
            if involved_modules.is_empty() {
                "<unknown>".to_string()
            } else {
                involved_modules.join(",")
            }
        );

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

        log::debug!(
            "[executor] mount_overlay done: target={}, mount_source={}",
            op.target,
            mount_source
        );

        #[cfg(any(target_os = "linux", target_os = "android"))]
        if !config.disable_umount {
            let _ = umount_mgr::send_umountable(&op.target);
        }

        Ok(involved_modules)
    }

    fn mount_magic(
        ids: &HashSet<String>,
        config: &config::Config,
        tempdir: &Path,
    ) -> Result<Vec<String>> {
        let magic_ws_path = tempdir.join("magic_workspace");

        log::debug!(
            "[executor] mount_magic preparing workspace: {}",
            magic_ws_path.display()
        );

        if !magic_ws_path.exists() {
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

        log::debug!("[executor] mount_magic done: module_count={}", ids.len());

        Ok(ids.iter().cloned().collect())
    }
}
