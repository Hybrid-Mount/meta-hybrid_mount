// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

pub mod path;
mod timing;
pub mod validation;

use std::{
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

pub fn get_mnt() -> Result<PathBuf> {
    for _ in 0..100 {
        let mut name = String::from("hm_");
        for _ in 0..10 {
            name.push(fastrand::alphanumeric());
        }
        let path = Path::new("/mnt").join(name);
        if !path.exists() {
            return Ok(path);
        }
    }
    bail!("failed to allocate a unique mount path under /mnt")
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
    use super::{get_mnt, parse_log_level};

    #[test]
    fn generated_mount_path_uses_hybrid_prefix() {
        let path = get_mnt().unwrap();
        let name = path.file_name().and_then(|name| name.to_str()).unwrap();

        assert_eq!(
            path.parent().and_then(|parent| parent.to_str()),
            Some("/mnt")
        );
        assert!(name.starts_with("hm_"));
    }

    #[test]
    fn log_level_parser_is_case_insensitive_and_rejects_directives() {
        assert_eq!(parse_log_level(" INFO "), Some(log::LevelFilter::Info));
        assert_eq!(parse_log_level("trace"), Some(log::LevelFilter::Trace));
        assert_eq!(parse_log_level("crate=debug"), None);
    }
}
