// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{collections::HashSet, path::Path};

use anyhow::{Result, bail};

use crate::{
    conf::config,
    core::{
        ops::planner::{MountPlan, OverlayOperation},
        recovery::{FailureStage, ModuleStageFailure},
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

pub struct Executer;

impl Executer {
    fn is_symlink_loop_mount_error(err: &anyhow::Error) -> bool {
        let mut cursor = Some(err.as_ref() as &(dyn std::error::Error + 'static));
        while let Some(current) = cursor {
            let msg = current.to_string();
            if msg.contains("Too many symbolic links") || msg.contains("os error 40") {
                return true;
            }
            cursor = current.source();
        }
        false
    }

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

    fn collect_overlay_modules_for_magic_fallback(plan: &MountPlan) -> HashSet<String> {
        let mut fallback_ids = HashSet::new();
        for op in &plan.overlay_ops {
            fallback_ids.extend(Self::collect_involved_modules(op));
        }
        fallback_ids
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
                        if config.enable_overlay_fallback
                            && Self::is_symlink_loop_mount_error(&err)
                            && !involved_modules.is_empty()
                        {
                            log::warn!(
                                "[executor] overlay op hit symlink-loop mount error on {}; fallback to magic mount for modules: {}",
                                op.target,
                                involved_modules.join(", ")
                            );
                            final_magic_ids.extend(involved_modules);
                            continue;
                        }
                        return Err(ModuleStageFailure::new(
                            FailureStage::Execute,
                            involved_modules,
                            anyhow::anyhow!("Overlay mount failed for {}: {:#}", op.target, err),
                        )
                        .into());
                    }
                }
            }
        } else {
            if !plan.overlay_ops.is_empty() {
                if config.enable_overlay_fallback {
                    let fallback_ids = Self::collect_overlay_modules_for_magic_fallback(plan);
                    if fallback_ids.is_empty() {
                        bail!(
                            "[executor] overlayfs unsupported and no modules could be inferred for magic fallback"
                        );
                    }
                    log::warn!(
                        "[executor] overlayfs unsupported, fallback enabled; switching {} modules to magic mount",
                        fallback_ids.len()
                    );
                    final_magic_ids.extend(fallback_ids);
                } else {
                    bail!("[executor] overlayfs unsupported and overlay operations are pending");
                }
            }
            log::info!("[executor] overlayfs unsupported, no overlay operations to apply");
        }

        let mut magic_need_list: Vec<String> = final_magic_ids.iter().cloned().collect();
        magic_need_list.sort();

        if !magic_need_list.is_empty() {
            let magic_need_ids: HashSet<String> = magic_need_list.iter().cloned().collect();
            log::info!(
                "[executor] applying magic mount for modules: {}",
                magic_need_list.join(", ")
            );
            let mounted_ids = Self::mount_magic(&magic_need_ids, config, tempdir.as_ref())
                .map_err(|err| {
                    ModuleStageFailure::new(
                        FailureStage::Execute,
                        magic_need_list.clone(),
                        anyhow::anyhow!(
                            "Failed to mount Magic Mount modules [{}]: {:#}",
                            magic_need_list.join(", "),
                            err
                        ),
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

        if defs::IGNORE_UNMOUNT_PARTITIONS
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

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, path::PathBuf};

    use anyhow::anyhow;

    use super::Executer;
    use crate::core::ops::planner::{MountPlan, OverlayOperation};

    #[test]
    fn collect_overlay_modules_for_magic_fallback_deduplicates_modules() {
        let mut plan = MountPlan::default();
        plan.overlay_ops.push(OverlayOperation {
            partition_name: "system".to_string(),
            target: "/system/bin".to_string(),
            lowerdirs: vec![PathBuf::from("/modA/system"), PathBuf::from("/modB/system")],
        });
        plan.overlay_ops.push(OverlayOperation {
            partition_name: "vendor".to_string(),
            target: "/vendor/lib".to_string(),
            lowerdirs: vec![PathBuf::from("/modA/vendor"), PathBuf::from("/modC/vendor")],
        });

        let result = Executer::collect_overlay_modules_for_magic_fallback(&plan);
        let expected = HashSet::from(["modA".to_string(), "modB".to_string(), "modC".to_string()]);
        assert_eq!(result, expected);
    }

    #[test]
    fn symlink_loop_detection_matches_expected_messages() {
        let err = anyhow!(
            "Failed to fsconfig create new fs: Too many symbolic links encountered (os error 40)"
        );
        assert!(Executer::is_symlink_loop_mount_error(&err));

        let other = anyhow!("permission denied");
        assert!(!Executer::is_symlink_loop_mount_error(&other));
    }
}
