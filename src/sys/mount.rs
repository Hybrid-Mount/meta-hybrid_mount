// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::path::Path;

#[cfg(any(target_os = "linux", target_os = "android"))]
use anyhow::Context;
use anyhow::Result;
#[cfg(not(any(target_os = "linux", target_os = "android")))]
use anyhow::bail;
#[cfg(any(target_os = "linux", target_os = "android"))]
use procfs::process::Process;
#[cfg(any(target_os = "linux", target_os = "android"))]
use rustix::mount::{MountFlags, mount};

#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::sys::fs::ensure_dir_exists;

pub fn detect_mount_source() -> String {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        if ksu::version().is_some() {
            crate::scoped_log!(debug, "sys:mount_source", "complete: source=KSU");
            return "KSU".to_string();
        }
    }
    crate::scoped_log!(debug, "sys:mount_source", "complete: source=APatch");
    "APatch".to_string()
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn is_mounted<P: AsRef<Path>>(path: P) -> Result<bool> {
    let path_str = path
        .as_ref()
        .to_str()
        .context("mount path is not valid UTF-8")?;
    let search = if path_str == "/" {
        "/"
    } else {
        path_str.trim_end_matches('/')
    };
    let mountinfo = Process::myself()?.mountinfo()?;
    Ok(mountinfo
        .into_iter()
        .any(|m| m.mount_point.to_string_lossy() == search))
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[allow(dead_code)]
pub fn mount_tmpfs(target: &Path, source: &str) -> Result<()> {
    crate::scoped_log!(
        info,
        "sys:mount_tmpfs",
        "start: source={}, target={}",
        source,
        target.display()
    );
    ensure_dir_exists(target)?;
    mount(
        source,
        target,
        c"tmpfs",
        MountFlags::empty(),
        Some(c"mode=0755"),
    )
    .context("Failed to mount tmpfs")?;
    crate::scoped_log!(
        info,
        "sys:mount_tmpfs",
        "complete: source={}, target={}",
        source,
        target.display()
    );
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
#[allow(dead_code)]
pub fn mount_tmpfs(_target: &Path, _source: &str) -> Result<()> {
    bail!("tmpfs mounting is only supported on linux/android")
}
