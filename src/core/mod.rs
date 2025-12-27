// Copyright 2025 Meta-Hybrid Mount Authors
// SPDX-License-Identifier: GPL-3.0-or-later

pub mod executor;
pub mod granary;
pub mod inventory;
pub mod modules;
pub mod planner;
pub mod state;
pub mod storage;
pub mod sync;
pub mod winnow;

use std::path::Path;

use anyhow::Result;

use crate::conf::config::Config;
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::ksu::try_umount;

pub struct Init;

pub struct StorageReady {
    pub handle: storage::StorageHandle,
}

pub struct ModulesReady {
    pub handle: storage::StorageHandle,
    pub modules: Vec<inventory::Module>,
}

pub struct Planned {
    pub handle: storage::StorageHandle,
    pub modules: Vec<inventory::Module>,
    pub plan: planner::MountPlan,
}

pub struct Executed {
    pub handle: storage::StorageHandle,
    #[allow(dead_code)]
    pub modules: Vec<inventory::Module>,
    pub plan: planner::MountPlan,
    pub result: executor::ExecutionResult,
}

pub struct OryzaEngine<S> {
    config: Config,
    state: S,
}

impl OryzaEngine<Init> {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            state: Init,
        }
    }

    pub fn init_storage(
        self,
        mnt_base: &Path,
        img_path: &Path,
    ) -> Result<OryzaEngine<StorageReady>> {
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

        log::info!(">> Storage Backend: [{}]", handle.mode.to_uppercase());

        Ok(OryzaEngine {
            config: self.config,
            state: StorageReady { handle },
        })
    }
}

impl OryzaEngine<StorageReady> {
    pub fn scan_and_sync(mut self) -> Result<OryzaEngine<ModulesReady>> {
        let modules = inventory::scan(&self.config.moduledir, &self.config)?;

        log::info!(
            ">> Inventory Scan: Found {} enabled modules.",
            modules.len()
        );

        sync::perform_sync(&modules, &self.state.handle.mount_point)?;

        self.state.handle.commit(self.config.disable_umount)?;

        Ok(OryzaEngine {
            config: self.config,
            state: ModulesReady {
                handle: self.state.handle,
                modules,
            },
        })
    }
}

impl OryzaEngine<ModulesReady> {
    pub fn generate_plan(self) -> Result<OryzaEngine<Planned>> {
        let plan = planner::generate(
            &self.config,
            &self.state.modules,
            &self.state.handle.mount_point,
        )?;

        plan.print_visuals();

        Ok(OryzaEngine {
            config: self.config,
            state: Planned {
                handle: self.state.handle,
                modules: self.state.modules,
                plan,
            },
        })
    }
}

impl OryzaEngine<Planned> {
    pub fn execute(self) -> Result<OryzaEngine<Executed>> {
        log::info!(">> Link Start! Executing mount plan...");

        let result = executor::execute(&self.state.plan, &self.config)?;

        Ok(OryzaEngine {
            config: self.config,
            state: Executed {
                handle: self.state.handle,
                modules: self.state.modules,
                plan: self.state.plan,
                result,
            },
        })
    }
}

impl OryzaEngine<Executed> {
    pub fn finalize(self) -> Result<()> {
        let mut nuke_active = false;

        #[cfg(any(target_os = "linux", target_os = "android"))]
        if self.state.handle.mode == "ext4" && self.config.enable_nuke {
            log::info!(">> Engaging Paw Pad Protocol (Stealth)...");

            match try_umount::ksu_nuke_sysfs(
                self.state.handle.mount_point.to_string_lossy().as_ref(),
            ) {
                Ok(_) => {
                    log::info!(">> Success: Paw Pad active. Sysfs traces purged.");

                    nuke_active = true;
                }
                Err(e) => {
                    log::warn!("!! Paw Pad failure: {:#}", e);
                }
            }
        }

        modules::update_description(
            &self.state.handle.mode,
            nuke_active,
            self.state.result.overlay_module_ids.len(),
            self.state.result.magic_module_ids.len(),
        );

        let storage_stats = storage::get_usage(&self.state.handle.mount_point);

        let active_mounts: Vec<String> = self
            .state
            .plan
            .overlay_ops
            .iter()
            .map(|op| op.partition_name.clone())
            .collect();

        let state = state::RuntimeState::new(
            self.state.handle.mode,
            self.state.handle.mount_point,
            self.state.result.overlay_module_ids,
            self.state.result.magic_module_ids,
            nuke_active,
            active_mounts,
            storage_stats,
        );

        if let Err(e) = state.save() {
            log::error!("Failed to save runtime state: {:#}", e);
        }

        granary::disengage_ratoon_protocol();

        log::info!(">> System operational. Mount sequence complete.");

        Ok(())
    }
}
