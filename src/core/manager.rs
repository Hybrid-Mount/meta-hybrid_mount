// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::{
    conf::config::Config,
    core::{
        inventory::{self, model as modules},
        ops::{
            executor::{self},
            planner, sync,
        },
        state,
        storage::{self, StorageHandle},
    },
};

pub struct Init;

pub struct StorageReady {
    pub handle: StorageHandle,
}

pub struct ModulesReady {
    pub handle: StorageHandle,
    pub modules: Vec<inventory::Module>,
}

pub struct Planned {
    pub handle: StorageHandle,
    pub plan: planner::MountPlan,
}

pub struct Executed {
    pub handle: StorageHandle,
    pub plan: planner::MountPlan,
    pub result: executor::ExecutionResult,
}

pub struct MountController<S> {
    config: Config,
    state: S,
    tempdir: PathBuf,
}

impl MountController<Init> {
    pub fn new<P>(config: Config, tempdir: P) -> Self
    where
        P: AsRef<Path>,
    {
        Self {
            config,
            state: Init,
            tempdir: tempdir.as_ref().to_path_buf(),
        }
    }

    pub fn init_storage(self, mnt_base: &Path) -> Result<MountController<StorageReady>> {
        log::info!(
            "[stage:init_storage] preparing mount storage at {}",
            mnt_base.display()
        );
        let handle = storage::setup(
            mnt_base,
            &self.config.moduledir,
            matches!(
                self.config.overlay_mode,
                crate::conf::config::OverlayMode::Ext4
            ),
            &self.config.mountsource,
            self.config.disable_umount,
        )?;

        log::info!(
            "[stage:init_storage] storage ready: mode={}, mount_point={}",
            handle.mode(),
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
    pub fn scan_and_sync(mut self) -> Result<MountController<ModulesReady>> {
        log::info!(
            "[stage:scan_and_sync] scanning modules from {}",
            self.config.moduledir.display()
        );
        let modules = inventory::scan(&self.config.moduledir, &self.config)?;

        log::info!(
            "[stage:scan_and_sync] scan complete: {} module(s)",
            modules.len()
        );

        log::info!("[stage:scan_and_sync] syncing module content into runtime storage");
        sync::perform_sync(&modules, self.state.handle.mount_point())?;

        self.state.handle.commit(self.config.disable_umount)?;

        log::info!("[stage:scan_and_sync] storage commit completed");

        Ok(MountController {
            config: self.config,
            state: ModulesReady {
                handle: self.state.handle,
                modules,
            },
            tempdir: self.tempdir,
        })
    }
}

impl MountController<ModulesReady> {
    pub fn generate_plan(self) -> Result<MountController<Planned>> {
        log::info!("[stage:generate_plan] generating mount plan");
        let plan = planner::generate(
            &self.config,
            &self.state.modules,
            self.state.handle.mount_point(),
        )?;

        log::info!(
            "[stage:generate_plan] plan ready: overlay_ops={}, overlay_modules={}, magic_modules={}",
            plan.overlay_ops.len(),
            plan.overlay_module_ids.len(),
            plan.magic_module_ids.len()
        );

        Ok(MountController {
            config: self.config,
            state: Planned {
                handle: self.state.handle,
                plan,
            },
            tempdir: self.tempdir,
        })
    }
}

impl MountController<Planned> {
    pub fn execute(self) -> Result<MountController<Executed>> {
        log::info!("[stage:execute] applying mount plan");
        let result =
            executor::Executer::execute(&self.state.plan, &self.config, self.tempdir.clone())?;

        log::info!(
            "[stage:execute] execution complete: overlay_mounted={}, magic_mounted={}",
            result.overlay_module_ids.len(),
            result.magic_module_ids.len()
        );

        Ok(MountController {
            config: self.config,
            state: Executed {
                handle: self.state.handle,
                plan: self.state.plan,
                result,
            },
            tempdir: self.tempdir,
        })
    }
}

impl MountController<Executed> {
    pub fn finalize(self) -> Result<()> {
        log::info!("[stage:finalize] writing runtime state and module descriptions");
        modules::update_description(
            self.state.handle.mode(),
            self.state.result.overlay_module_ids.len(),
            self.state.result.magic_module_ids.len(),
        );

        let mut active_mounts: Vec<String> = self
            .state
            .plan
            .overlay_ops
            .iter()
            .map(|op| op.partition_name.clone())
            .collect();

        active_mounts.sort();
        active_mounts.dedup();

        let state = state::RuntimeState::new(
            self.state.handle.mode().to_string(),
            self.state.handle.mount_point().to_path_buf(),
            self.state.result.overlay_module_ids,
            self.state.result.magic_module_ids,
            active_mounts,
        );

        let _ = state.save();

        log::info!("[stage:finalize] boot sequence finalized");

        Ok(())
    }
}
