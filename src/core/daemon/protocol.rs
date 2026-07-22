// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

// ── Request / Response envelope ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonRequest {
    pub command: DaemonCommand,
    pub config_path: PathBuf,
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
    #[cfg(feature = "kasumi")]
    Kasumi(KasumiCommand),
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
    List,
    #[serde(rename = "api-modules-apply")]
    Apply {
        modules: Vec<crate::core::api::ModuleApplyEntry>,
    },
}

// ── Kasumi: LKM, rules, hide, maps, uname, runtime ─────────────────────

#[cfg(feature = "kasumi")]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum KasumiCommand {
    // -- core status / info
    #[serde(rename = "kasumi-status")]
    Status,
    #[serde(rename = "kasumi-list")]
    List,
    #[serde(rename = "kasumi-version")]
    Version,
    #[serde(rename = "kasumi-features")]
    Features,
    #[serde(rename = "kasumi-hooks")]
    Hooks,
    #[serde(rename = "kasumi-apply-config-runtime")]
    ApplyConfigRuntime,
    #[serde(rename = "kasumi-clear")]
    Clear,
    #[serde(rename = "kasumi-release-connection")]
    ReleaseConnection,
    #[serde(rename = "kasumi-invalidate-cache")]
    InvalidateCache,
    #[serde(rename = "kasumi-fix-mounts")]
    FixMounts,
    // -- uname
    #[serde(rename = "kasumi-restore-uname-global")]
    RestoreUnameGlobal,
    #[serde(rename = "kasumi-set-uname")]
    SetUname {
        mode: String,
        release: String,
        version: String,
    },
    #[serde(rename = "kasumi-clear-uname")]
    ClearUname { mode: String },
    // -- rules
    #[serde(rename = "kasumi-rule-add")]
    RuleAdd {
        target: PathBuf,
        source: PathBuf,
        file_type: i32,
    },
    #[serde(rename = "kasumi-rule-merge")]
    RuleMerge { target: PathBuf, source: PathBuf },
    #[serde(rename = "kasumi-rule-hide")]
    RuleHide { path: PathBuf },
    #[serde(rename = "kasumi-rule-delete")]
    RuleDelete { path: PathBuf },
    #[serde(rename = "kasumi-rule-add-dir")]
    RuleAddDir {
        target_base: PathBuf,
        source_dir: PathBuf,
    },
    #[serde(rename = "kasumi-rule-remove-dir")]
    RuleRemoveDir {
        target_base: PathBuf,
        source_dir: PathBuf,
    },
    // -- user hide
    #[serde(rename = "hide-list")]
    HideList,
    #[serde(rename = "hide-add")]
    HideAdd { path: PathBuf },
    #[serde(rename = "hide-remove")]
    HideRemove { path: PathBuf },
    #[serde(rename = "hide-apply")]
    HideApply,
    // -- LKM
    #[serde(rename = "lkm-status")]
    LkmStatus,
    #[serde(rename = "lkm-load")]
    LkmLoad,
    #[serde(rename = "lkm-unload")]
    LkmUnload,
    // -- maps spoof
    #[serde(rename = "api-kasumi-maps-add")]
    MapsAdd { rule: serde_json::Value },
    #[serde(rename = "api-kasumi-maps-clear")]
    MapsClear,
}
