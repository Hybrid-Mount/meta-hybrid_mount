// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

mod custom_bind;
mod magic;
mod overlay;

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};

use crate::{
    conf::config,
    core::{
        failure::{FailureStage, ModuleStageFailure},
        inventory::Module,
        ops::plan::{MountPlan, OverlayOperation},
        runtime_state::MountStatistics,
    },
    sys::mount::MountRollback,
    utils,
};

pub struct ExecutionResult {
    pub overlay_module_ids: Vec<String>,
    pub overlay_partitions: Vec<String>,
    pub magic_module_ids: Vec<String>,
    pub magic_mount_targets: Vec<String>,
    pub custom_mount_targets: Vec<String>,
    pub mount_stats: MountStatistics,
    pub(crate) rollback_targets: Vec<PathBuf>,
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
        let mut rollback = MountRollback::default();
        match Self::execute_inner(plan, modules, config, tempdir.as_ref(), &mut rollback) {
            Ok(mut result) => {
                result.rollback_targets = rollback.into_targets();
                Ok(result)
            }
            Err(error) => Err(rollback.attach_rollback(error)),
        }
    }

    fn execute_inner(
        plan: &mut MountPlan,
        modules: &[Module],
        config: &config::Config,
        tempdir: &Path,
        rollback: &mut MountRollback,
    ) -> Result<ExecutionResult> {
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

        let overlay_apply_timer = crate::utils::StageTimer::start("executor", "overlay_apply");
        if should_probe_overlay(plan) {
            let overlay_probe_timer =
                crate::utils::StageTimer::start("executor", "overlay_support_probe");
            let overlay_supported = Self::is_supported()?;
            overlay_probe_timer.finish();
            if !overlay_supported {
                bail!("[executor] overlayfs unsupported and overlay operations are pending");
            }

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
                    Ok((ids, overlay_mount_targets)) => {
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
                        rollback.extend(overlay_mount_targets);
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
            crate::scoped_log!(
                info,
                "executor",
                "overlayfs probe skipped: pending_overlay_ops=0"
            );
        }
        overlay_apply_timer.finish();

        let magic_need_list: Vec<String> = final_magic_ids.iter().cloned().collect();
        let mut persisted_magic_mount_targets = Vec::new();

        let magic_timer = crate::utils::StageTimer::start("executor", "magic_apply");
        if !magic_need_list.is_empty() {
            crate::scoped_log!(
                debug,
                "executor",
                "magic apply: modules={}",
                magic_need_list.join(", ")
            );
            let (mounted_ids, magic_stats, magic_mount_targets) =
                magic::mount_magic(modules, &magic_need_list, config, tempdir).map_err(|err| {
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
            persisted_magic_mount_targets = magic_mount_targets
                .iter()
                .map(|path| path.display().to_string())
                .collect();
            persisted_magic_mount_targets.sort();
            persisted_magic_mount_targets.dedup();
            rollback.extend(magic_mount_targets);
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
        let (custom_mount_paths, custom_stats) = custom_bind::mount_custom_binds(config)
            .context("Failed to apply custom bind mounts")?;
        rollback.extend(custom_mount_paths.iter().cloned());
        mount_stats.merge(&custom_stats);
        custom_bind_timer.finish();

        let result_overlay: Vec<String> = final_overlay_ids.into_iter().collect();
        let result_magic: Vec<String> = final_magic_ids.into_iter().collect();
        let custom_mount_targets = custom_mount_paths
            .into_iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>();
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
            magic_mount_targets: persisted_magic_mount_targets,
            custom_mount_targets,
            mount_stats,
            rollback_targets: Vec::new(),
        })
    }

    fn is_supported() -> Result<bool> {
        crate::mount::overlayfs::utils::is_overlay_supported()
    }
}

fn should_probe_overlay(plan: &MountPlan) -> bool {
    !plan.overlay_ops.is_empty()
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn overlay_probe_is_only_needed_for_overlay_operations() {
        let magic_only = MountPlan {
            magic_module_ids: vec!["magic".to_string()],
            ..Default::default()
        };
        assert!(!should_probe_overlay(&magic_only));

        let with_overlay = MountPlan {
            overlay_ops: vec![OverlayOperation {
                partition_name: "system".to_string(),
                target: "/system".to_string(),
                lowerdirs: vec![PathBuf::from("/mnt/hm_test/module/system")],
            }],
            ..Default::default()
        };
        assert!(should_probe_overlay(&with_overlay));
    }
}
