// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::{
    collections::HashSet,
    fs,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, ensure};

use crate::{
    core::storage::{StorageHandle, StorageMode},
    mount::overlayfs::utils as overlay_utils,
    sys::fs::{ensure_dir_exists, lsetfilecon},
};

const EXT4_MIN_IMAGE_SIZE_BYTES: u64 = 64 * 1024 * 1024;
const EXT4_GROWTH_FACTOR: f64 = 1.2;
const STAT_BLOCK_SIZE_BYTES: u64 = 512;
const MODULES_IMG_SELINUX_CONTEXT: &str = "u:object_r:ksu_file:s0";
const MKFS_EXT4_BLOCK_SIZE: &str = "1024";
const MKFS_EXT4_BYTES_PER_INODE: &str = "4096";
const E2FSCK_SUCCESS_MAX_EXIT_CODE: i32 = 3;

pub(super) fn setup_ext4_image(
    target: &Path,
    img_path: &Path,
    source_paths: &[PathBuf],
) -> Result<StorageHandle> {
    crate::scoped_log!(trace, "storage:ext4", "backend select: mode=ext4");
    let total_size = calculate_total_size(source_paths)?;
    let min_size = EXT4_MIN_IMAGE_SIZE_BYTES;
    let grow_size = std::cmp::max((total_size as f64 * EXT4_GROWTH_FACTOR) as u64, min_size);

    fs::File::create(img_path)?.set_len(grow_size)?;
    format_ext4_image(img_path)?;
    check_image(img_path)?;
    lsetfilecon(img_path, MODULES_IMG_SELINUX_CONTEXT).with_context(|| {
        format!(
            "failed to set SELinux context on ext4 image {}",
            img_path.display()
        )
    })?;
    ensure_dir_exists(target)?;

    overlay_utils::mount_ext4(img_path, target)?;

    Ok(StorageHandle::new(target, StorageMode::Ext4))
}

fn calculate_total_size(paths: &[PathBuf]) -> Result<u64> {
    let mut total_size = 0;
    let mut visited_node_map = HashSet::new();
    let mut stack: Vec<PathBuf> = paths.to_vec();

    while let Some(current) = stack.pop() {
        let metadata = fs::symlink_metadata(&current)
            .with_context(|| format!("failed to inspect source path {}", current.display()))?;

        let file_type = metadata.file_type();
        if file_type.is_file() {
            let dev = metadata.dev();
            let ino = metadata.ino();

            if !visited_node_map.insert((dev, ino)) {
                continue;
            }

            total_size += metadata.blocks() * STAT_BLOCK_SIZE_BYTES;
        } else if file_type.is_dir() {
            let entries = current.read_dir().with_context(|| {
                format!("failed to read source directory {}", current.display())
            })?;
            for entry in entries {
                stack.push(
                    entry
                        .with_context(|| {
                            format!("failed to enumerate source directory {}", current.display())
                        })?
                        .path(),
                );
            }
        }
    }
    Ok(total_size)
}

fn format_ext4_image(img_path: &Path) -> Result<()> {
    let result = Command::new("mkfs.ext4")
        .arg("-b")
        .arg(MKFS_EXT4_BLOCK_SIZE)
        .arg("-i")
        .arg(MKFS_EXT4_BYTES_PER_INODE)
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
        (0..=E2FSCK_SUCCESS_MAX_EXIT_CODE).contains(&code),
        "e2fsck failed for {} with exit code {}",
        img_path.display(),
        code
    );
    Ok(())
}
