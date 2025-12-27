// Copyright 2025 Meta-Hybrid Mount Authors
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    collections::HashSet,
    ffi::CString,
    path::Path,
    sync::{Mutex, OnceLock},
};

use anyhow::{Context, Result, bail};
use nix::ioctl_write_ptr_bad;

use crate::ksu::{
    get_fd,
    magic::{KSU_IOCTL_ADD_TRY_UMOUNT, KSU_IOCTL_NUKE_EXT4_SYSFS},
};

static SENT_UNMOUNTS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

#[repr(C)]
struct KsuAddTryUmount {
    arg: u64,
    flags: u32,
    mode: u8,
}

#[repr(C)]
struct NukeExt4SysfsCmd {
    arg: u64,
}

ioctl_write_ptr_bad!(
    ksu_add_try_umount,
    KSU_IOCTL_ADD_TRY_UMOUNT,
    KsuAddTryUmount
);

ioctl_write_ptr_bad!(
    ksu_nuke_ext4_sysfs,
    KSU_IOCTL_NUKE_EXT4_SYSFS,
    NukeExt4SysfsCmd
);

pub fn send_unmountable<P>(target: P) -> Result<()>
where
    P: AsRef<Path>,
{
    let path_ref = target.as_ref();
    let path_str = path_ref.to_string_lossy().to_string();

    if path_str.is_empty() {
        return Ok(());
    }

    let cache = SENT_UNMOUNTS.get_or_init(|| Mutex::new(HashSet::new()));
    let mut set = cache.lock().unwrap();

    if set.contains(&path_str) {
        log::debug!("Unmount skipped (dedup): {}", path_str);

        return Ok(());
    }

    set.insert(path_str.clone());

    let path = CString::new(path_str)?;

    let cmd = KsuAddTryUmount {
        arg: path.as_ptr() as u64,
        flags: 2,
        mode: 1,
    };

    let fd = get_fd();

    if fd < 0 {
        return Ok(());
    }

    unsafe {
        ksu_add_try_umount(fd, &cmd)?;
    }

    Ok(())
}

pub fn ksu_nuke_sysfs(target: &str) -> Result<()> {
    let c_path = CString::new(target)?;

    let cmd = NukeExt4SysfsCmd {
        arg: c_path.as_ptr() as u64,
    };

    let fd = get_fd();

    if fd < 0 {
        bail!("KSU driver not available");
    }

    unsafe {
        ksu_nuke_ext4_sysfs(fd, &cmd).context("KSU Nuke Sysfs ioctl failed")?;
    }

    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn ksu_nuke_sysfs(_target: &str) -> Result<()> {
    bail!("Not supported on this OS")
}
