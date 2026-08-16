// ReHybrid-Mount
//
// SPDX-License-Identifier: GPL-3.0-only

use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

/// 全工程统一错误类型。后续 Stage 会按子系统补充挂载/扫描/规划变体。
#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),

    #[error("TOML serialize error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("{0}")]
    Msg(String),
}

impl Error {
    pub fn msg(message: impl Into<String>) -> Self {
        Self::Msg(message.into())
    }
}
