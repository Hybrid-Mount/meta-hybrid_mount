// Copyright 2025 Meta-Hybrid Mount Authors
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::{Context, Result, bail};
use log::{info, warn};
use std::{
    ffi::CString,
    fs,
    io::{BufRead, BufReader},
    os::fd::{AsRawFd, OwnedFd},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use rustix::{
    fd::AsFd,
    fs::{CWD, XattrFlags, getxattr, setxattr},
    mount::*,
};

use crate::defs::{KSU_OVERLAY_SOURCE, RUN_DIR};
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::try_umount::send_unmountable;

const PAGE_LIMIT: usize = 4000;

const MS_MOVE: u32 = 8192;

enum StashedMount {
    Modern(OwnedFd),
    Legacy(PathBuf),
}

struct StagedMountGuard {
    mounts: Vec<PathBuf>,
    committed: bool,
}

impl Drop for StagedMountGuard {
    fn drop(&mut self) {
        if !self.committed {
            for path in self.mounts.iter().rev() {
                let _ = unmount(path, UnmountFlags::DETACH);

                let _ = fs::remove_dir(path);
            }
        }
    }
}

fn umount_dir(src: impl AsRef<Path>) -> Result<()> {
    unmount(src.as_ref(), UnmountFlags::DETACH)
        .with_context(|| format!("Failed to umount {}", src.as_ref().display()))?;

    Ok(())
}

fn get_overlay_features() -> String {
    let mut features = String::new();

    if Path::new("/sys/module/overlay/parameters/redirect_dir").exists() {
        features.push_str(",redirect_dir=on");
    }

    if Path::new("/sys/module/overlay/parameters/metacopy").exists() {
        if !features.contains("redirect_dir") {
            features.push_str(",redirect_dir=on");
        }

        features.push_str(",metacopy=on");
    }

    features
}

fn get_sub_mounts(parent: &str) -> Result<Vec<String>> {
    let file = fs::File::open("/proc/mounts").context("Failed to open /proc/mounts")?;

    let reader = BufReader::new(file);

    let mut sub_mounts = Vec::new();

    let parent_prefix = if parent.ends_with('/') {
        parent.to_string()
    } else {
        format!("{}/", parent)
    };

    for line in reader.lines() {
        let line = line?;

        let parts: Vec<&str> = line.split_whitespace().collect();

        if parts.len() < 2 {
            continue;
        }

        let mount_point = parts[1];

        if mount_point.starts_with(&parent_prefix)
            && mount_point != parent
            && !mount_point.contains("hybrid_mount")
        {
            sub_mounts.push(mount_point.to_string());
        }
    }

    sub_mounts.sort_by_key(|a| a.len());

    Ok(sub_mounts)
}

fn clone_path_context(source: &Path, target: &Path) -> Result<()> {
    let mut buf = vec![0u8; 256];

    let name = "security.selinux";

    match getxattr(source, name, &mut buf) {
        Ok(len) => {
            let _ = setxattr(target, name, &buf[..len], XattrFlags::empty());
        }
        Err(rustix::io::Errno::RANGE) => {
            let mut large_buf = vec![0u8; 1024];

            if let Ok(len) = getxattr(source, name, &mut large_buf) {
                let _ = setxattr(target, name, &large_buf[..len], XattrFlags::empty());
            }
        }
        _ => {}
    }

    Ok(())
}

fn recursive_context_align(
    target_base: &Path,
    module_base: &Path,
    current_module_path: &Path,
) -> Result<()> {
    if current_module_path.is_symlink() {
        return Ok(());
    }

    if current_module_path.is_dir() {
        for entry in fs::read_dir(current_module_path)? {
            let entry = entry?;

            recursive_context_align(target_base, module_base, &entry.path())?;
        }
    } else if let Ok(relative) = current_module_path.strip_prefix(module_base) {
        let target_path = target_base.join(relative);

        if target_path.exists() {
            let _ = clone_path_context(&target_path, current_module_path);
        }
    }

    Ok(())
}

fn align_overlay_contexts(target_root: &str, module_roots: &[String]) {
    let target_path = Path::new(target_root);

    for module_root in module_roots {
        let module_path = Path::new(module_root);

        if module_path.exists() {
            let _ = recursive_context_align(target_path, module_path, module_path);
        }
    }
}

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

    match do_mount_overlay(
        &lowerdir_config,
        upperdir.clone(),
        workdir.clone(),
        dest.as_ref(),
        #[cfg(any(target_os = "linux", target_os = "android"))]
        disable_umount,
    ) {
        Ok(_) => Ok(()),
        Err(e) => {
            if lowerdir_config.len() >= PAGE_LIMIT {
                if upperdir.is_some() || workdir.is_some() {
                    return Err(e);
                }

                info!(
                    "Direct overlay mount failed (possibly due to length limits), switching to staged mount. Error: {}",
                    e
                );

                return mount_overlayfs_staged(
                    lower_dirs,
                    lowest,
                    dest,
                    #[cfg(any(target_os = "linux", target_os = "android"))]
                    disable_umount,
                );
            }

            Err(e)
        }
    }
}

fn mount_overlayfs_staged(
    lower_dirs: &[String],
    lowest: &str,
    dest: impl AsRef<Path>,
    #[cfg(any(target_os = "linux", target_os = "android"))] disable_umount: bool,
) -> Result<()> {
    let mut batches: Vec<Vec<String>> = Vec::new();

    let mut current_batch: Vec<String> = Vec::new();

    let mut current_len = 0;

    const SAFE_CHUNK_SIZE: usize = 3500;

    for dir in lower_dirs {
        if current_len + dir.len() + 1 > SAFE_CHUNK_SIZE {
            batches.push(current_batch);

            current_batch = Vec::new();

            current_len = 0;
        }

        current_batch.push(dir.clone());

        current_len += dir.len() + 1;
    }

    if !current_batch.is_empty() {
        batches.push(current_batch);
    }

    let staging_root = Path::new(RUN_DIR).join("staging");

    if !staging_root.exists() {
        fs::create_dir_all(&staging_root).context("failed to create staging dir")?;
    }

    let mut current_base = lowest.to_string();

    let mut guard = StagedMountGuard {
        mounts: Vec::new(),
        committed: false,
    };

    for (i, batch) in batches.iter().rev().enumerate() {
        let is_last_layer = i == batches.len() - 1;

        let target_path = if is_last_layer {
            dest.as_ref().to_path_buf()
        } else {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .context("System time is before UNIX EPOCH")?
                .as_nanos();

            let stage_dir = staging_root.join(format!("stage_{}_{}", timestamp, i));

            fs::create_dir_all(&stage_dir)
                .with_context(|| format!("Failed to create stage dir {:?}", stage_dir))?;

            stage_dir
        };

        let lowerdir_str = batch
            .iter()
            .map(|s| s.as_str())
            .chain(std::iter::once(current_base.as_str()))
            .collect::<Vec<_>>()
            .join(":");

        do_mount_overlay(
            &lowerdir_str,
            None,
            None,
            &target_path,
            #[cfg(any(target_os = "linux", target_os = "android"))]
            disable_umount,
        )?;

        if !is_last_layer {
            guard.mounts.push(target_path.clone());

            current_base = target_path.to_string_lossy().to_string();
        }
    }

    guard.committed = true;

    Ok(())
}

fn do_mount_overlay(
    lowerdir_config: &str,
    upperdir: Option<PathBuf>,
    workdir: Option<PathBuf>,
    dest: impl AsRef<Path>,
    #[cfg(any(target_os = "linux", target_os = "android"))] disable_umount: bool,
) -> Result<()> {
    let upperdir_s = upperdir
        .filter(|up| up.exists())
        .map(|e| e.display().to_string());

    let workdir_s = workdir
        .filter(|wd| wd.exists())
        .map(|e| e.display().to_string());

    let extra_features = get_overlay_features();

    let result = (|| {
        let fs = fsopen("overlay", FsOpenFlags::FSOPEN_CLOEXEC)?;

        let fs = fs.as_fd();

        fsconfig_set_string(fs, "lowerdir", lowerdir_config)?;

        if let (Some(upperdir), Some(workdir)) = (&upperdir_s, &workdir_s) {
            fsconfig_set_string(fs, "upperdir", upperdir)?;

            fsconfig_set_string(fs, "workdir", workdir)?;
        }

        if extra_features.contains("redirect_dir") {
            let _ = fsconfig_set_string(fs, "redirect_dir", "on");
        }

        if extra_features.contains("metacopy") {
            let _ = fsconfig_set_string(fs, "metacopy", "on");
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

    if let Err(fsopen_err) = result {
        let mut data = format!("lowerdir={lowerdir_config}");

        if let (Some(upperdir), Some(workdir)) = (upperdir_s, workdir_s) {
            data = format!("{data},upperdir={upperdir},workdir={workdir}");
        }

        data.push_str(&extra_features);

        let data_c = CString::new(data).context("Invalid string for mount data")?;

        mount(
            KSU_OVERLAY_SOURCE,
            dest.as_ref(),
            "overlay",
            MountFlags::empty(),
            Some(data_c.as_c_str()),
        )
        .with_context(|| format!("Legacy mount failed (fsopen also failed: {})", fsopen_err))?;
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    if !disable_umount {
        let _ = send_unmountable(dest.as_ref());
    }

    Ok(())
}

fn mount_overlay_child(
    mount_point: &str,
    relative: &str,
    module_roots: &[String],
    stock: StashedMount,
    #[cfg(any(target_os = "linux", target_os = "android"))] disable_umount: bool,
) -> Result<()> {
    let has_modification = module_roots.iter().any(|lower| {
        let path = Path::new(lower).join(relative.trim_start_matches('/'));

        path.exists()
    });

    if !has_modification {
        match stock {
            StashedMount::Modern(fd) => {
                move_mount(
                    fd.as_fd(),
                    "",
                    CWD,
                    mount_point,
                    MoveMountFlags::MOVE_MOUNT_F_EMPTY_PATH,
                )
                .with_context(|| format!("move_mount failed to {}", mount_point))?;
            }
            StashedMount::Legacy(path) => {
                #[allow(clippy::useless_conversion)]
                let flags = MountFlags::from_bits_retain(MS_MOVE.into());

                mount(&path, mount_point, "", flags, None)
                    .with_context(|| format!("legacy move mount failed to {}", mount_point))?;

                let _ = fs::remove_dir(path);
            }
        }

        #[cfg(any(target_os = "linux", target_os = "android"))]
        if !disable_umount {
            let _ = send_unmountable(mount_point);
        }

        return Ok(());
    }

    let mut lower_dirs: Vec<String> = vec![];

    for lower in module_roots {
        let path = Path::new(lower).join(relative.trim_start_matches('/'));

        if path.is_dir() {
            lower_dirs.push(path.display().to_string());
        } else if path.exists() {
            return Ok(());
        }
    }

    if lower_dirs.is_empty() {
        return Ok(());
    }

    let lower_root = match &stock {
        StashedMount::Modern(fd) => format!("/proc/self/fd/{}", fd.as_raw_fd()),
        StashedMount::Legacy(path) => path.to_string_lossy().to_string(),
    };

    if let Err(e) = mount_overlayfs(
        &lower_dirs,
        &lower_root,
        None,
        None,
        mount_point,
        #[cfg(any(target_os = "linux", target_os = "android"))]
        disable_umount,
    ) {
        warn!(
            "failed to overlay child {mount_point}: {:#}, fallback to bind mount",
            e
        );

        match stock {
            StashedMount::Modern(fd) => {
                move_mount(
                    fd.as_fd(),
                    "",
                    CWD,
                    mount_point,
                    MoveMountFlags::MOVE_MOUNT_F_EMPTY_PATH,
                )?;
            }
            StashedMount::Legacy(path) => {
                #[allow(clippy::useless_conversion)]
                let flags = MountFlags::from_bits_retain(MS_MOVE.into());

                mount(&path, mount_point, "", flags, None)?;

                let _ = fs::remove_dir(path);
            }
        }

        #[cfg(any(target_os = "linux", target_os = "android"))]
        if !disable_umount {
            let _ = send_unmountable(mount_point);
        }
    } else if let StashedMount::Legacy(path) = stock {
        let _ = fs::remove_dir(path);
    }

    Ok(())
}

pub fn mount_overlay(
    target_root: &str,
    module_roots: &[String],
    workdir: Option<PathBuf>,
    upperdir: Option<PathBuf>,
    #[cfg(any(target_os = "linux", target_os = "android"))] disable_umount: bool,
) -> Result<()> {
    align_overlay_contexts(target_root, module_roots);

    let root_file = fs::File::open(target_root)
        .with_context(|| format!("failed to open target root {}", target_root))?;

    let stock_root = format!("/proc/self/fd/{}", root_file.as_raw_fd());

    let mut all_child_mounts = Vec::new();

    match get_sub_mounts(target_root) {
        Ok(sub_mounts) => {
            if !sub_mounts.is_empty() {
                info!(
                    "Auto-detected sub-mounts under {}: {:?}",
                    target_root, sub_mounts
                );
            }

            all_child_mounts = sub_mounts;
        }
        Err(e) => {
            warn!("Failed to detect sub-mounts: {}", e);
        }
    }

    let mut stashed_mounts = Vec::new();

    let stash_base = Path::new(RUN_DIR).join("stash");

    for mount_point in &all_child_mounts {
        let relative = mount_point.replacen(target_root, "", 1);

        let relative_clean = relative.trim_start_matches('/');

        match open_tree(
            root_file.as_fd(),
            relative_clean,
            OpenTreeFlags::OPEN_TREE_CLOEXEC
                | OpenTreeFlags::OPEN_TREE_CLONE
                | OpenTreeFlags::AT_RECURSIVE,
        ) {
            Ok(fd) => {
                stashed_mounts.push((mount_point.clone(), relative, StashedMount::Modern(fd)))
            }
            Err(e) => {
                warn!(
                    "open_tree failed for {}: {}, falling back to legacy stash",
                    mount_point, e
                );

                let stash_path = stash_base.join(relative_clean);

                if let Err(err) = fs::create_dir_all(&stash_path) {
                    warn!(
                        "Failed to create stash dir {}: {}",
                        stash_path.display(),
                        err
                    );

                    continue;
                }

                if let Err(err) = mount(
                    mount_point,
                    &stash_path,
                    "",
                    MountFlags::BIND | MountFlags::REC,
                    None,
                ) {
                    warn!("Legacy stash failed for {}: {}", mount_point, err);

                    continue;
                }

                stashed_mounts.push((
                    mount_point.clone(),
                    relative,
                    StashedMount::Legacy(stash_path),
                ));
            }
        }
    }

    mount_overlayfs(
        module_roots,
        &stock_root,
        upperdir,
        workdir,
        target_root,
        #[cfg(any(target_os = "linux", target_os = "android"))]
        disable_umount,
    )
    .with_context(|| format!("mount overlayfs for root {target_root} failed"))?;

    for (mount_point, relative, stock) in stashed_mounts {
        if let Err(e) = mount_overlay_child(
            &mount_point,
            &relative,
            module_roots,
            stock,
            #[cfg(any(target_os = "linux", target_os = "android"))]
            disable_umount,
        ) {
            warn!(
                "failed to restore child mount {mount_point}: {:#}, revert overlay on {}",
                e, target_root
            );

            if let Err(umount_err) = umount_dir(target_root) {
                log::error!(
                    "CRITICAL: Failed to revert overlay on {}: {}",
                    target_root,
                    umount_err
                );
            }

            bail!("Child mount restoration failed: {}", e);
        }
    }

    Ok(())
}
