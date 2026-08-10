// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

mod custom_bind;
mod magic;
mod overlay;

use std::{collections::BTreeSet, path::Path};

use anyhow::{Context, Result, bail};

#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::mount::umount_mgr;
use crate::{
    conf::config,
    core::{
        failure::{FailureStage, ModuleStageFailure},
        inventory::Module,
        ops::plan::{MountPlan, OverlayOperation},
        runtime_state::MountStatistics,
    },
    utils,
};

pub struct ExecutionResult {
    pub overlay_module_ids: Vec<String>,
    pub overlay_partitions: Vec<String>,
    pub magic_module_ids: Vec<String>,
    pub custom_mount_targets: Vec<String>,
    pub mount_stats: MountStatistics,
}

pub struct Executor;

impl Executor {
    pub fn execute<P>(
        plan: &mut MountPlan,
        modules: &[Module],
        config: &config::Config,
        tempdir: P,
    ) -> Result<ExecutionResult>
    where
        P: AsRef<Path>,
    {
        let total_timer = crate::utils::StageTimer::start("executor", "execute_total");
        crate::scoped_log!(
            info,
            "executor",
            "start: overlay_ops={}, preselected_magic_modules={}",
            plan.overlay_ops.len(),
            plan.magic_module_ids.len()
        );
        let mut final_magic_ids: BTreeSet<String> = plan.magic_module_ids.iter().cloned().collect();
        let mut final_overlay_ids: BTreeSet<String> = BTreeSet::new();
        let mut final_overlay_partitions: BTreeSet<String> = BTreeSet::new();
        let mut mount_stats = MountStatistics::default();

        let overlay_probe_timer =
            crate::utils::StageTimer::start("executor", "overlay_support_probe");
        let overlay_supported = Self::is_supported()?;
        overlay_probe_timer.finish();
        let overlay_apply_timer = crate::utils::StageTimer::start("executor", "overlay_apply");
        if overlay_supported {
            crate::scoped_log!(info, "executor", "overlayfs: supported=true");
            for op in &plan.overlay_ops {
                crate::scoped_log!(
                    debug,
                    "executor",
                    "overlay apply: partition={}, target={}, layers={}",
                    op.partition_name,
                    op.target,
                    op.lowerdirs.len()
                );

                let overlay_result = overlay::mount_overlay(op, config);

                match overlay_result {
                    Ok(ids) => {
                        crate::scoped_log!(
                            debug,
                            "executor",
                            "overlay success: target={}, modules={}",
                            op.target,
                            ids.len()
                        );
                        final_overlay_partitions.insert(op.partition_name.clone());
                        final_overlay_ids.extend(ids);
                        mount_stats.record_overlay_mount();
                    }
                    Err(err) => {
                        let involved_modules = collect_involved_modules(op);
                        if is_symlink_loop_error(&err) {
                            crate::scoped_log!(
                                error,
                                "executor",
                                "overlay failed: target={}, reason=symlink_loop",
                                op.target
                            );
                        } else {
                            crate::scoped_log!(
                                error,
                                "executor",
                                "overlay failed: target={}, reason=non_symlink_loop",
                                op.target
                            );
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
                bail!("[executor] overlayfs unsupported and overlay operations are pending");
            }
            crate::scoped_log!(
                info,
                "executor",
                "overlayfs: supported=false, pending_overlay_ops=0"
            );
        }
        overlay_apply_timer.finish();

        let magic_need_list: Vec<String> = final_magic_ids.iter().cloned().collect();

        let magic_timer = crate::utils::StageTimer::start("executor", "magic_apply");
        if !magic_need_list.is_empty() {
            crate::scoped_log!(
                debug,
                "executor",
                "magic apply: modules={}",
                magic_need_list.join(", ")
            );
            let (mounted_ids, magic_stats) =
                magic::mount_magic(modules, &magic_need_list, config, tempdir.as_ref()).map_err(
                    |err| {
                        ModuleStageFailure::new(
                            FailureStage::Execute,
                            magic_need_list.clone(),
                            anyhow::anyhow!(
                                "Failed to mount Magic Mount modules [{}]: {:#}",
                                magic_need_list.join(", "),
                                err
                            ),
                        )
                    },
                )?;
            mount_stats.merge(&magic_stats);
            let mounted_ids: BTreeSet<String> = mounted_ids.into_iter().collect();
            final_magic_ids.retain(|id| mounted_ids.contains(id));
            crate::scoped_log!(
                info,
                "executor",
                "magic complete: mounted_modules={}",
                mounted_ids.len()
            );
        }
        magic_timer.finish();

        let custom_bind_timer = crate::utils::StageTimer::start("executor", "custom_bind_apply");
        let (custom_mount_targets, custom_stats) = custom_bind::mount_custom_binds(config)
            .context("Failed to apply custom bind mounts")?;
        mount_stats.merge(&custom_stats);
        custom_bind_timer.finish();

        #[cfg(any(target_os = "linux", target_os = "android"))]
        if !config.disable_umount {
            let timer = crate::utils::StageTimer::start("executor", "umount_commit");
            umount_mgr::commit().context("Failed to commit umountable mount list")?;
            timer.finish();
        }

        let result_overlay: Vec<String> = final_overlay_ids.into_iter().collect();
        let result_magic: Vec<String> = final_magic_ids.into_iter().collect();
        total_timer.finish();

        crate::scoped_log!(
            info,
            "executor",
            "complete: overlay_modules={}, magic_modules={}, custom_mounts={}",
            result_overlay.len(),
            result_magic.len(),
            custom_mount_targets.len()
        );

        Ok(ExecutionResult {
            overlay_module_ids: result_overlay,
            overlay_partitions: final_overlay_partitions.into_iter().collect(),
            magic_module_ids: result_magic,
            custom_mount_targets,
            mount_stats,
        })
    }

    fn is_supported() -> Result<bool> {
        crate::mount::overlayfs::utils::is_overlay_supported()
    }
}

fn is_symlink_loop_error(err: &anyhow::Error) -> bool {
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
