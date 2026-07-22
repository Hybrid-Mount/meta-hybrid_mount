// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use serde::{Deserialize, Serialize};

use crate::domain::{ModuleRules, MountMode};

mod apply;
mod metadata;
mod payload;
mod runtime_index;
mod scan_info;

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
    pub is_blacklisted: bool,
    pub rules: ModuleRules,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleApplyEntry {
    pub id: String,
    pub enabled: Option<bool>,
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
