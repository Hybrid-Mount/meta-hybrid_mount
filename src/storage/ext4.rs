// ReHybrid-Mount
//
// SPDX-License-Identifier: GPL-3.0-only

//! ext4 loop 镜像 staging(行为对齐 v4.2.0):
//! 按源目录占用估算镜像大小(硬链接去重、符号链接跳过),
//! `mkfs.ext4 -b 1024 -i 4096` 格式化,`e2fsck -yf` 校验(0..=3 为成功),
//! loop 挂载失败时修复重试,成功后 nuke 清空内容。
//!
//! Stage 3 脚手架:入口在 Stage 5 CLI 接入前暂未被二进制入口使用;
//! 接入完成后移除本豁免,恢复 dead_code 检查。
#![allow(dead_code)]

use std::collections::HashSet;

#[cfg(unix)]
use std::{
    fs,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    process::Command,
};

#[cfg(unix)]
use crate::errors::{Error, Result};
#[cfg(unix)]
use crate::overlayfs::utils as overlay_utils;
#[cfg(unix)]
use crate::storage::{StorageHandle, StorageMode};
#[cfg(unix)]
use crate::sys;

const EXT4_MIN_IMAGE_SIZE_BYTES: u64 = 64 * 1024 * 1024;
const EXT4_GROWTH_FACTOR: f64 = 1.2;
const STAT_BLOCK_SIZE_BYTES: u64 = 512;
const MODULES_IMG_SELINUX_CONTEXT: &str = "u:object_r:ksu_file:s0";
const MKFS_EXT4_BLOCK_SIZE: &str = "1024";
const MKFS_EXT4_BYTES_PER_INODE: &str = "4096";
const E2FSCK_SUCCESS_MAX_EXIT_CODE: i32 = 3;

/// 文件大小统计器:`(dev, ino)` 去重避免硬链接重复计数。
#[derive(Debug, Default)]
pub struct SizeCounter {
    total: u64,
    visited: HashSet<(u64, u64)>,
}

impl SizeCounter {
    /// 记录一个文件块数;重复 inode 返回 `false`。
    pub fn add_file(&mut self, dev: u64, ino: u64, blocks: u64) -> bool {
        if !self.visited.insert((dev, ino)) {
            return false;
        }
        self.total = self
            .total
            .saturating_add(blocks.saturating_mul(STAT_BLOCK_SIZE_BYTES));
        true
    }

    pub const fn total(&self) -> u64 {
        self.total
    }
}

/// 镜像容量计划:源占用 × 1.2,且不小于 64 MiB(v4.2.0 行为)。
pub fn planned_image_size(total_size: u64) -> u64 {
    let grown = (total_size as f64 * EXT4_GROWTH_FACTOR) as u64;
    grown.max(EXT4_MIN_IMAGE_SIZE_BYTES)
}

#[cfg(unix)]
pub(super) fn setup_ext4_image(
    target: &Path,
    img_path: &Path,
    source_paths: &[PathBuf],
) -> Result<StorageHandle> {
    log::info!("storage backend select: mode=ext4");

    let total_size = calculate_total_size(source_paths)?;
    let image_size = planned_image_size(total_size);

    fs::File::create(img_path)?.set_len(image_size)?;
    format_ext4_image(img_path)?;
    check_image(img_path)?;

    if let Err(err) = crate::utils::lsetfilecon(img_path, MODULES_IMG_SELINUX_CONTEXT) {
        log::warn!(
            "selinux context set failed: path={}, error={err}",
            img_path.display()
        );
    }

    crate::utils::ensure_dir_exists(target)?;
    mount_ext4_with_repair(img_path, target)?;
    reset_mount_state(target);

    Ok(StorageHandle::new(target, StorageMode::Ext4))
}

#[cfg(unix)]
fn calculate_total_size(paths: &[PathBuf]) -> Result<u64> {
    let mut counter = SizeCounter::default();
    let mut stack: Vec<PathBuf> = paths.iter().filter(|path| path.exists()).cloned().collect();

    while let Some(current) = stack.pop() {
        let metadata = match fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(err) if err.raw_os_error() == Some(libc::ELOOP) => {
                log::warn!("size scan symlink loop: path={}", current.display());
                continue;
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                log::debug!("size skip: path={}, reason=not_found", current.display());
                continue;
            }
            Err(err) => return Err(err.into()),
        };

        let file_type = metadata.file_type();
        if file_type.is_file() {
            counter.add_file(metadata.dev(), metadata.ino(), metadata.blocks());
        } else if file_type.is_dir() {
            if let Ok(entries) = current.read_dir() {
                for entry in entries.flatten() {
                    stack.push(entry.path());
                }
            } else {
                log::error!("read dir failed: path={}", current.display());
            }
        } else if file_type.is_symlink() {
            log::debug!("size skip: path={}, reason=symlink", current.display());
        }
    }

    Ok(counter.total())
}

#[cfg(unix)]
fn format_ext4_image(img_path: &Path) -> Result<()> {
    let output = Command::new("mkfs.ext4")
        .arg("-b")
        .arg(MKFS_EXT4_BLOCK_SIZE)
        .arg("-i")
        .arg(MKFS_EXT4_BYTES_PER_INODE)
        .arg(img_path)
        .output()
        .map_err(|err| Error::msg(format!("execute mkfs.ext4: {err}")))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(Error::msg(format!(
            "mkfs.ext4 failed for {}: {}",
            img_path.display(),
            String::from_utf8_lossy(&output.stderr)
        )))
    }
}

#[cfg(unix)]
fn check_image(img_path: &Path) -> Result<()> {
    let status = Command::new("e2fsck")
        .args(["-yf"])
        .arg(img_path)
        .status()
        .map_err(|err| Error::msg(format!("execute e2fsck {}: {err}", img_path.display())))?;

    let code = status.code().ok_or_else(|| {
        Error::msg(format!(
            "e2fsck exited without code: {}",
            img_path.display()
        ))
    })?;

    if code <= E2FSCK_SUCCESS_MAX_EXIT_CODE {
        Ok(())
    } else {
        Err(Error::msg(format!(
            "e2fsck failed for {} with exit code {code}",
            img_path.display()
        )))
    }
}

#[cfg(unix)]
fn mount_ext4_with_repair(img_path: &Path, target: &Path) -> Result<()> {
    if overlay_utils::mount_ext4(img_path, target).is_ok() {
        return Ok(());
    }

    sys::mount::repair_image(img_path)?;
    overlay_utils::mount_ext4(img_path, target)
}

#[cfg(unix)]
fn reset_mount_state(target: &Path) {
    sys::nuke::nuke_path(target);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planned_image_size_enforces_minimum() {
        assert_eq!(planned_image_size(0), EXT4_MIN_IMAGE_SIZE_BYTES);
        assert_eq!(planned_image_size(1_000), EXT4_MIN_IMAGE_SIZE_BYTES);
    }

    #[test]
    fn planned_image_size_grows_with_source() {
        let total = 100 * 1024 * 1024;
        assert_eq!(planned_image_size(total), (total as f64 * 1.2) as u64);
    }

    #[test]
    fn size_counter_deduplicates_hardlinks() {
        let mut counter = SizeCounter::default();

        assert!(counter.add_file(1, 10, 8));
        assert!(!counter.add_file(1, 10, 8));
        assert!(counter.add_file(1, 11, 2));

        assert_eq!(counter.total(), (8 + 2) * STAT_BLOCK_SIZE_BYTES);
    }

    #[test]
    fn size_counter_saturates_instead_of_overflowing() {
        let mut counter = SizeCounter::default();
        assert!(counter.add_file(1, 10, u64::MAX));
        assert_eq!(counter.total(), u64::MAX);
    }

    #[test]
    fn size_counter_counts_distinct_devices() {
        let mut counter = SizeCounter::default();
        assert!(counter.add_file(1, 10, 1));
        assert!(counter.add_file(2, 10, 1));
        assert_eq!(counter.total(), 2 * STAT_BLOCK_SIZE_BYTES);
    }
}
