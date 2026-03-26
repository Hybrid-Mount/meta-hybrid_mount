// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

pub mod validation;

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::Result;
use log::LevelFilter;

pub use self::validation::*;
use crate::defs;

pub fn get_mnt() -> PathBuf {
    let mut name = String::new();

    for _ in 0..10 {
        name.push(fastrand::alphanumeric());
    }

    let ret = Path::new("/mnt").join(name);
    log::trace!("mnt: {}", ret.display());
    ret
}

pub fn init_logging() -> Result<()> {
    let level = if fs::exists(defs::TRACING).unwrap_or(false) {
        LevelFilter::Trace
    } else {
        LevelFilter::Info
    };

    #[cfg(target_os = "android")]
    {
        android_logger::init_once(
            android_logger::Config::default()
                .with_max_level(level)
                .with_tag("Hybrid_Logger"),
        );
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
        builder.filter_level(level).init();
    }
    Ok(())
}
