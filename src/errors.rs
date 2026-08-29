// SPDX-License-Identifier: GPL-3.0-only

use std::fmt;
use std::io;
use std::path::PathBuf;

use thiserror::Error;

use crate::sys::process::{ProcessError, ProcessErrorKind};

pub type Result<T> = std::result::Result<T, Error>;

/// 错误的处置分类。调用方不得根据 Display 文本猜测可重试性。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorClass {
    /// 明确值得重试的瞬态失败(中断、超时、EBUSY 等)。
    Transient,
    /// 重试也不会自行恢复的失败。
    Permanent,
    /// 必须由用户修改模块/配置/设备状态后才能恢复。
    ManualRecovery,
}

/// 带 context/path/source 的 I/O 错误类型。所有字段保持结构化，
/// Display 文本在此生成，调用点不预格式化字符串。
#[derive(Debug)]
pub struct IoError {
    pub context: &'static str,
    pub path: Option<PathBuf>,
    pub source: io::Error,
}

impl IoError {
    pub fn new(context: &'static str, path: Option<PathBuf>, source: io::Error) -> Self {
        Self {
            context,
            path,
            source,
        }
    }
}

impl fmt::Display for IoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.context)?;
        if let Some(path) = &self.path {
            write!(f, " for {}", path.display())?;
        }
        write!(f, ": {}", self.source)
    }
}

impl std::error::Error for IoError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

/// 带 operation/path/source 的层边界上下文错误。
///
/// 各子系统变体共用该结构：错误文本在 Display 时生成，
/// 调用点只提供结构化字段，不再把错误提前拼成字符串。
#[derive(Debug)]
pub struct ContextError {
    pub operation: &'static str,
    pub path: Option<PathBuf>,
    pub source: CausalError,
}

impl ContextError {
    pub fn new(
        operation: &'static str,
        path: Option<PathBuf>,
        source: impl Into<CausalError>,
    ) -> Self {
        Self {
            operation,
            path,
            source: source.into(),
        }
    }
}

impl fmt::Display for ContextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.operation)?;
        if let Some(path) = &self.path {
            write!(f, " for {}", path.display())?;
        }
        write!(f, ": {}", self.source)
    }
}

impl std::error::Error for ContextError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

/// 层边界统一翻译后的原因类型。子进程、rustix、procfs 与 serde 错误
/// 都在这里收敛，而不是让调用点各自格式化。
#[derive(Debug, Error)]
pub enum CausalError {
    #[error(transparent)]
    Io(#[from] io::Error),

    #[error(transparent)]
    TomlParse(#[from] toml::de::Error),

    #[error(transparent)]
    TomlSerialize(#[from] toml::ser::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[error(transparent)]
    Errno(#[from] rustix::io::Errno),

    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[error(transparent)]
    Procfs(#[from] procfs::ProcError),

    #[error(transparent)]
    Subprocess(#[from] ProcessError),

    #[error("{0}")]
    Message(String),
}

impl CausalError {
    pub fn classify(&self) -> ErrorClass {
        match self {
            Self::Io(source) => classify_io(source),
            Self::TomlParse(_) | Self::TomlSerialize(_) | Self::Json(_) => ErrorClass::Permanent,
            #[cfg(any(target_os = "linux", target_os = "android"))]
            Self::Errno(errno) => classify_errno(errno),
            #[cfg(any(target_os = "linux", target_os = "android"))]
            Self::Procfs(_) => ErrorClass::Permanent,
            Self::Subprocess(err) => classify_process(err),
            Self::Message(_) => ErrorClass::Permanent,
        }
    }
}

impl From<String> for CausalError {
    fn from(message: String) -> Self {
        Self::Message(message)
    }
}

/// 全工程统一错误类型。
///
/// 已按子系统建立可匹配变体；`Msg` 仅保留给尚未迁移的低风险路径，
/// 新代码禁止再通过 `Error::msg` 预格式化可结构化的错误。
#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[cfg_attr(not(any(target_os = "linux", target_os = "android")), allow(dead_code))]
    #[error("{0}")]
    IoContext(Box<IoError>),

    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),

    #[error("TOML serialize error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),

    #[error("read config {}: {source}", path.display())]
    ConfigRead { path: PathBuf, source: io::Error },

    #[error("parse config {}: {source}", path.display())]
    ConfigParse {
        path: PathBuf,
        source: toml::de::Error,
    },

    #[error("global default_mode=ignore is not supported; set per-module ignore rules instead")]
    UnsupportedGlobalDefaultMode,

    #[error("read module blacklist {}: {source}", path.display())]
    ModuleBlacklistRead { path: PathBuf, source: io::Error },

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

    #[error("Invalid module ID: '{module_id:?}'. Must match /^[a-zA-Z][a-zA-Z0-9._-]*$/")]
    InvalidModuleID { module_id: String },

    #[error("read module directory {}: {source}", path.display())]
    ScanReadDir { path: PathBuf, source: io::Error },

    #[error(
        "duplicate module id {module_id:?}: declared by {} and {}",
        first.display(),
        second.display()
    )]
    DuplicateModuleId {
        module_id: String,
        first: PathBuf,
        second: PathBuf,
    },

    #[error("invalid module id in module blacklist {}: {module_id:?}", path.display())]
    InvalidBlacklistModuleId { path: PathBuf, module_id: String },

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
    Mount(Box<ContextError>),

    #[error("{0}")]
    Storage(Box<ContextError>),

    #[error("{0}")]
    Lkm(Box<ContextError>),

    #[error("{0}")]
    State(Box<ContextError>),

    #[error("{0}")]
    Subprocess(#[from] ProcessError),

    #[error("{0}")]
    Msg(String),
}

impl Error {
    pub fn msg(message: impl Into<String>) -> Self {
        Self::Msg(message.into())
    }

    /// 穷尽匹配每个变体；新增变体必须在这里显式选择分类。
    pub fn classify(&self) -> ErrorClass {
        match self {
            Self::Io(source) => classify_io(source),
            Self::IoContext(err) => classify_io(&err.source),
            Self::TomlParse(_) | Self::TomlSerialize(_) | Self::Json(_) => ErrorClass::Permanent,
            Self::ConfigRead { source, .. } => classify_io(source),
            Self::ConfigParse { .. } => ErrorClass::ManualRecovery,
            Self::UnsupportedGlobalDefaultMode => ErrorClass::ManualRecovery,
            Self::ModuleBlacklistRead { source, .. } => classify_io(source),
            Self::ModuleBlacklistParse { .. } => ErrorClass::ManualRecovery,
            Self::MountRootSymlink { .. }
            | Self::MountRootFile { .. }
            | Self::DirDeclared { .. }
            | Self::InvalidModuleID { .. }
            | Self::InvalidBlacklistModuleId { .. } => ErrorClass::ManualRecovery,
            Self::RegularDirectory { .. } => ErrorClass::Permanent,
            Self::ScanReadDir { source, .. } => classify_io(source),
            Self::DuplicateModuleId { .. } | Self::PlanConflict { .. } => {
                ErrorClass::ManualRecovery
            }
            #[cfg(any(target_os = "linux", target_os = "android"))]
            Self::Sys(errno) => classify_errno(errno),
            Self::Mount(err) | Self::Storage(err) | Self::Lkm(err) | Self::State(err) => {
                err.source.classify()
            }
            Self::Subprocess(err) => classify_process(err),
            Self::Msg(_) => ErrorClass::Permanent,
        }
    }

    pub fn is_retryable(&self) -> bool {
        self.classify() == ErrorClass::Transient
    }

    pub fn requires_manual_intervention(&self) -> bool {
        self.classify() == ErrorClass::ManualRecovery
    }
}

fn classify_io(source: &io::Error) -> ErrorClass {
    match source.kind() {
        io::ErrorKind::Interrupted
        | io::ErrorKind::WouldBlock
        | io::ErrorKind::TimedOut
        | io::ErrorKind::ConnectionReset
        | io::ErrorKind::ConnectionAborted
        | io::ErrorKind::ConnectionRefused => ErrorClass::Transient,
        io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied => ErrorClass::ManualRecovery,
        _ => ErrorClass::Permanent,
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn classify_errno(errno: &rustix::io::Errno) -> ErrorClass {
    let code = errno.raw_os_error();
    if code == libc::EINTR || code == libc::EAGAIN || code == libc::EBUSY || code == libc::ESTALE {
        ErrorClass::Transient
    } else {
        ErrorClass::Permanent
    }
}

fn classify_process(err: &ProcessError) -> ErrorClass {
    match err.kind {
        ProcessErrorKind::Spawn { ref source }
        | ProcessErrorKind::Wait { ref source }
        | ProcessErrorKind::Reader { ref source, .. } => classify_io(source),
        ProcessErrorKind::PipeMissing { .. } | ProcessErrorKind::UnexpectedExit(_) => {
            ErrorClass::Permanent
        }
        ProcessErrorKind::Timeout { .. } | ProcessErrorKind::DrainTimeout { .. } => {
            ErrorClass::Transient
        }
    }
}

#[cfg(test)]
#[path = "errors_tests.rs"]
mod tests;
