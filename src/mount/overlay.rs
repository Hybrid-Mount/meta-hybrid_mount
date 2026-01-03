// Copyright 2025 Meta-Hybrid Mount Authors
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    ffi::CString,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use log::{info, warn};
use procfs::process::Process;
use rustix::{fd::AsFd, fs::CWD, mount::*};

use crate::defs::KSU_OVERLAY_SOURCE;
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::try_umount::send_unmountable;

pub fn mount_overlayfs(
    lower_dirs: &[String],
    lowest: &str,
    upperdir: Option<PathBuf>,
    workdir: Option<PathBuf>,
    dest: impl AsRef<Path>,
    #[cfg(any(target_os = "linux", target_os = "android"))] disable_umount: bool,
) -> Result<()> {
    let lowerdir_config = lower_dirs
        .iter()
        .map(|s| s.as_ref())
        .chain(std::iter::once(lowest))
        .collect::<Vec<_>>()
        .join(":");
    info!(
        "mount overlayfs on {:?}, lowerdir={}, upperdir={:?}, workdir={:?}",
        dest.as_ref(),
        lowerdir_config,
        upperdir,
        workdir
    );

    let upperdir_s = upperdir
        .filter(|up| up.exists())
        .map(|e| e.display().to_string());
    let workdir_s = workdir
        .filter(|wd| wd.exists())
        .map(|e| e.display().to_string());

    // Try New API (fsopen)
    let result = (|| {
        let fs = fsopen("overlay", FsOpenFlags::FSOPEN_CLOEXEC)?;
        let fs = fs.as_fd();
        fsconfig_set_string(fs, "lowerdir", &lowerdir_config)?;
        if let (Some(upper), Some(work)) = (&upperdir_s, &workdir_s) {
            fsconfig_set_string(fs, "upperdir", upper)?;
            fsconfig_set_string(fs, "workdir", work)?;
        }
        fsconfig_set_string(fs, "source", KSU_OVERLAY_SOURCE)?;
        fsconfig_create(fs)?;
        let mount = fsmount(fs, FsMountFlags::FSMOUNT_CLOEXEC, MountAttrFlags::empty())?;
        move_mount(
            mount.as_fd(),
            "",
            CWD,
            dest.as_ref(),
            MoveMountFlags::MOVE_MOUNT_F_EMPTY_PATH,
        )
    })();

    // Fallback to Old API (mount)
    if let Err(e) = result {
        warn!("fsopen mount failed: {e:#}, fallback to mount");
        let mut data = format!("lowerdir={lowerdir_config}");
        if let (Some(upper), Some(work)) = (upperdir_s, workdir_s) {
            data = format!("{data},upperdir={upper},workdir={work}");
        }
        let data_c = CString::new(data)?;
        mount(
            KSU_OVERLAY_SOURCE,
            dest.as_ref(),
            "overlay",
            MountFlags::empty(),
            data_c.as_c_str(),
        )?;
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    if !disable_umount {
        let _ = send_unmountable(dest.as_ref());
    }

    Ok(())
}

pub fn bind_mount(
    from: impl AsRef<Path>,
    to: impl AsRef<Path>,
    #[cfg(any(target_os = "linux", target_os = "android"))] disable_umount: bool,
) -> Result<()> {
    info!(
        "bind mount {} -> {}",
        from.as_ref().display(),
        to.as_ref().display()
    );
    let tree = open_tree(
        CWD,
        from.as_ref(),
        OpenTreeFlags::OPEN_TREE_CLOEXEC
            | OpenTreeFlags::OPEN_TREE_CLONE
            | OpenTreeFlags::AT_RECURSIVE,
    )?;
    move_mount(
        tree.as_fd(),
        "",
        CWD,
        to.as_ref(),
        MoveMountFlags::MOVE_MOUNT_F_EMPTY_PATH,
    )?;

    #[cfg(any(target_os = "linux", target_os = "android"))]
    if !disable_umount {
        let _ = send_unmountable(to.as_ref());
    }

    Ok(())
}

pub fn mount_overlay(
    root: &str,
    module_roots: &[String],
    workdir: Option<PathBuf>,
    upperdir: Option<PathBuf>,
    #[cfg(any(target_os = "linux", target_os = "android"))] disable_umount: bool,
) -> Result<()> {
    info!("mount overlay for {root}");

    // Safety check: ensure root exists before chdir
    if !Path::new(root).exists() {
        warn!("Target root {} does not exist, skipping.", root);
        return Ok(());
    }

    std::env::set_current_dir(root).with_context(|| format!("failed to chdir to {root}"))?;
    let stock_root = ".";

    // collect child mounts before mounting the root
    let mounts = Process::myself()?
        .mountinfo()
        .with_context(|| "get mountinfo")?;
    let mut mount_seq = mounts
        .0
        .iter()
        .filter(|m| {
            m.mount_point.starts_with(root) && !Path::new(&root).starts_with(&m.mount_point)
        })
        .map(|m| m.mount_point.to_str())
        .collect::<Vec<_>>();
    mount_seq.sort();
    mount_seq.dedup();

    mount_overlayfs(
        module_roots,
        root,
        upperdir,
        workdir,
        root,
        #[cfg(any(target_os = "linux", target_os = "android"))]
        disable_umount,
    )
    .with_context(|| "mount overlayfs for root failed")?;

    // Handle child mounts (nested mounts)
    for mount_point in mount_seq.iter() {
        let Some(mount_point) = mount_point else {
            continue;
        };
        let relative = mount_point.replacen(root, "", 1);
        let stock_root_child: String = format!("{stock_root}{relative}");
        if !Path::new(&stock_root_child).exists() {
            continue;
        }

        // Use bind mount to restore visibility of child mounts
        if let Err(e) = bind_mount(
            &stock_root_child,
            mount_point,
            #[cfg(any(target_os = "linux", target_os = "android"))]
            disable_umount,
        ) {
            warn!("failed to restore child mount {mount_point}: {e:#}");
        }
    }
    Ok(())
}

#[allow(dead_code)]
pub fn umount_dir(src: impl AsRef<Path>) -> Result<()> {
    unmount(src.as_ref(), UnmountFlags::DETACH)
        .with_context(|| format!("Failed to umount {}", src.as_ref().display()))?;
    Ok(())
}
