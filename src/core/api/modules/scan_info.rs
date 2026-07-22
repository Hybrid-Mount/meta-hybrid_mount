// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::{fs, path::Path};

use anyhow::{Context, Result};

use super::metadata::{ModuleMetadata, read_module_metadata};
use crate::defs;

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct ModuleMarkers {
    blocked: bool,
}

impl ModuleMarkers {
    pub(super) fn blocks_mount(self) -> bool {
        self.blocked
    }
}

#[derive(Debug, Clone)]
pub(super) struct ModuleScanInfo {
    pub(super) metadata: ModuleMetadata,
    pub(super) markers: ModuleMarkers,
}

pub(super) fn module_scan_info(module_path: &Path, module_id: &str) -> Result<ModuleScanInfo> {
    let metadata = read_module_metadata(module_path, module_id)?;
    let mut markers = ModuleMarkers::default();

    let entries = fs::read_dir(module_path)
        .with_context(|| format!("failed to read module directory {}", module_path.display()))?;
    for entry in entries {
        let entry = entry.with_context(|| {
            format!(
                "failed to enumerate module directory {}",
                module_path.display()
            )
        })?;
        let file_name = entry.file_name();
        if [
            defs::DISABLE_FILE_NAME,
            defs::REMOVE_FILE_NAME,
            defs::SKIP_MOUNT_FILE_NAME,
        ]
        .into_iter()
        .any(|marker| file_name == marker)
        {
            markers.blocked = true;
        }
    }

    Ok(ModuleScanInfo { metadata, markers })
}
