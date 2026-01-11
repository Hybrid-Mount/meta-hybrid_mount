// Copyright 2025 Meta-Hybrid Mount Authors
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    ffi::CString,
    fs::File,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use procfs::process::Process;
use rustix::{fd::AsFd, fs::CWD, mount::*};
use tracing::{info, warn};

use crate::defs::KSU_OVERLAY_SOURCE;
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::try_umount::send_unmountable;

pub fn mount_overlayfs(
    lower_dirs: &[String],
    lowest: &str,
    upperdir: Option<PathBuf>,
    workdir: Option<PathBuf>,
    dest: impl AsRef<Path>,
    #[cfg(any(target_os = "linux", target_os = "android"))] _disable_umount: bool,
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

    if lowerdir_config.len() > 4096 {
        warn!(
            "OverlayFS lowerdir string is very long ({} chars). This might exceed kernel limits and fail.",
            lowerdir_config.len()
        );
    }

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

    Ok(())
}

fn bind_mount_at(
    dfd: impl AsFd,
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
        dfd,
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

#[allow(dead_code)]
pub fn bind_mount(
    from: impl AsRef<Path>,
    to: impl AsRef<Path>,
    #[cfg(any(target_os = "linux", target_os = "android"))] disable_umount: bool,
) -> Result<()> {
    bind_mount_at(
        CWD,
        from,
        to,
        #[cfg(any(target_os = "linux", target_os = "android"))]
        disable_umount,
    )
}

pub fn mount_overlay(
    root: &str,
    module_roots: &[String],
    workdir: Option<PathBuf>,
    upperdir: Option<PathBuf>,
    #[cfg(any(target_os = "linux", target_os = "android"))] disable_umount: bool,
) -> Result<()> {
    info!("mount overlay for {root}");

    if !Path::new(root).exists() {
        warn!("Target root {} does not exist, skipping.", root);
        return Ok(());
    }

    let root_fd = File::open(root).with_context(|| format!("failed to open root {root}"))?;

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

    let mut failed_restores = Vec::new();

    // Handle child mounts (nested mounts)
    for mount_point in mount_seq.iter() {
        let Some(mount_point) = mount_point else {
            continue;
        };

        let mp_path = Path::new(mount_point);
        let relative_path = mp_path.strip_prefix(root).unwrap_or(mp_path);

        // Use bind mount to restore visibility of child mounts
        if let Err(e) = bind_mount_at(
            &root_fd,
            relative_path,
            mp_path,
            #[cfg(any(target_os = "linux", target_os = "android"))]
            disable_umount,
        ) {
            warn!("failed to restore child mount {mount_point}: {e:#}");
            failed_restores.push(mount_point);
        }
    }

    if !failed_restores.is_empty() {
        warn!(
            "Critical: Failed to restore {} child mounts. Rolling back overlay.",
            failed_restores.len()
        );

        if let Err(e) = umount_dir(root) {
            warn!("Rollback failed for {}: {}", root, e);
        }

        bail!("Failed to restore child mounts, overlay rolled back.");
    }

    Ok(())
}

#[allow(dead_code)]
pub fn umount_dir(src: impl AsRef<Path>) -> Result<()> {
    unmount(src.as_ref(), UnmountFlags::DETACH)
        .with_context(|| format!("Failed to umount {}", src.as_ref().display()))?;
    Ok(())
}
