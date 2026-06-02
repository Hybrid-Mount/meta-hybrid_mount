// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::{collections::HashSet, path::Path};

use anyhow::{Result, bail};

use crate::{
    conf::config::{Config, OverlayMode},
    core::{
        backend_capabilities::BackendCapabilities,
        inventory::Module,
        ops::{mirror_sync, plan::MountPlan},
        storage,
    },
    defs,
    mount::kasumi,
};

pub struct KasumiCoordinator<'a> {
    config: &'a Config,
}

impl<'a> KasumiCoordinator<'a> {
    pub fn new(config: &'a Config) -> Self {
        Self { config }
    }

    pub fn prepare_mirror_storage(
        &self,
        capabilities: &BackendCapabilities,
        modules: &[Module],
        plan: &MountPlan,
        source_base: &Path,
    ) -> Result<()> {
        if !capabilities.can_use_kasumi() || plan.kasumi_module_ids.is_empty() {
            return Ok(());
        }

        let kasumi_ids: HashSet<&str> = plan.kasumi_module_ids.iter().map(String::as_str).collect();
        let mut kasumi_modules = Vec::new();
        for module in modules {
            if !kasumi_ids.contains(module.id.as_str()) {
                continue;
            }
            let source_path = source_base.join(&module.id);
            if !source_path.exists() {
                bail!(
                    "planned Kasumi module {} is missing prepared storage at {}",
                    module.id,
                    source_path.display()
                );
            }
            kasumi_modules.push(Module {
                id: module.id.clone(),
                source_path,
                rules: module.rules.clone(),
            });
        }
        if kasumi_modules.len() != plan.kasumi_module_ids.len() {
            bail!(
                "planned Kasumi modules are not present in inventory: expected={}, found={}",
                plan.kasumi_module_ids.len(),
                kasumi_modules.len()
            );
        }

        let kasumi_sources = kasumi_modules
            .iter()
            .map(|module| module.source_path.clone())
            .collect::<Vec<_>>();

        crate::scoped_log!(
            info,
            "kasumi:coordinator",
            "mirror storage start: target={}, modules={}",
            self.config.kasumi.mirror_path.display(),
            kasumi_modules.len()
        );

        let kasumi_storage = storage::setup_with_sources(
            &self.config.kasumi.mirror_path,
            &kasumi_sources,
            matches!(self.config.overlay_mode, OverlayMode::Ext4),
            &self.config.mountsource,
            true,
            Path::new(defs::KASUMI_IMG_FILE),
        )?;

        mirror_sync::sync_modules(&kasumi_modules, kasumi_storage.mount_point())?;

        crate::scoped_log!(
            info,
            "kasumi:coordinator",
            "mirror storage complete: mode={}, target={}",
            kasumi_storage.mode().as_str(),
            self.config.kasumi.mirror_path.display()
        );

        Ok(())
    }

    pub fn reset_runtime(&self) -> Result<bool> {
        kasumi::reset_runtime(self.config)
    }

    pub fn apply_runtime(&self, plan: &mut MountPlan, modules: &[Module]) -> Result<bool> {
        kasumi::apply(plan, modules, self.config)
    }

    pub fn hide_overlay_xattrs(&self, target: &Path) {
        if !self.config.kasumi.enabled
            || !self.config.kasumi.enable_hidexattr
            || !kasumi::can_operate(self.config)
        {
            return;
        }

        if let Err(err) = crate::sys::kasumi::hide_overlay_xattrs(target) {
            crate::scoped_log!(
                warn,
                "kasumi:coordinator",
                "hide overlay xattrs failed: target={}, error={:#}",
                target.display(),
                err
            );
        }
    }
}
