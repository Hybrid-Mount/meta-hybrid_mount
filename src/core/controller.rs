// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
#[cfg(any(target_os = "linux", target_os = "android"))]
use rustix::mount::{UnmountFlags, unmount as umount};

#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::sys::mount::is_mounted;
use crate::{
    conf::config::Config,
    core::{
        inventory::{self},
        ops::{
            executor::{self},
            plan::MountPlan,
            prepare,
        },
        runtime_finalization,
        storage::StorageHandle,
    },
    defs,
    sys::mount::{MountRollback, detach_mount},
};

pub struct Init;

pub struct StorageReady {
    pub handle: StorageHandle,
}

pub struct Planned {
    pub handle: StorageHandle,
    pub inventory: inventory::InventorySnapshot,
    pub plan: MountPlan,
}

pub struct Executed {
    pub handle: StorageHandle,
    pub result: executor::ExecutionResult,
    pub inventory_summary: inventory::InventorySummary,
}

pub struct MountController<S> {
    config: Config,
    state: S,
    tempdir: PathBuf,
}

impl MountController<Init> {
    pub fn new<P>(config: Config, tempdir: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        Ok(Self {
            config,
            state: Init,
            tempdir: tempdir.as_ref().to_path_buf(),
        })
    }

    pub fn init_storage(self, mnt_base: &Path) -> Result<MountController<StorageReady>> {
        let timer = crate::utils::StageTimer::start("controller", "storage_setup");
        crate::scoped_log!(
            info,
            "controller:init_storage",
            "start: mount_base={}",
            mnt_base.display()
        );
        #[cfg(feature = "control-plane")]
        let force_ext4 = matches!(
            self.config.overlay_mode,
            crate::conf::config::OverlayMode::Ext4
        );
        #[cfg(not(feature = "control-plane"))]
        let force_ext4 = true;
        let handle = crate::core::storage::setup(
            mnt_base,
            &self.config.moduledir,
            force_ext4,
            &self.config.mountsource,
            self.config.disable_umount,
        )?;
        timer.finish();

        crate::scoped_log!(
            info,
            "controller:init_storage",
            "complete: mode={}, mount_point={}",
            handle.mode().as_str(),
            handle.mount_point().display()
        );

        Ok(MountController {
            config: self.config,
            state: StorageReady { handle },
            tempdir: self.tempdir,
        })
    }
}

impl MountController<StorageReady> {
    pub fn scan_and_prepare_plan(self) -> Result<MountController<Planned>> {
        crate::scoped_log!(
            info,
            "controller:scan_and_prepare_plan",
            "scan start: moduledir={}",
            self.config.moduledir.display()
        );
        let inventory = inventory::scan_snapshot(&self.config)?;
        let modules = &inventory.modules;

        crate::scoped_log!(
            info,
            "controller:scan_and_prepare_plan",
            "scan complete: modules={}",
            modules.len()
        );

        crate::scoped_log!(info, "controller:scan_and_prepare_plan", "prepare start");
        let plan = prepare::prepare_mount_plan(modules, self.state.handle.mount_point())?;

        crate::scoped_log!(
            info,
            "controller:scan_and_prepare_plan",
            "prepare complete: overlay_ops={}, overlay_modules={}, magic_modules={}, copied_entries={}, copied_bytes={}",
            plan.overlay_ops.len(),
            plan.overlay_module_ids.len(),
            plan.magic_module_ids.len(),
            plan.prepare_metrics.copied_entries,
            plan.prepare_metrics.copied_bytes,
        );

        Ok(MountController {
            config: self.config,
            state: Planned {
                handle: self.state.handle,
                inventory,
                plan,
            },
            tempdir: self.tempdir,
        })
    }
}

impl MountController<Planned> {
    pub fn execute(mut self) -> Result<MountController<Executed>> {
        crate::scoped_log!(info, "controller:execute", "start");
        let result = executor::Executor::execute(
            &mut self.state.plan,
            &self.state.inventory.modules,
            &self.config,
            self.tempdir.clone(),
        )?;

        crate::scoped_log!(
            info,
            "controller:execute",
            "complete: overlay_mounted={}, magic_mounted={}",
            result.overlay_module_ids.len(),
            result.magic_module_ids.len()
        );

        Ok(MountController {
            config: self.config,
            state: Executed {
                handle: self.state.handle,
                result,
                inventory_summary: self.state.inventory.summary,
            },
            tempdir: self.tempdir,
        })
    }
}

impl MountController<Executed> {
    pub fn finalize(mut self) -> Result<()> {
        let timer = crate::utils::StageTimer::start("controller", "finalize_total");
        crate::scoped_log!(info, "controller:finalize", "start");
        let rollback_targets = std::mem::take(&mut self.state.result.rollback_targets);
        let mut recovery_state = None;
        let finalize_result = finalize_transaction_with(
            rollback_targets,
            || {
                let runtime_state = runtime_finalization::build_state(
                    &self.config,
                    self.state.handle.mode(),
                    self.state.handle.mount_point(),
                    &self.state.result,
                    &self.state.inventory_summary,
                )?;
                recovery_state = Some(runtime_state);

                let cleanup_timer = crate::utils::StageTimer::start("controller", "cleanup");
                clean_up(
                    &self.tempdir,
                    self.state.handle.mode(),
                    self.config.disable_umount,
                )?;
                cleanup_timer.finish();
                runtime_finalization::save_state(
                    recovery_state
                        .as_ref()
                        .expect("runtime state must exist after a successful build"),
                )?;
                Ok(())
            },
            detach_mount,
        );

        if let Err(failure) = finalize_result {
            let recovery_result = if failure.rollback_complete {
                clear_failed_runtime_state()
                    .context("failed to clear runtime state after a complete rollback")
            } else if let Some(state) = recovery_state.as_ref() {
                state
                    .save()
                    .context("failed to persist recovery state after an incomplete rollback")
            } else {
                crate::scoped_log!(
                    warn,
                    "controller:rollback",
                    "rollback incomplete before runtime state could be built; preserving existing state"
                );
                Ok(())
            };

            return Err(match recovery_result {
                Ok(()) => failure.error,
                Err(recovery_error) => failure.error.context(format!("{recovery_error:#}")),
            });
        }

        #[cfg(any(target_os = "linux", target_os = "android"))]
        if !self.config.disable_umount {
            // This must remain the final fallible operation. KernelSU's locked
            // userspace API cannot safely remove one committed try-umount
            // entry, so rolling VFS mounts back after a partial commit would
            // leave entries capable of resolving to an underlying mount.
            crate::mount::umount_mgr::commit()
                .context("Failed to commit KernelSU try-umount entries")?;
        }

        timer.finish();

        crate::scoped_log!(info, "controller:finalize", "complete");

        Ok(())
    }
}

#[derive(Debug)]
struct FinalizeTransactionFailure {
    error: anyhow::Error,
    rollback_complete: bool,
}

fn finalize_transaction_with<F, D>(
    rollback_targets: Vec<PathBuf>,
    finalize: F,
    mut detach: D,
) -> std::result::Result<(), FinalizeTransactionFailure>
where
    F: FnOnce() -> Result<()>,
    D: FnMut(&Path) -> Result<()>,
{
    let Err(error) = finalize() else {
        return Ok(());
    };

    let mut rollback = MountRollback::default();
    rollback.extend(rollback_targets);
    match rollback.rollback_with(&mut detach) {
        Ok(()) => Err(FinalizeTransactionFailure {
            error,
            rollback_complete: true,
        }),
        Err(rollback_error) => Err(FinalizeTransactionFailure {
            error: error.context(format!(
                "additionally failed to roll back finalized mount targets: {rollback_error:#}"
            )),
            rollback_complete: false,
        }),
    }
}

fn clear_failed_runtime_state() -> Result<()> {
    match fs::remove_file(defs::STATE_FILE) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error)
            .with_context(|| format!("failed to remove runtime state {}", defs::STATE_FILE)),
    }
}

fn clean_up(
    tempdir: &Path,
    storage_mode: crate::core::storage::StorageMode,
    disable_umount: bool,
) -> Result<()> {
    if disable_umount {
        crate::scoped_log!(
            debug,
            "controller:finalize",
            "cleanup skipped: path={}, reason=disable_umount",
            tempdir.display()
        );
        return Ok(());
    }

    if !crate::utils::is_mount_workspace_path(tempdir) {
        crate::scoped_log!(
            debug,
            "controller:finalize",
            "cleanup skipped: path={}, reason=not_owned_workspace",
            tempdir.display()
        );
        return Ok(());
    }

    clean_up_path(tempdir, storage_mode)
}

fn clean_up_path(tempdir: &Path, storage_mode: crate::core::storage::StorageMode) -> Result<()> {
    crate::scoped_log!(
        info,
        "controller:finalize",
        "cleanup: remove={}",
        tempdir.display()
    );
    detach_tempdir_mount(tempdir)?;
    remove_path(tempdir)?;

    crate::core::storage::cleanup_artifacts(storage_mode)?;
    Ok(())
}

pub(crate) fn clean_up_failed_workspace(tempdir: &Path) -> Result<()> {
    if !crate::utils::is_mount_workspace_path(tempdir) {
        anyhow::bail!(
            "refusing to clean a path outside the Hybrid Mount workspace roots: {}",
            tempdir.display()
        );
    }

    crate::scoped_log!(
        warn,
        "controller:rollback",
        "cleaning failed workspace: path={}",
        tempdir.display()
    );
    detach_tempdir_mount(tempdir)?;
    remove_path(tempdir)
}

fn detach_tempdir_mount(tempdir: &Path) -> Result<()> {
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        let _ = tempdir;
        Ok(())
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        if !is_mounted(tempdir)? {
            return Ok(());
        }

        crate::scoped_log!(
            info,
            "controller:finalize",
            "cleanup umount: path={}",
            tempdir.display()
        );
        if let Err(err) = umount(tempdir, UnmountFlags::DETACH) {
            crate::scoped_log!(
                warn,
                "controller:finalize",
                "cleanup umount failed: path={}, error={:#}",
                tempdir.display(),
                err
            );
            return Err(err.into());
        }
        crate::scoped_log!(
            info,
            "controller:finalize",
            "cleanup umount complete: path={}",
            tempdir.display()
        );
        Ok(())
    }
}

fn remove_path(path: &Path) -> Result<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err.into()),
    };

    if metadata.file_type().is_dir() {
        fs::remove_dir_all(path)
            .with_context(|| format!("failed to remove directory {}", path.display()))?;
    } else {
        fs::remove_file(path)
            .with_context(|| format!("failed to remove file {}", path.display()))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finalize_failure_rolls_back_all_targets_in_reverse_order() {
        let targets = vec![
            PathBuf::from("/system"),
            PathBuf::from("/system/etc/hosts"),
            PathBuf::from("/vendor"),
        ];
        let mut detached = Vec::new();

        let failure = finalize_transaction_with(
            targets,
            || anyhow::bail!("runtime state save failed"),
            |path| {
                detached.push(path.to_path_buf());
                Ok(())
            },
        )
        .unwrap_err();

        assert!(format!("{:#}", failure.error).contains("runtime state save failed"));
        assert!(failure.rollback_complete);
        assert_eq!(
            detached,
            vec![
                PathBuf::from("/vendor"),
                PathBuf::from("/system/etc/hosts"),
                PathBuf::from("/system"),
            ]
        );
    }

    #[test]
    fn successful_finalize_disarms_without_detaching() {
        let mut detached = Vec::new();

        finalize_transaction_with(
            vec![PathBuf::from("/system")],
            || Ok(()),
            |path| {
                detached.push(path.to_path_buf());
                Ok(())
            },
        )
        .unwrap();

        assert!(detached.is_empty());
    }

    #[test]
    fn rollback_failure_keeps_recovery_state_eligible() {
        let failure = finalize_transaction_with(
            vec![PathBuf::from("/system")],
            || anyhow::bail!("state save failed"),
            |_| anyhow::bail!("detach failed"),
        )
        .unwrap_err();

        assert!(!failure.rollback_complete);
        let chain = format!("{:#}", failure.error);
        assert!(chain.contains("state save failed"));
        assert!(chain.contains("detach failed"));
    }
}
