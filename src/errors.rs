// SPDX-License-Identifier: GPL-3.0-only

use std::fmt;
use std::io;
use std::path::PathBuf;

use thiserror::Error;

use crate::sys::process::{ProcessError, ProcessErrorKind};

pub type Result<T> = std::result::Result<T, Error>;

/// How an error should be handled. Callers must not infer retryability from Display text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorClass {
    /// Transient failures worth retrying: interruption, timeout, EBUSY and similar.
    Transient,
    /// Failures that retrying alone will not recover.
    Permanent,
    /// Failures the user must resolve by changing a module, the config or the device state.
    ManualRecovery,
}

impl ErrorClass {
    /// Stable error-class token used in boot logs.
    pub fn label(self) -> &'static str {
        match self {
            Self::Transient => "transient",
            Self::Permanent => "permanent",
            Self::ManualRecovery => "manual_recovery",
        }
    }
}

/// I/O error carrying context, path and source. Fields stay structured and the
/// Display text is generated here, so call sites never pre-format strings.
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

/// Layering-boundary context error carrying operation, path and source.
///
/// Shared by every subsystem variant: the text is generated at Display time and
/// call sites supply structured fields instead of pre-joining an error into a string.
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

/// Cause type after the layering boundary translation. Subprocess, rustix, procfs
/// and serde errors all converge here rather than being formatted per call site.
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

/// The project-wide error type.
///
/// Variants are matchable per subsystem; `Msg` remains only for low-risk paths not yet
/// migrated, and new code must not use `Error::msg` to pre-format a structurable error.
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
    Vfs(Box<ContextError>),

    #[error("VFS protocol error: {detail}")]
    VfsProtocol { detail: String },

    #[error("VFS kernel provider is unavailable: {reason}")]
    VfsUnavailable { reason: String },

    #[error("a foreign NoMount VFS kernel implementation is present: {detail}")]
    VfsForeignNomount { detail: String },

    #[error("unsupported VFS protocol version {found:?} (supported: {supported})")]
    VfsUnsupportedVersion { found: String, supported: String },

    #[error("{detail}\nRun 'hybrid-mount vfs help' for usage.")]
    VfsCliUsage { detail: String },

    #[error("{0}")]
    Subprocess(#[from] ProcessError),

    #[error("{0}")]
    Msg(String),
}

impl Error {
    /// Only the new VFS argument contract uses exit code 2; old commands keep code 1.
    pub fn exit_code(&self) -> i32 {
        if matches!(self, Self::VfsCliUsage { .. }) {
            2
        } else {
            1
        }
    }

    pub fn msg(message: impl Into<String>) -> Self {
        Self::Msg(message.into())
    }

    /// Exhaustively matched over every variant; a new variant must pick a class here.
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
            Self::Mount(err)
            | Self::Storage(err)
            | Self::Lkm(err)
            | Self::State(err)
            | Self::Vfs(err) => err.source.classify(),
            Self::VfsProtocol { .. } | Self::VfsCliUsage { .. } => ErrorClass::Permanent,
            Self::VfsUnavailable { .. }
            | Self::VfsForeignNomount { .. }
            | Self::VfsUnsupportedVersion { .. } => ErrorClass::ManualRecovery,
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
    let transient = [
        rustix::io::Errno::INTR.raw_os_error(),
        rustix::io::Errno::AGAIN.raw_os_error(),
        rustix::io::Errno::BUSY.raw_os_error(),
        rustix::io::Errno::STALE.raw_os_error(),
    ];
    if transient.contains(&code) {
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
