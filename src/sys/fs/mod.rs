// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

pub mod file;
pub mod xattr;

pub use file::*;
pub use xattr::*;

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn check_kernel_config(key: &str) -> anyhow::Result<bool> {
    use std::{fs, io::Read};

    use anyhow::Context;
    use flate2::read::GzDecoder;

    let file =
        fs::File::open("/proc/config.gz").with_context(|| "failed to open /proc/config.gz")?;
    let mut config = String::new();
    GzDecoder::new(file)
        .read_to_string(&mut config)
        .with_context(|| "failed to decompress /proc/config.gz")?;

    let found = config.lines().any(|line| {
        if line.starts_with('#') {
            return false;
        }
        let Some((k, v)) = line.split_once('=') else {
            return false;
        };
        k.trim() == key && v.trim() == "y"
    });
    Ok(found)
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn check_kernel_config(_key: &str) -> anyhow::Result<bool> {
    Ok(false)
}
