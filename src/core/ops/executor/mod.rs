// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

mod fallback;
mod magic;
mod overlay;

use std::{collections::BTreeSet, path::Path};

use anyhow::{Result, bail};

#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::mount::umount_mgr;
use crate::{
    conf::config,
    core::{
        inventory::Module,
        ops::planner::MountPlan,
        recovery::{FailureStage, ModuleStageFailure},
        runtime_state::MountStatistics,
    },
    mount::hymofs,
};

pub struct ExecutionResult {
    pub overlay_module_ids: Vec<String>,
    pub magic_module_ids: Vec<String>,
    pub hymofs_module_ids: Vec<String>,
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
        crate::scoped_log!(
            info,
            "executor",
            "start: overlay_ops={}, preselected_magic_modules={}, preselected_hymofs_modules={}",
            plan.overlay_ops.len(),
            plan.magic_module_ids.len(),
            plan.hymofs_module_ids.len()
        );
        let mut final_magic_ids: BTreeSet<String> = plan.magic_module_ids.iter().cloned().collect();
        let mut final_overlay_ids: BTreeSet<String> = BTreeSet::new();
        let planned_hymofs_ids = plan.hymofs_module_ids.clone();
        let mut mount_stats = MountStatistics::default();

        let hymofs_available = if config.hymofs.enabled {
            hymofs::reset_runtime(config).map_err(|err| {
                ModuleStageFailure::new(
                    FailureStage::Execute,
                    planned_hymofs_ids.clone(),
                    anyhow::anyhow!("Failed to reset HymoFS runtime: {:#}", err),
                )
            })?
        } else {
            crate::scoped_log!(
                debug,
                "executor",
                "hymofs disabled: skip_runtime_reset=true"
            );
            false
        };
        if !hymofs_available && !planned_hymofs_ids.is_empty() {
            return Err(ModuleStageFailure::new(
                FailureStage::Execute,
                planned_hymofs_ids.clone(),
                anyhow::anyhow!("HymoFS became unavailable before execution"),
            )
            .into());
        }

        if Self::is_supported()? {
            crate::scoped_log!(info, "executor", "overlayfs: supported=true");
            for op in &plan.overlay_ops {
                crate::scoped_log!(
                    info,
                    "executor",
                    "overlay apply: partition={}, target={}, layers={}",
                    op.partition_name,
                    op.target,
                    op.lowerdirs.len()
                );
                match overlay::mount_overlay(op, config) {
                    Ok(ids) => {
                        crate::scoped_log!(
                            info,
                            "executor",
                            "overlay success: target={}, modules={}",
                            op.target,
                            ids.len()
                        );
                        final_overlay_ids.extend(ids);
                        mount_stats.record_overlay_mount();
                    }
                    Err(err) => {
                        let involved_modules = fallback::collect_involved_modules(op);
                        let is_symlink_loop = fallback::is_symlink_loop_mount_error(&err);
                        if is_symlink_loop {
                            if !fallback::overlay_fallback_allowed(config) {
                                crate::scoped_log!(
                                    error,
                                    "executor",
                                    "overlay fallback denied: target={}, reason=symlink_loop, enable_overlay_fallback=false",
                                    op.target
                                );
                            } else if involved_modules.is_empty() {
                                crate::scoped_log!(
                                    error,
                                    "executor",
                                    "overlay fallback denied: target={}, reason=symlink_loop_no_modules",
                                    op.target
                                );
                            } else {
                                crate::scoped_log!(
                                    warn,
                                    "executor",
                                    "overlay fallback: target={}, reason=symlink_loop, modules={}",
                                    op.target,
                                    involved_modules.join(", ")
                                );
                                mount_stats.record_failed();
                                final_magic_ids.extend(involved_modules);
                                continue;
                            }
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
                if fallback::overlay_fallback_allowed(config) {
                    let fallback_ids = fallback::collect_overlay_modules_for_magic_fallback(plan);
                    if fallback_ids.is_empty() {
                        bail!(
                            "[executor] overlayfs unsupported and no modules could be inferred for magic fallback"
                        );
                    }
                    crate::scoped_log!(
                        warn,
                        "executor",
                        "overlayfs fallback: supported=false, switched_modules={}",
                        fallback_ids.len()
                    );
                    final_magic_ids.extend(fallback_ids);
                } else {
                    bail!("[executor] overlayfs unsupported and overlay operations are pending");
                }
            }
            crate::scoped_log!(
                info,
                "executor",
                "overlayfs: supported=false, pending_overlay_ops=0"
            );
        }

        plan.hymofs_add_rules.clear();
        plan.hymofs_merge_rules.clear();
        plan.hymofs_hide_rules.clear();
        let final_hymofs_ids = plan.hymofs_module_ids.clone();

        let magic_need_list: Vec<String> = final_magic_ids.iter().cloned().collect();

        if !magic_need_list.is_empty() {
            crate::scoped_log!(
                info,
                "executor",
                "magic apply: modules={}",
                magic_need_list.join(", ")
            );
            let (mounted_ids, magic_stats) =
                magic::mount_magic(&magic_need_list, config, tempdir.as_ref()).map_err(|err| {
                    let failed_module_ids =
                        fallback::resolve_magic_failure_modules(&err, &magic_need_list);
                    ModuleStageFailure::new(
                        FailureStage::Execute,
                        failed_module_ids.clone(),
                        anyhow::anyhow!(
                            "Failed to mount Magic Mount modules [{}]: {:#}",
                            failed_module_ids.join(", "),
                            err
                        ),
                    )
                })?;
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

        if config.hymofs.enabled {
            if let Err(err) = hymofs::apply(plan, modules, config) {
                return Err(ModuleStageFailure::new(
                    FailureStage::Execute,
                    final_hymofs_ids.clone(),
                    anyhow::anyhow!("Failed to apply HymoFS late rules: {:#}", err),
                )
                .into());
            }
        } else {
            crate::scoped_log!(
                debug,
                "executor",
                "hymofs disabled: skip_runtime_apply=true"
            );
        }

        #[cfg(any(target_os = "linux", target_os = "android"))]
        if !config.disable_umount {
            let _ = umount_mgr::commit();
        }

        let result_overlay: Vec<String> = final_overlay_ids.into_iter().collect();
        let result_magic: Vec<String> = final_magic_ids.into_iter().collect();

        crate::scoped_log!(
            info,
            "executor",
            "complete: overlay_modules={}, magic_modules={}, hymofs_modules={}",
            result_overlay.len(),
            result_magic.len(),
            final_hymofs_ids.len()
        );

        Ok(ExecutionResult {
            overlay_module_ids: result_overlay,
            magic_module_ids: result_magic,
            hymofs_module_ids: final_hymofs_ids,
            mount_stats,
        })
    }

    fn is_supported() -> Result<bool> {
        crate::mount::overlayfs::utils::is_overlay_supported()
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, path::PathBuf};

    use anyhow::anyhow;

    use super::fallback;
    use crate::{
        conf::config::{Config, OverlayMode},
        core::ops::planner::{MountPlan, OverlayOperation},
        mount::magic_mount::MagicMountModuleFailure,
    };

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

        let result = fallback::collect_overlay_modules_for_magic_fallback(&plan);
        let expected = HashSet::from(["modA".to_string(), "modB".to_string(), "modC".to_string()]);
        assert_eq!(result, expected);
    }

    #[test]
    fn symlink_loop_detection_matches_expected_messages() {
        let err = anyhow!(
            "Failed to fsconfig create new fs: Too many symbolic links encountered (os error 40)"
        );
        assert!(fallback::is_symlink_loop_mount_error(&err));

        let other = anyhow!("permission denied");
        assert!(!fallback::is_symlink_loop_mount_error(&other));
    }

    #[test]
    fn resolve_magic_failure_modules_prefers_precise_modules() {
        let err: anyhow::Error = MagicMountModuleFailure::new(
            vec!["modB".to_string(), "modA".to_string()],
            anyhow!("tmpfs mount failed"),
        )
        .into();
        let fallback = vec!["modA".to_string(), "modB".to_string(), "modC".to_string()];

        let result = fallback::resolve_magic_failure_modules(&err, &fallback);
        assert_eq!(result, vec!["modB".to_string(), "modA".to_string()]);
    }

    #[test]
    fn resolve_magic_failure_modules_falls_back_when_untyped_error() {
        let err = anyhow!("unknown magic mount error");
        let fallback = vec!["modA".to_string(), "modC".to_string()];

        let result = fallback::resolve_magic_failure_modules(&err, &fallback);
        assert_eq!(result, fallback);
    }

    #[test]
    fn overlay_fallback_is_controlled_by_enable_overlay_fallback() {
        let mut cfg = Config {
            overlay_mode: OverlayMode::Tmpfs,
            ..Config::default()
        };
        cfg.enable_overlay_fallback = false;
        assert!(!fallback::overlay_fallback_allowed(&cfg));

        cfg.enable_overlay_fallback = true;
        assert!(fallback::overlay_fallback_allowed(&cfg));
    }
}
