// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

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
        let mirror_path = validate_mirror_path(&self.config.kasumi.mirror_path)?;

        let kasumi_storage = storage::setup_with_sources(
            &mirror_path,
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

    pub fn hide_overlay_xattrs(&self, target: &Path) -> Result<()> {
        if !self.config.kasumi.enabled
            || !self.config.kasumi.enable_overlay_xattr_hide
            || !kasumi::can_operate(self.config)?
        {
            return Ok(());
        }

        crate::sys::kasumi::hide_overlay_xattrs(target)
    }
}

fn validate_mirror_path(path: &Path) -> Result<PathBuf> {
    if !path.is_absolute() {
        bail!("Kasumi mirror_path must be absolute: {}", path.display());
    }

    let normalized = crate::utils::normalize_path(path);
    let default = Path::new(defs::KASUMI_MIRROR_DIR);
    if normalized == default {
        return Ok(normalized);
    }

    let allowed_parent = Path::new("/dev");
    if normalized.parent() == Some(allowed_parent)
        && normalized
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("kasumi_mirror"))
    {
        return Ok(normalized);
    }

    bail!(
        "Kasumi mirror_path must be {} or /dev/kasumi_mirror*: {}",
        defs::KASUMI_MIRROR_DIR,
        path.display()
    )
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::validate_mirror_path;
    use crate::defs;

    #[test]
    fn validate_mirror_path_accepts_default() {
        assert_eq!(
            validate_mirror_path(Path::new(defs::KASUMI_MIRROR_DIR)).unwrap(),
            Path::new(defs::KASUMI_MIRROR_DIR)
        );
    }

    #[test]
    fn validate_mirror_path_accepts_dev_kasumi_prefix() {
        assert_eq!(
            validate_mirror_path(Path::new("/dev/kasumi_mirror_test")).unwrap(),
            Path::new("/dev/kasumi_mirror_test")
        );
    }

    #[test]
    fn validate_mirror_path_rejects_root_and_system_paths() {
        assert!(validate_mirror_path(Path::new("/")).is_err());
        assert!(validate_mirror_path(Path::new("/system")).is_err());
        assert!(validate_mirror_path(Path::new("/data/adb/kasumi_mirror")).is_err());
    }

    #[test]
    fn validate_mirror_path_rejects_relative_paths() {
        assert!(validate_mirror_path(Path::new("kasumi_mirror")).is_err());
    }
}
