// SPDX-License-Identifier: GPL-3.0-only

//! ext4 loop 镜像 staging(行为对齐 v4.2.0):
//! 按源目录占用估算镜像大小(硬链接去重、符号链接跳过),
//! 使用纯 Rust formatter 创建并审计新镜像，外部 `e2fsck` 仅作挂载失败后的兼容 fallback,
//! loop 挂载失败时修复重试，成功后通过 KernelSU nuke 隐藏 ext4 sysfs 信息。

use std::collections::HashSet;

#[cfg(unix)]
use std::{
    fs,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::Arc,
};

#[cfg(unix)]
use fs_ext4::Filesystem;
#[cfg(unix)]
use fs_ext4::block_io::{BlockDevice, FileDevice};
#[cfg(unix)]
use fs_ext4::mkfs::format_filesystem;

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
// am-fs-ext4 uses 8192 inodes per group. At a 2 KiB block size each group is
// 32 MiB, preserving v4.2.0's `-i 4096` inode density while still using the
// crate's validated multi-group formatter (which requires blocks >= 2 KiB).
const RUST_EXT4_BLOCK_SIZE: u32 = 2048;
const RUST_EXT4_LABEL: &str = "Hybrid-Mount";

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

    log::info!(
        "ext4 image plan: source_bytes={}, image_bytes={}, source_roots={}",
        total_size,
        image_size,
        source_paths.len()
    );

    fs::File::create(img_path)?.set_len(image_size)?;
    log::info!("ext4 image format start: path={}", img_path.display());
    format_ext4_image(img_path)?;
    log::info!("ext4 image check start: path={}", img_path.display());
    check_image(img_path)?;

    if let Err(err) = crate::utils::lsetfilecon(img_path, MODULES_IMG_SELINUX_CONTEXT) {
        log::warn!(
            "selinux context set failed: path={}, error={err}",
            img_path.display()
        );
    }

    crate::utils::ensure_dir_exists(target)?;
    mount_ext4_with_repair(img_path, target)?;
    log::info!(
        "ext4 image mounted: image={}, target={}",
        img_path.display(),
        target.display()
    );
    sys::nuke::nuke_ext4_sysfs(target);

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
    let path = img_path.to_str().ok_or_else(|| {
        Error::msg(format!(
            "ext4 image path is not UTF-8: {}",
            img_path.display()
        ))
    })?;
    let device = FileDevice::open_rw(path).map_err(|err| {
        Error::msg(format!(
            "open ext4 image for pure Rust format {}: {err:?}",
            img_path.display()
        ))
    })?;
    let image_size = device.size_bytes();

    format_filesystem(
        &device,
        Some(RUST_EXT4_LABEL),
        None,
        image_size,
        RUST_EXT4_BLOCK_SIZE,
    )
    .map_err(|err| {
        Error::msg(format!(
            "pure Rust ext4 format failed for {}: {err:?}",
            img_path.display()
        ))
    })?;
    device.flush().map_err(|err| {
        Error::msg(format!(
            "flush pure Rust ext4 image {}: {err:?}",
            img_path.display()
        ))
    })?;
    log::info!(
        "ext4 image format complete: path={}, implementation=am-fs-ext4, block_size={}, image_bytes={}",
        img_path.display(),
        RUST_EXT4_BLOCK_SIZE,
        image_size
    );
    Ok(())
}

#[cfg(unix)]
fn check_image(img_path: &Path) -> Result<()> {
    audit_image(img_path, false)
}

#[cfg(unix)]
fn audit_image(img_path: &Path, repair: bool) -> Result<()> {
    let path = img_path.to_str().ok_or_else(|| {
        Error::msg(format!(
            "ext4 image path is not UTF-8: {}",
            img_path.display()
        ))
    })?;
    let device = FileDevice::open_rw(path).map_err(|err| {
        Error::msg(format!(
            "open ext4 image for pure Rust audit {}: {err:?}",
            img_path.display()
        ))
    })?;
    let filesystem = Filesystem::mount(Arc::new(device)).map_err(|err| {
        Error::msg(format!(
            "mount ext4 image in pure Rust audit {}: {err:?}",
            img_path.display()
        ))
    })?;
    let report = filesystem
        .audit_repair(u32::MAX, u32::MAX, repair)
        .map_err(|err| {
            Error::msg(format!(
                "pure Rust ext4 audit failed for {}: {err:?}",
                img_path.display()
            ))
        })?;

    log::info!(
        "ext4 image audit complete: path={}, implementation=am-fs-ext4, repair={}, initial_anomalies={}, repaired={}, remaining_anomalies={}, directories={}, entries={}",
        img_path.display(),
        repair,
        report.initial_anomalies_count,
        report.repaired_count,
        report.anomalies_count,
        report.directories_scanned,
        report.entries_scanned
    );
    if report.anomalies_count == 0 {
        Ok(())
    } else {
        Err(Error::msg(format!(
            "pure Rust ext4 audit found {} remaining anomalies in {}",
            report.anomalies_count,
            img_path.display()
        )))
    }
}

#[cfg(unix)]
fn mount_ext4_with_repair(img_path: &Path, target: &Path) -> Result<()> {
    let first_error = match overlay_utils::mount_ext4(img_path, target) {
        Ok(()) => return Ok(()),
        Err(err) => err,
    };
    log::warn!(
        "ext4 image first mount failed: image={}, target={}, repair=pure_rust, error={first_error}",
        img_path.display(),
        target.display()
    );

    match audit_image(img_path, true) {
        Ok(()) => match overlay_utils::mount_ext4(img_path, target) {
            Ok(()) => return Ok(()),
            Err(err) => log::warn!(
                "ext4 mount retry after pure Rust repair failed: image={}, target={}, fallback=e2fsck, error={err}",
                img_path.display(),
                target.display()
            ),
        },
        Err(err) => log::warn!(
            "pure Rust ext4 repair incomplete: image={}, fallback=e2fsck, error={err}",
            img_path.display()
        ),
    }

    sys::mount::repair_image(img_path).map_err(|repair_err| {
        Error::msg(format!(
            "ext4 mount failed for {} at {} ({first_error}); pure Rust repair did not recover it and external e2fsck fallback failed: {repair_err}",
            img_path.display(),
            target.display()
        ))
    })?;
    log::info!(
        "ext4 external compatibility repair complete: image={}, retry_target={}",
        img_path.display(),
        target.display()
    );
    overlay_utils::mount_ext4(img_path, target).map_err(|err| {
        Error::msg(format!(
            "mount repaired ext4 image {} at {}: {err}",
            img_path.display(),
            target.display()
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pure_rust_formatter_creates_a_clean_production_sized_image() {
        use fs_ext4::Filesystem;
        use fs_ext4::block_io::{BlockDevice, FileDevice};
        use fs_ext4::mkfs::format_filesystem;
        use std::fs;
        use std::sync::Arc;

        let image = std::env::temp_dir().join(format!(
            "hybrid-mount-ext4-format-{}-{}.img",
            std::process::id(),
            getrandom::u64().unwrap()
        ));
        fs::File::create(&image)
            .unwrap()
            .set_len(EXT4_MIN_IMAGE_SIZE_BYTES)
            .unwrap();

        let device = FileDevice::open_rw(image.to_str().unwrap()).unwrap();
        format_filesystem(
            &device,
            Some(RUST_EXT4_LABEL),
            None,
            EXT4_MIN_IMAGE_SIZE_BYTES,
            RUST_EXT4_BLOCK_SIZE,
        )
        .unwrap();
        device.flush().unwrap();

        let filesystem = Filesystem::mount(Arc::new(device)).unwrap();
        let report = filesystem.audit_repair(u32::MAX, u32::MAX, false).unwrap();
        assert_eq!(report.anomalies_count, 0);
        drop(filesystem);
        fs::remove_file(image).unwrap();
    }

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
