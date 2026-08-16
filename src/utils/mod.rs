// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

pub mod path;
pub mod sync;
pub mod validation;

use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

use anyhow::Result;

pub use self::{path::*, sync::*, validation::*};
#[macro_export]
macro_rules! scoped_log {
    ($level:ident, $scope:literal, $fmt:literal $(, $args:expr)* $(,)?) => {
        log::$level!(concat!("[", $scope, "] ", $fmt) $(, $args)*)
    };
}

pub fn get_mnt() -> PathBuf {
    for _ in 0..100 {
        let mut name = String::from("hm_");
        for _ in 0..10 {
            name.push(fastrand::alphanumeric());
        }
        let path = Path::new("/mnt").join(name);
        if !path.exists() {
            return path;
        }
    }
    Path::new("/mnt").join(format!("hm_mnt_{}", std::process::id()))
}

pub fn init_logging() -> Result<()> {
    static LOGGER_INIT: OnceLock<()> = OnceLock::new();
    if LOGGER_INIT.get().is_some() {
        return Ok(());
    }

    #[cfg(target_os = "android")]
    {
        android_logger::init_once(
            android_logger::Config::default()
                .with_max_level(log::LevelFilter::Trace)
                .with_tag("Hybrid_Logger"),
        );
        LOGGER_INIT.set(()).ok();
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
            .filter_level(log::LevelFilter::Trace)
            .try_init()
            .ok();
        LOGGER_INIT.set(()).ok();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::get_mnt;

    #[test]
    fn generated_mount_path_uses_hybrid_prefix() {
        let path = get_mnt();
        let name = path.file_name().and_then(|name| name.to_str()).unwrap();

        assert_eq!(
            path.parent().and_then(|parent| parent.to_str()),
            Some("/mnt")
        );
        assert!(name.starts_with("hm_"));
    }
}
