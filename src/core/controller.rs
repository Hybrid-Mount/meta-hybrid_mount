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

#[cfg(feature = "kasumi")]
use crate::core::failure::ModuleStageFailure;
#[cfg(feature = "kasumi")]
use crate::core::kasumi_coordinator::KasumiCoordinator;
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::sys::mount::is_mounted;
use crate::{
    conf::config::Config,
    core::{
        backend_capabilities::BackendCapabilities,
        inventory::{self},
        ops::{
            executor::{self},
            plan::MountPlan,
            prepare,
        },
        runtime_finalization,
        storage::StorageHandle,
    },
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
    backend_capabilities: BackendCapabilities,
    state: S,
    tempdir: PathBuf,
}

impl MountController<Init> {
    pub fn new<P>(config: Config, tempdir: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let timer = crate::utils::StageTimer::start("controller", "backend_detection");
        let backend_capabilities = BackendCapabilities::detect(&config)?;
        timer.finish();
        Ok(Self {
            backend_capabilities,
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
            backend_capabilities: self.backend_capabilities,
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
        let plan = prepare::prepare_mount_plan(
            modules,
            self.state.handle.mount_point(),
            &self.backend_capabilities,
        )?;

        crate::scoped_log!(
            info,
            "controller:scan_and_prepare_plan",
            "prepare complete: overlay_ops={}, overlay_modules={}, magic_modules={}, kasumi_modules={}, copied_entries={}, copied_bytes={}, kasumi_rule_compile=deferred",
            plan.overlay_ops.len(),
            plan.overlay_module_ids.len(),
            plan.magic_module_ids.len(),
            {
                #[cfg(feature = "kasumi")]
                {
                    plan.kasumi_module_ids.len()
                }
                #[cfg(not(feature = "kasumi"))]
                {
                    0usize
                }
            },
            plan.prepare_metrics.copied_entries,
            plan.prepare_metrics.copied_bytes,
        );

        #[cfg(feature = "kasumi")]
        {
            let timer = crate::utils::StageTimer::start("controller", "kasumi_mirror_sync");
            let kasumi = KasumiCoordinator::new(&self.config);
            kasumi
                .prepare_mirror_storage(
                    &self.backend_capabilities,
                    modules,
                    &plan,
                    self.state.handle.mount_point(),
                )
                .map_err(|err| {
                    ModuleStageFailure::sync(
                        plan.kasumi_module_ids.clone(),
                        anyhow::anyhow!("Failed to prepare Kasumi mirror storage: {:#}", err),
                    )
                })?;
            timer.finish();
        }

        Ok(MountController {
            config: self.config,
            backend_capabilities: self.backend_capabilities,
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
            "complete: overlay_mounted={}, magic_mounted={}, kasumi_mounted={}",
            result.overlay_module_ids.len(),
            result.magic_module_ids.len(),
            result.kasumi_count()
        );

        Ok(MountController {
            config: self.config,
            backend_capabilities: self.backend_capabilities,
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
    pub fn finalize(self) -> Result<()> {
        let timer = crate::utils::StageTimer::start("controller", "finalize_total");
        crate::scoped_log!(info, "controller:finalize", "start");
        runtime_finalization::finalize(
            &self.config,
            self.state.handle.mode(),
            self.state.handle.mount_point(),
            &self.state.result,
            &self.state.inventory_summary,
        )?;

        let cleanup_timer = crate::utils::StageTimer::start("controller", "cleanup");
        clean_up(
            &self.tempdir,
            &self.config.kasumi.mirror_path,
            self.state.handle.mode(),
            self.config.disable_umount,
        )?;
        cleanup_timer.finish();
        timer.finish();

        crate::scoped_log!(info, "controller:finalize", "complete");

        Ok(())
    }
}

fn clean_up(
    tempdir: &Path,
    kasumi_mirror_path: &Path,
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

    if !tempdir.starts_with("/mnt") {
        crate::scoped_log!(
            debug,
            "controller:finalize",
            "cleanup skipped: path={}, reason=outside_mnt",
            tempdir.display()
        );
        return Ok(());
    }

    clean_up_path(tempdir, kasumi_mirror_path, storage_mode)
}

fn clean_up_path(
    tempdir: &Path,
    kasumi_mirror_path: &Path,
    storage_mode: crate::core::storage::StorageMode,
) -> Result<()> {
    if tempdir == kasumi_mirror_path {
        crate::scoped_log!(
            info,
            "controller:finalize",
            "cleanup skipped: path={}, reason=kasumi_mirror",
            tempdir.display()
        );
        return Ok(());
    }

    if kasumi_mirror_path.starts_with(tempdir) {
        let preserved_child = kasumi_mirror_path
            .strip_prefix(tempdir)
            .with_context(|| {
                format!(
                    "failed to resolve Kasumi mirror {} under {}",
                    kasumi_mirror_path.display(),
                    tempdir.display()
                )
            })?
            .components()
            .next()
            .map(|component| component.as_os_str().to_owned())
            .context("Kasumi mirror path has no child component")?;

        crate::scoped_log!(
            info,
            "controller:finalize",
            "cleanup partial: path={}, preserve={}",
            tempdir.display(),
            kasumi_mirror_path.display()
        );

        let entries = match fs::read_dir(tempdir) {
            Ok(entries) => entries,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(err.into()),
        };

        for entry in entries {
            let entry = entry?;
            if entry.file_name() == preserved_child {
                continue;
            }
            remove_path(&entry.path())?;
        }

        return Ok(());
    }

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
