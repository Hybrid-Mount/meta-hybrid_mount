// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::domain::{ModuleRules, MountMode};

mod apply;
mod metadata;
mod payload;
mod runtime_index;
mod scan_cache;

#[cfg(test)]
mod tests;

pub use self::{
    apply::apply_modules_payload, metadata::build_version_payload, payload::build_modules_payload,
};

#[derive(Debug, Clone, Serialize)]
pub struct ModuleListEntry {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub mode: MountMode,
    pub is_mounted: bool,
    pub enabled: bool,
    pub source_path: PathBuf,
    pub rules: ModuleRules,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mount_error: Option<String>,
    #[serde(skip_serializing_if = "is_false")]
    pub suggest_ignore: bool,
}

fn is_false(v: &bool) -> bool {
    !*v
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleApplyEntry {
    pub id: String,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub source_path: Option<PathBuf>,
    #[serde(default)]
    pub rules: ModuleRules,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModulesApplyPayload {
    pub updated: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct VersionPayload {
    pub version: String,
}
