// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::{
    fs,
    io::{self, Read},
    path::Path,
};

use super::VersionPayload;
use crate::defs;

#[derive(Debug, Clone)]
pub(super) struct ModuleMetadata {
    pub(super) name: String,
    pub(super) version: String,
    pub(super) author: String,
    pub(super) description: String,
}

const MAX_MODULE_PROP_BYTES: u64 = 64 * 1024;

pub fn build_version_payload() -> VersionPayload {
    let metadata = read_module_metadata(Path::new(defs::HYBRID_MOUNT_MODULE_DIR), "hybrid_mount");
    VersionPayload {
        version: if metadata.version == "unknown" {
            env!("CARGO_PKG_VERSION").to_string()
        } else {
            metadata.version
        },
    }
}

pub(super) fn read_module_metadata(module_path: &Path, module_id: &str) -> ModuleMetadata {
    let prop_path = module_path.join("module.prop");
    let Ok(metadata) = fs::symlink_metadata(&prop_path) else {
        return default_module_metadata(module_id);
    };
    if !metadata.file_type().is_file() {
        return default_module_metadata(module_id);
    }
    if metadata.len() > MAX_MODULE_PROP_BYTES {
        crate::scoped_log!(
            warn,
            "api:modules",
            "metadata fallback: module={}, path={}, reason=module_prop_too_large, bytes={}, max_bytes={}",
            module_id,
            prop_path.display(),
            metadata.len(),
            MAX_MODULE_PROP_BYTES
        );
        return default_module_metadata(module_id);
    }

    let raw = match read_module_prop_limited(&prop_path) {
        Ok(raw) => raw,
        Err(err) => {
            crate::scoped_log!(
                warn,
                "api:modules",
                "metadata fallback: module={}, path={}, reason=read_failed, error={}",
                module_id,
                prop_path.display(),
                err
            );
            return default_module_metadata(module_id);
        }
    };

    let mut metadata = default_module_metadata(module_id);
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "name" if !value.is_empty() => metadata.name = value.to_string(),
            "version" if !value.is_empty() => metadata.version = value.to_string(),
            "author" if !value.is_empty() => metadata.author = value.to_string(),
            "description" if !value.is_empty() => metadata.description = value.to_string(),
            _ => {}
        }
    }

    metadata
}

fn read_module_prop_limited(prop_path: &Path) -> io::Result<String> {
    let file = fs::File::open(prop_path)?;
    let mut reader = file.take(MAX_MODULE_PROP_BYTES);
    let mut raw = String::new();
    reader.read_to_string(&mut raw)?;
    Ok(raw)
}

fn default_module_metadata(module_id: &str) -> ModuleMetadata {
    ModuleMetadata {
        name: module_id.to_string(),
        version: "unknown".to_string(),
        author: "unknown".to_string(),
        description: "No description".to_string(),
    }
}
