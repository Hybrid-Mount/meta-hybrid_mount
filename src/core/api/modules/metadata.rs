// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::{
    fs,
    io::{self, Read},
    path::Path,
};

use anyhow::{Context, Result, bail};

use super::VersionPayload;

#[derive(Debug, Clone)]
pub(super) struct ModuleMetadata {
    pub(super) name: String,
    pub(super) version: String,
    pub(super) author: String,
    pub(super) description: String,
}

const MAX_MODULE_PROP_BYTES: u64 = 64 * 1024;

pub fn build_version_payload() -> VersionPayload {
    VersionPayload {
        version: env!("CARGO_PKG_VERSION").to_string(),
    }
}

pub(super) fn read_module_metadata(module_path: &Path, module_id: &str) -> Result<ModuleMetadata> {
    let prop_path = module_path.join("module.prop");
    let metadata = fs::symlink_metadata(&prop_path)
        .with_context(|| format!("missing module.prop for module {module_id}"))?;
    if !metadata.file_type().is_file() {
        bail!("module.prop is not a regular file for module {module_id}");
    }
    if metadata.len() > MAX_MODULE_PROP_BYTES {
        bail!(
            "module.prop is too large for module {}: {} bytes exceeds {}",
            module_id,
            metadata.len(),
            MAX_MODULE_PROP_BYTES
        );
    }

    let raw = read_module_prop_limited(&prop_path)
        .with_context(|| format!("failed to read module.prop for module {module_id}"))?;

    let mut name = None;
    let mut version = None;
    let mut author = None;
    let mut description = None;
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
            "name" if !value.is_empty() => name = Some(value.to_string()),
            "version" if !value.is_empty() => version = Some(value.to_string()),
            "author" if !value.is_empty() => author = Some(value.to_string()),
            "description" if !value.is_empty() => description = Some(value.to_string()),
            _ => {}
        }
    }

    Ok(ModuleMetadata {
        name: name.with_context(|| format!("module {module_id} has no name"))?,
        version: version.with_context(|| format!("module {module_id} has no version"))?,
        author: author.with_context(|| format!("module {module_id} has no author"))?,
        description: description
            .with_context(|| format!("module {module_id} has no description"))?,
    })
}

fn read_module_prop_limited(prop_path: &Path) -> io::Result<String> {
    let file = fs::File::open(prop_path)?;
    let mut reader = file.take(MAX_MODULE_PROP_BYTES);
    let mut raw = String::new();
    reader.read_to_string(&mut raw)?;
    Ok(raw)
}
