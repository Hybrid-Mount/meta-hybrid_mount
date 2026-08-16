// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

pub mod path;
mod timing;
pub mod validation;

use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::OnceLock,
};

#[cfg(not(target_os = "android"))]
use anyhow::Context;
use anyhow::{Result, bail};
pub use timing::StageTimer;

pub use self::{path::*, validation::*};

const LOG_LEVEL_ENV: &str = "HYBRID_MOUNT_LOG_LEVEL";
const DEFAULT_LOG_LEVEL: log::LevelFilter = log::LevelFilter::Info;

#[macro_export]
macro_rules! scoped_log {
    ($level:ident, $scope:literal, $fmt:literal $(, $args:expr)* $(,)?) => {
        log::$level!(concat!("[", $scope, "] ", $fmt) $(, $args)*)
    };
}

const MOUNT_WORKSPACE_ROOTS: [&str; 2] = ["/mnt", "/debug_ramdisk"];

fn random_workspace_name() -> String {
    let mut name = String::from("hm_");
    for _ in 0..10 {
        name.push(fastrand::alphanumeric());
    }
    name
}

fn is_workspace_name(name: &str) -> bool {
    let Some(suffix) = name.strip_prefix("hm_") else {
        return false;
    };
    suffix.len() == 10 && suffix.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

fn create_mount_workspace_in(roots: &[PathBuf]) -> Result<PathBuf> {
    let mut failures = Vec::new();

    for root in roots {
        let mut exhausted_names = true;
        for _ in 0..100 {
            let path = root.join(random_workspace_name());
            match fs::create_dir(&path) {
                Ok(()) => return Ok(path),
                Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    failures.push(format!("{}: {error}", root.display()));
                    exhausted_names = false;
                    break;
                }
            }
        }

        if exhausted_names {
            failures.push(format!(
                "{}: exhausted unique workspace names",
                root.display()
            ));
        }
    }

    bail!(
        "failed to create a mount workspace under any candidate root: {}",
        failures.join("; ")
    )
}

pub fn create_mount_workspace() -> Result<PathBuf> {
    let roots = MOUNT_WORKSPACE_ROOTS.map(PathBuf::from);
    create_mount_workspace_in(&roots)
}

pub fn is_mount_workspace_path(path: &Path) -> bool {
    MOUNT_WORKSPACE_ROOTS.iter().any(|root| {
        let Ok(relative) = path.strip_prefix(root) else {
            return false;
        };
        let mut components = relative.components();
        let Some(std::path::Component::Normal(name)) = components.next() else {
            return false;
        };
        name.to_str().is_some_and(is_workspace_name)
            && components.all(|component| matches!(component, std::path::Component::Normal(_)))
    })
}

pub fn init_logging() -> Result<()> {
    static LOGGER_INIT: OnceLock<()> = OnceLock::new();
    if LOGGER_INIT.get().is_some() {
        return Ok(());
    }

    let requested_level = std::env::var(LOG_LEVEL_ENV).ok();
    let level = requested_level
        .as_deref()
        .and_then(parse_log_level)
        .unwrap_or(DEFAULT_LOG_LEVEL);

    #[cfg(target_os = "android")]
    {
        android_logger::init_once(
            android_logger::Config::default()
                .with_max_level(level)
                .with_tag("Hybrid_Logger"),
        );
        LOGGER_INIT
            .set(())
            .map_err(|_| anyhow::anyhow!("logger initialization raced"))?;
    }

    #[cfg(not(target_os = "android"))]
    {
        use std::io::Write;

        let mut builder = env_logger::Builder::new();
        builder.format(|buf, record| {
            writeln!(
                buf,
                "[{}] [{}] {}",
                record.level(),
                record.target(),
                record.args()
            )
        });
        builder
            .filter_level(level)
            .try_init()
            .context("failed to initialize logger")?;
        LOGGER_INIT
            .set(())
            .map_err(|_| anyhow::anyhow!("logger initialization raced"))?;
    }

    if let Some(requested) = requested_level
        && parse_log_level(&requested).is_none()
    {
        crate::scoped_log!(
            warn,
            "logging",
            "invalid_level: variable={}, value={:?}, fallback={}",
            LOG_LEVEL_ENV,
            requested,
            DEFAULT_LOG_LEVEL
        );
    }
    crate::scoped_log!(info, "logging", "initialized: level={}", level);
    Ok(())
}

fn parse_log_level(value: &str) -> Option<log::LevelFilter> {
    match value.trim().to_ascii_lowercase().as_str() {
        "off" => Some(log::LevelFilter::Off),
        "error" => Some(log::LevelFilter::Error),
        "warn" => Some(log::LevelFilter::Warn),
        "info" => Some(log::LevelFilter::Info),
        "debug" => Some(log::LevelFilter::Debug),
        "trace" => Some(log::LevelFilter::Trace),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use super::{create_mount_workspace_in, is_mount_workspace_path, parse_log_level};

    #[test]
    fn generated_mount_path_uses_hybrid_prefix() {
        let temp = tempfile::tempdir().unwrap();
        let path = create_mount_workspace_in(&[temp.path().to_path_buf()]).unwrap();
        let name = path.file_name().and_then(|name| name.to_str()).unwrap();

        assert_eq!(path.parent(), Some(temp.path()));
        assert!(name.starts_with("hm_"));
        assert_eq!(name.len(), 13);
        assert!(path.is_dir());
    }

    #[test]
    fn workspace_creation_falls_back_to_the_next_root() {
        let temp = tempfile::tempdir().unwrap();
        let unusable = temp.path().join("not-a-directory");
        fs::write(&unusable, b"file").unwrap();
        let fallback = temp.path().join("fallback");
        fs::create_dir(&fallback).unwrap();

        let path = create_mount_workspace_in(&[unusable, fallback.clone()]).unwrap();

        assert_eq!(path.parent(), Some(fallback.as_path()));
    }

    #[test]
    fn workspace_path_match_is_exact_and_supports_fallback_root() {
        assert!(is_mount_workspace_path(Path::new("/mnt/hm_a1B2c3D4e5")));
        assert!(is_mount_workspace_path(Path::new(
            "/debug_ramdisk/hm_a1B2c3D4e5/child"
        )));
        assert!(!is_mount_workspace_path(Path::new("/mnt/hm_too_short")));
        assert!(!is_mount_workspace_path(Path::new(
            "/data/mnt/hm_a1B2c3D4e5"
        )));
    }

    #[test]
    fn log_level_parser_is_case_insensitive_and_rejects_directives() {
        assert_eq!(parse_log_level(" INFO "), Some(log::LevelFilter::Info));
        assert_eq!(parse_log_level("trace"), Some(log::LevelFilter::Trace));
        assert_eq!(parse_log_level("crate=debug"), None);
    }
}
