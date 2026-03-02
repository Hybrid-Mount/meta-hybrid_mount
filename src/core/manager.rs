// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::{
    conf::config::Config,
    core::{
        inventory::{self, model as modules},
        ops::{
            executor::{self, NativeMount},
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

    pub fn init_storage(
        self,
        mnt_base: &Path,
        img_path: &Path,
    ) -> Result<MountController<StorageReady>> {
        let handle = storage::setup(
            mnt_base,
            img_path,
            &self.config.moduledir,
            matches!(
                self.config.overlay_mode,
                crate::conf::config::OverlayMode::Ext4
            ),
            matches!(
                self.config.overlay_mode,
                crate::conf::config::OverlayMode::Erofs
            ),
            &self.config.mountsource,
            self.config.disable_umount,
        )?;

        Ok(MountController {
            config: self.config,
            state: StorageReady { handle },
            tempdir: self.tempdir,
        })
    }
}

impl MountController<StorageReady> {
    pub fn scan_and_sync(mut self) -> Result<MountController<ModulesReady>> {
        let modules = inventory::scan(&self.config.moduledir, &self.config)?;

        sync::perform_sync(&modules, self.state.handle.mount_point())?;

        if self.state.handle.mode() == "erofs_staging" {
            let needs_magic = modules.iter().any(|m| {
                m.rules.default_mode == inventory::MountMode::Magic
                    || m.rules
                        .paths
                        .values()
                        .any(|v| *v == inventory::MountMode::Magic)
            });

            if needs_magic {
                let magic_ws = self.state.handle.mount_point().join("magic_workspace");
                if !magic_ws.exists() {
                    let _ = std::fs::create_dir(magic_ws);
                }
            }
        }

        self.state.handle.commit(self.config.disable_umount)?;

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
        let plan = planner::generate(
            &self.config,
            &self.state.modules,
            self.state.handle.mount_point(),
        )?;

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
        let driver = NativeMount;
        let result = executor::execute(
            &self.state.plan,
            &self.config,
            self.tempdir.clone(),
            &driver,
        )?;

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

        Ok(())
    }
}
