// SPDX-License-Identifier: GPL-3.0-only

use std::path::PathBuf;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

/// 全工程统一错误类型。后续 Stage 会按子系统补充扫描/规划变体。
#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),

    #[error("TOML serialize error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),

    #[error("read config {}: {source}", path.display())]
    ConfigRead {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("parse config {}: {source}", path.display())]
    ConfigParse {
        path: PathBuf,
        source: toml::de::Error,
    },

    #[error("global default_mode=ignore is not supported; set per-module ignore rules instead")]
    UnsupportedGlobalDefaultMode,

    #[error("read module blacklist {}: {source}", path.display())]
    ModuleBlacklistRead {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("parse module blacklist {}: {source}", path.display())]
    ModuleBlacklistParse {
        path: PathBuf,
        source: toml::de::Error,
    },

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[cfg_attr(not(any(target_os = "linux", target_os = "android")), allow(dead_code))]
    #[error("cannot mount root symlink {path:?}!")]
    MountRootSymlink { path: String },

    #[cfg_attr(not(any(target_os = "linux", target_os = "android")), allow(dead_code))]
    #[error("cannot mount root file {path:?}!")]
    MountRootFile { path: String },

    #[cfg_attr(not(any(target_os = "linux", target_os = "android")), allow(dead_code))]
    #[error("dir {path:?} is declared as replaced but it is root!")]
    DirDeclared { path: String },

    #[error("{path:?} is not a regular directory")]
    RegularDirectory { path: String },

    #[error("Invalid module ID: '{module_id:?}'. Must match /^[a-zA-Z][a-zA-Z0-9._-]+$/")]
    InvalidModuleID { module_id: String },

    #[error(
        "plan conflict at {target:?}: {first_backend}({first_source}) vs {second_backend}({second_source})"
    )]
    PlanConflict {
        target: String,
        first_backend: String,
        first_source: String,
        second_backend: String,
        second_source: String,
    },

    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[error("system call error: {0}")]
    Sys(#[from] rustix::io::Errno),

    #[error("{0}")]
    Msg(String),
}

impl Error {
    pub fn msg(message: impl Into<String>) -> Self {
        Self::Msg(message.into())
    }
}
