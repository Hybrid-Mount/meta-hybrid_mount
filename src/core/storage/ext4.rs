// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    collections::HashSet, fs, io::ErrorKind, os::unix::fs::MetadataExt, path::Path,
    process::Command,
};

use anyhow::{Context, Result, bail, ensure};
use jwalk::WalkDir;
use rustix::mount::{UnmountFlags, unmount as umount};

use crate::{
    core::storage::backends::Ext4Backend,
    mount::overlayfs::utils as overlay_utils,
    sys::{
        fs::{ensure_dir_exists, lsetfilecon},
        nuke,
    },
};

const DEFAULT_SELINUX_CONTEXT: &str = "u:object_r:system_file:s0";

pub(super) fn setup_ext4_image(
    target: &Path,
    img_path: &Path,
    moduledir: &Path,
) -> Result<Ext4Backend> {
    crate::scoped_log!(trace, "storage:ext4", "backend select: mode=ext4");
    let total_size = calculate_total_size(moduledir)?;
    let min_size = 64 * 1024 * 1024;
    let grow_size = std::cmp::max((total_size as f64 * 1.2) as u64, min_size);

    fs::File::create(img_path)?.set_len(grow_size)?;
    format_ext4_image(img_path)?;
    check_image(img_path)?;
    let _ = lsetfilecon(img_path, "u:object_r:ksu_file:s0");
    ensure_dir_exists(target)?;

    mount_ext4_with_repair(img_path, target)?;
    reset_mount_state(target)?;
    relabel_mount_tree(target);

    Ok(Ext4Backend::new(target))
}

fn calculate_total_size(path: &Path) -> Result<u64> {
    let mut total_size = 0;
    let mut visited_node_map = HashSet::new();
    let mut stack = vec![path.to_path_buf()];

    while let Some(current) = stack.pop() {
        let metadata = match fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(err) if err.raw_os_error() == Some(libc::ELOOP) => {
                crate::scoped_log!(
                    warn,
                    "storage:ext4",
                    "size skip: path={}, reason=symlink_loop, error={}",
                    current.display(),
                    err
                );
                continue;
            }
            Err(err) if err.kind() == ErrorKind::NotFound => {
                crate::scoped_log!(
                    debug,
                    "storage:ext4",
                    "size skip: path={}, reason=not_found",
                    current.display()
                );
                continue;
            }
            Err(err) => return Err(err.into()),
        };

        let file_type = metadata.file_type();
        if file_type.is_file() {
            let dev = metadata.dev();
            let ino = metadata.ino();

            if !visited_node_map.insert((dev, ino)) {
                continue;
            }

            total_size += metadata.blocks() * 512;
        } else if file_type.is_dir() {
            match current.read_dir() {
                Ok(entries) => {
                    for entry in entries.flatten() {
                        stack.push(entry.path());
                    }
                }
                Err(_) => {
                    crate::scoped_log!(
                        error,
                        "storage:ext4",
                        "read dir failed: path={}",
                        current.display()
                    )
                }
            }
        } else if file_type.is_symlink() {
            crate::scoped_log!(
                debug,
                "storage:ext4",
                "size skip: path={}, reason=symlink",
                current.display()
            );
        }
    }
    Ok(total_size)
}

fn format_ext4_image(img_path: &Path) -> Result<()> {
    let result = Command::new("mkfs.ext4")
        .arg("-b")
        .arg("1024")
        .arg("-i")
        .arg("4096")
        .arg(img_path)
        .stdout(std::process::Stdio::piped())
        .output()?;

    ensure!(result.status.success(), "Failed to format ext4 image");
    Ok(())
}

fn check_image(img_path: &Path) -> Result<()> {
    let path_str = img_path.to_str().context("Invalid path string")?;
    let status = Command::new("e2fsck")
        .args(["-yf", path_str])
        .status()
        .with_context(|| format!("Failed to exec e2fsck {}", img_path.display()))?;

    let code = status
        .code()
        .context("e2fsck exited without an exit code (terminated by signal)")?;

    ensure!(
        (0..=3).contains(&code),
        "e2fsck failed for {} with exit code {}",
        img_path.display(),
        code
    );
    Ok(())
}

fn mount_ext4_with_repair(img_path: &Path, target: &Path) -> Result<()> {
    if overlay_utils::mount_ext4(img_path, target).is_err() {
        if crate::sys::mount::repair_image(img_path).is_ok() {
            overlay_utils::mount_ext4(img_path, target)?;
        } else {
            bail!("Failed to repair modules.img");
        }
    }
    Ok(())
}

fn reset_mount_state(target: &Path) -> Result<()> {
    if nuke::nuke_path(target).is_err() {
        umount(target, UnmountFlags::DETACH)?;
    }
    Ok(())
}

fn relabel_mount_tree(target: &Path) {
    for dir_entry in WalkDir::new(target).parallelism(jwalk::Parallelism::Serial) {
        if let Some(path) = dir_entry.ok().map(|dir_entry| dir_entry.path()) {
            let _ = lsetfilecon(&path, DEFAULT_SELINUX_CONTEXT);
        }
    }
}
