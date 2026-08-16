// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

// ── Request / Response envelope ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonRequest {
    pub command: DaemonCommand,
    #[serde(default)]
    pub config_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl DaemonResponse {
    pub fn success(data: serde_json::Value) -> Self {
        Self {
            ok: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            data: None,
            error: Some(message.into()),
        }
    }
}

// ── Top-level command (untagged → delegates to sub-enums) ────────────────

/// Wire format stays flat: `{"type": "ping"}`, `{"type": "api-config-get"}`, …
/// The `#[serde(untagged)]` outer enum dispatches deserialization to the
/// first internally-tagged sub-enum that matches the `"type"` field.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DaemonCommand {
    System(SystemCommand),
    Config(ConfigCommand),
    Modules(ModulesCommand),
    Batch(BatchCommand),
}

// ── System: health, lifecycle, storage, info, misc ──────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SystemCommand {
    #[serde(rename = "ping")]
    Ping,
    #[serde(rename = "webui-start")]
    WebuiStart,
    #[serde(rename = "shutdown")]
    Shutdown,
    #[serde(rename = "init")]
    Init,
    #[serde(rename = "status")]
    Status,
    #[serde(rename = "api-storage")]
    ApiStorage,
    #[serde(rename = "api-mount-stats")]
    ApiMountStats,
    #[serde(rename = "api-mount-topology")]
    ApiMountTopology,
    #[serde(rename = "api-partitions")]
    ApiPartitions,
    #[serde(rename = "api-system-info")]
    ApiSystemInfo,
    #[serde(rename = "api-version")]
    ApiVersion,
    #[serde(rename = "api-kernel-uname")]
    ApiKernelUname,
    #[serde(rename = "api-open-url")]
    ApiOpenUrl { url: String },
    #[serde(rename = "api-reboot")]
    ApiReboot,
    #[serde(rename = "clear-mount-errors")]
    ClearMountErrors,
}

// ── Config: CRUD for the TOML configuration ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ConfigCommand {
    #[serde(rename = "api-config-get")]
    Get,
    #[serde(rename = "api-config-set")]
    Set { config: serde_json::Value },
    #[serde(rename = "api-config-patch")]
    Patch {
        patch: serde_json::Value,
        /// Deprecated compatibility flag. Runtime application was removed with
        /// the Kasumi backend; the server always reports `applied: false` and
        /// `reboot_required: true`.
        #[serde(default)]
        apply_runtime: bool,
    },
    #[serde(rename = "api-config-reset")]
    Reset,
}

// ── Modules: module listing and bulk operations ─────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ModulesCommand {
    #[serde(rename = "api-modules-list")]
    List { path: Option<PathBuf> },
    #[serde(rename = "api-modules-apply")]
    Apply {
        modules: Vec<crate::core::api::ModuleApplyEntry>,
    },
}
// ── Batch: multiple commands in one round-trip ──────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum BatchCommand {
    #[serde(rename = "batch")]
    Batch { commands: Vec<DaemonCommand> },
}
