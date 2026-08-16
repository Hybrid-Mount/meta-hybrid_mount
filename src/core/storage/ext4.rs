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
const EXT4_GROWTH_DENOMINATOR: u64 = 5;
const STAT_BLOCK_SIZE_BYTES: u64 = 512;
const MODULES_IMG_SELINUX_CONTEXT: &str = "u:object_r:ksu_file:s0";
const MKFS_EXT4_BLOCK_SIZE: &str = "1024";
const MKFS_EXT4_BYTES_PER_INODE: u64 = 4096;
const E2FSCK_SUCCESS_MAX_EXIT_CODE: i32 = 3;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct SourceUsage {
    data_bytes: u64,
    inode_count: u64,
}

pub(super) fn setup_ext4_image(
    target: &Path,
    img_path: &Path,
    source_paths: &[PathBuf],
) -> Result<StorageHandle> {
    crate::scoped_log!(trace, "storage:ext4", "backend select: mode=ext4");
    let usage = calculate_source_usage(source_paths)?;
    let image_size = required_image_size(usage);

    fs::File::create(img_path)?.set_len(image_size)?;
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

fn calculate_source_usage(paths: &[PathBuf]) -> Result<SourceUsage> {
    let mut usage = SourceUsage::default();
    let mut visited_node_map = HashSet::new();
    let mut stack: Vec<PathBuf> = paths.to_vec();

    while let Some(current) = stack.pop() {
        let metadata = fs::symlink_metadata(&current)
            .with_context(|| format!("failed to inspect source path {}", current.display()))?;
        usage.inode_count = usage.inode_count.saturating_add(1);

        let file_type = metadata.file_type();
        if file_type.is_file() {
            let dev = metadata.dev();
            let ino = metadata.ino();

            if !visited_node_map.insert((dev, ino)) {
                continue;
            }

            usage.data_bytes = usage
                .data_bytes
                .saturating_add(metadata.blocks().saturating_mul(STAT_BLOCK_SIZE_BYTES));
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
    Ok(usage)
}

fn required_image_size(usage: SourceUsage) -> u64 {
    let inode_bytes = usage.inode_count.saturating_mul(MKFS_EXT4_BYTES_PER_INODE);
    let required = usage.data_bytes.max(inode_bytes);
    let growth = required.saturating_add(EXT4_GROWTH_DENOMINATOR - 1) / EXT4_GROWTH_DENOMINATOR;

    EXT4_MIN_IMAGE_SIZE_BYTES.max(required.saturating_add(growth))
}

fn format_ext4_image(img_path: &Path) -> Result<()> {
    let result = Command::new("mkfs.ext4")
        .arg("-b")
        .arg(MKFS_EXT4_BLOCK_SIZE)
        .arg("-i")
        .arg(MKFS_EXT4_BYTES_PER_INODE.to_string())
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

#[cfg(test)]
mod tests {
    use std::os::unix::fs::symlink;

    use super::*;

    #[test]
    fn source_usage_counts_directories_files_and_symlinks() {
        let temp = tempfile::tempdir().unwrap();
        let nested = temp.path().join("nested");
        fs::create_dir(&nested).unwrap();
        fs::write(nested.join("file"), b"content").unwrap();
        symlink("file", nested.join("link")).unwrap();

        let usage = calculate_source_usage(&[temp.path().to_path_buf()]).unwrap();

        assert_eq!(usage.inode_count, 4);
        assert!(usage.data_bytes > 0);
    }

    #[test]
    fn inode_demand_can_grow_the_image_beyond_the_minimum() {
        let inode_count = EXT4_MIN_IMAGE_SIZE_BYTES / MKFS_EXT4_BYTES_PER_INODE + 1;
        let usage = SourceUsage {
            data_bytes: 0,
            inode_count,
        };

        let size = required_image_size(usage);

        assert!(size > EXT4_MIN_IMAGE_SIZE_BYTES);
        assert!(size >= inode_count * MKFS_EXT4_BYTES_PER_INODE);
    }

    #[test]
    fn data_demand_still_controls_image_size_for_large_files() {
        let usage = SourceUsage {
            data_bytes: EXT4_MIN_IMAGE_SIZE_BYTES * 2,
            inode_count: 1,
        };

        assert!(required_image_size(usage) > usage.data_bytes);
    }
}
