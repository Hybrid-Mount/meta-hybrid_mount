// SPDX-License-Identifier: GPL-3.0-only

//! ext4 loop 镜像 staging:
//! 按 staging 后的逻辑数据量和 inode 需求动态估算镜像大小，
//! 使用 Android 系统 `/system/bin/mke2fs`，结合上游 meta-overlayfs 的无日志配置
//! 与 Hybrid Mount 4.2.0 的 1 KiB block / 4 KiB inode 密度格式化；首次挂载前用
//! 系统 `e2fsck` 校验，挂载失败时再修复重试。

#[cfg(unix)]
use std::{fs, path::PathBuf};
#[cfg(any(target_os = "linux", target_os = "android"))]
use std::{path::Path, process::Command};

#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::errors::Error;
#[cfg(unix)]
use crate::errors::Result;
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::overlayfs::utils as overlay_utils;
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::storage::{StorageHandle, StorageMode};
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::sys;

const EXT4_MIN_IMAGE_SIZE_BYTES: u64 = 64 * 1024 * 1024;
const EXT4_FIXED_HEADROOM_BYTES: u64 = 16 * 1024 * 1024;
const EXT4_GROWTH_NUMERATOR: u64 = 5;
const EXT4_GROWTH_DENOMINATOR: u64 = 4;
// Keep the historical 1 KiB geometry for small images. Older Android
// e2fsck builds can crash while checking 1 KiB images with more than 32 groups.
const EXT4_BLOCK_SIZE_BYTES: u64 = 1024;
const EXT4_LARGE_BLOCK_SIZE_BYTES: u64 = 4096;
const EXT4_BLOCKS_PER_GROUP_FACTOR: u64 = 8;
const EXT4_MAX_1K_BLOCK_GROUPS: u64 = 32;
const EXT4_IMAGE_ALIGNMENT_BYTES: u64 = 4 * 1024 * 1024;
const EXT4_BYTES_PER_INODE: u64 = 4096;
const SYSTEM_MKE2FS: &str = "/system/bin/mke2fs";
const MODULES_IMG_SELINUX_CONTEXT: &str = "u:object_r:ksu_file:s0";

/// staging 逻辑占用统计器。
///
/// 普通复制会把 F2FS 压缩/稀疏文件展开，也不会保留源端硬链接的块共享，
/// 所以每个普通文件都按 4.2.0 使用的 1 KiB 块向上取整，并分别统计所有目标 inode。
#[derive(Debug, Default)]
pub struct SizeCounter {
    data_bytes: u64,
    entries: u64,
}

impl SizeCounter {
    pub fn add_file(&mut self, logical_size: u64) {
        self.data_bytes = self
            .data_bytes
            .saturating_add(round_up(logical_size, EXT4_BLOCK_SIZE_BYTES));
        self.entries = self.entries.saturating_add(1);
    }

    pub fn add_metadata_entry(&mut self) {
        self.entries = self.entries.saturating_add(1);
    }

    pub const fn total(&self) -> u64 {
        self.data_bytes
    }

    pub const fn entries(&self) -> u64 {
        self.entries
    }
}

const fn round_up(value: u64, alignment: u64) -> u64 {
    if alignment == 0 {
        return value;
    }
    let remainder = value % alignment;
    if remainder == 0 {
        value
    } else {
        value.saturating_add(alignment - remainder)
    }
}

const fn multiply_ratio_ceil(value: u64, numerator: u64, denominator: u64) -> u64 {
    if denominator == 0 {
        return u64::MAX;
    }
    let whole = (value / denominator).saturating_mul(numerator);
    let remainder = value % denominator;
    let fractional = remainder
        .saturating_mul(numerator)
        .saturating_add(denominator - 1)
        / denominator;
    whole.saturating_add(fractional)
}

const fn ext4_block_group_size_bytes(block_size: u64) -> u64 {
    block_size
        .saturating_mul(block_size)
        .saturating_mul(EXT4_BLOCKS_PER_GROUP_FACTOR)
}

const fn ext4_block_group_count(image_size: u64, block_size: u64) -> u64 {
    let group_size = ext4_block_group_size_bytes(block_size);
    if group_size == 0 {
        return u64::MAX;
    }
    let groups = image_size / group_size;
    if image_size.is_multiple_of(group_size) {
        groups
    } else {
        groups.saturating_add(1)
    }
}

/// Select a block size that keeps legacy 1 KiB images away from the Android
/// `e2fsck` multi-group crash while retaining the old geometry where possible.
const fn select_ext4_block_size(image_size: u64) -> u64 {
    if ext4_block_group_count(image_size, EXT4_BLOCK_SIZE_BYTES) <= EXT4_MAX_1K_BLOCK_GROUPS {
        EXT4_BLOCK_SIZE_BYTES
    } else {
        EXT4_LARGE_BLOCK_SIZE_BYTES
    }
}

fn mke2fs_args(block_size: u64) -> Vec<String> {
    vec![
        "-t".to_owned(),
        "ext4".to_owned(),
        "-b".to_owned(),
        block_size.to_string(),
        "-i".to_owned(),
        "4096".to_owned(),
        "-O".to_owned(),
        "^has_journal".to_owned(),
        "-F".to_owned(),
    ]
}

/// 镜像容量计划：
///
/// - 数据需求为逻辑文件块 × 1.25 + 16 MiB；
/// - inode 需求按 4.2.0 的 4 KiB/inode 再留 25% 余量；
/// - 两者取较大值，最低 64 MiB，并按 4 MiB 对齐以保持稀疏镜像尺寸规整。
pub fn planned_image_size(data_bytes: u64, entries: u64) -> u64 {
    let data_requirement =
        multiply_ratio_ceil(data_bytes, EXT4_GROWTH_NUMERATOR, EXT4_GROWTH_DENOMINATOR)
            .saturating_add(EXT4_FIXED_HEADROOM_BYTES);
    let inode_requirement =
        multiply_ratio_ceil(entries, EXT4_GROWTH_NUMERATOR, EXT4_GROWTH_DENOMINATOR)
            .saturating_mul(EXT4_BYTES_PER_INODE);
    round_up(
        data_requirement
            .max(inode_requirement)
            .max(EXT4_MIN_IMAGE_SIZE_BYTES),
        EXT4_IMAGE_ALIGNMENT_BYTES,
    )
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub(super) fn setup_ext4_image(
    target: &Path,
    img_path: &Path,
    source_paths: &[PathBuf],
) -> Result<StorageHandle> {
    log::info!("storage backend select: mode=ext4");

    let usage = calculate_total_size(source_paths)?;
    let image_size = planned_image_size(usage.total(), usage.entries());
    let block_size = select_ext4_block_size(image_size);

    log::info!(
        "ext4 image plan: source_bytes={}, source_entries={}, image_bytes={}, block_size={}, source_roots={}",
        usage.total(),
        usage.entries(),
        image_size,
        block_size,
        source_paths.len()
    );

    // set_len creates the same sparse image that upstream creates with truncate.
    fs::File::create(img_path)?.set_len(image_size)?;
    log::info!("ext4 image format start: path={}", img_path.display());
    format_ext4_image(img_path, block_size)?;
    check_ext4_image(img_path)?;

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
    // Overlay lowerdirs keep the ext4 superblock alive even when the staging
    // mount is detached later, so conceal the sysfs node while it is mounted.
    sys::nuke::nuke_ext4_sysfs(target);

    Ok(StorageHandle::new(target, StorageMode::Ext4))
}

#[cfg(unix)]
fn calculate_total_size(paths: &[PathBuf]) -> Result<SizeCounter> {
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
            counter.add_file(metadata.len());
        } else if file_type.is_dir() {
            counter.add_metadata_entry();
            for entry in current.read_dir()? {
                stack.push(entry?.path());
            }
        } else {
            // Symlinks, whiteout character devices, and other special entries
            // still consume an inode in the prepared ext4 tree.
            counter.add_metadata_entry();
        }
    }

    Ok(counter)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn format_ext4_image(img_path: &Path, block_size: u64) -> Result<()> {
    let output = Command::new(SYSTEM_MKE2FS)
        .args(mke2fs_args(block_size))
        .arg(img_path)
        .output()
        .map_err(|err| {
            Error::msg(format!(
                "execute system mke2fs for {}: {err}",
                img_path.display()
            ))
        })?;

    if !output.status.success() {
        let exit = output
            .status
            .code()
            .map_or_else(|| "signal".to_owned(), |code| code.to_string());
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(Error::msg(format!(
            "system mke2fs failed for {} with exit {exit}: stderr={}, stdout={}",
            img_path.display(),
            stderr.trim(),
            stdout.trim()
        )));
    }

    log::info!(
        "ext4 image format complete: path={}, implementation={}, block_size={}, bytes_per_inode={}, features=^has_journal",
        img_path.display(),
        SYSTEM_MKE2FS,
        block_size,
        EXT4_BYTES_PER_INODE
    );
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn check_ext4_image(img_path: &Path) -> Result<()> {
    log::info!(
        "ext4 image check start: path={}, implementation=system_e2fsck",
        img_path.display()
    );
    sys::mount::repair_image(img_path).map_err(|err| {
        Error::msg(format!(
            "system e2fsck validation failed for {}: {err}",
            img_path.display()
        ))
    })?;
    log::info!("ext4 image check complete: path={}", img_path.display());
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn mount_ext4_with_repair(img_path: &Path, target: &Path) -> Result<()> {
    let first_error = match overlay_utils::mount_ext4(img_path, target) {
        Ok(()) => return Ok(()),
        Err(err) => err,
    };
    log::warn!(
        "ext4 image first mount failed: image={}, target={}, repair=e2fsck, error={first_error}",
        img_path.display(),
        target.display()
    );

    sys::mount::repair_image(img_path).map_err(|repair_err| {
        Error::msg(format!(
            "ext4 mount failed for {} at {} ({first_error}); system e2fsck failed: {repair_err}",
            img_path.display(),
            target.display()
        ))
    })?;
    log::info!(
        "ext4 system repair complete: image={}, retry_target={}",
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
    fn formatter_arguments_combine_upstream_and_v4_geometry() {
        let args = mke2fs_args(EXT4_BLOCK_SIZE_BYTES);
        assert_eq!(
            args.iter().map(String::as_str).collect::<Vec<_>>(),
            [
                "-t",
                "ext4",
                "-b",
                "1024",
                "-i",
                "4096",
                "-O",
                "^has_journal",
                "-F"
            ]
        );
        assert_eq!(SYSTEM_MKE2FS, "/system/bin/mke2fs");
    }

    #[test]
    fn formatter_arguments_support_large_image_geometry() {
        let args = mke2fs_args(EXT4_LARGE_BLOCK_SIZE_BYTES);
        assert_eq!(args[3], "4096");
    }

    #[test]
    fn planned_image_size_enforces_minimum() {
        assert_eq!(planned_image_size(0, 0), EXT4_MIN_IMAGE_SIZE_BYTES);
        assert_eq!(planned_image_size(1_000, 1), EXT4_MIN_IMAGE_SIZE_BYTES);
    }

    #[test]
    fn planned_image_size_grows_with_source_data() {
        let total = 100 * 1024 * 1024;
        assert_eq!(planned_image_size(total, 1), 144 * 1024 * 1024);
    }

    #[test]
    fn planned_image_size_accounts_for_inode_demand() {
        assert_eq!(planned_image_size(0, 100_000), 492 * 1024 * 1024);
    }

    #[test]
    fn large_images_use_four_kib_blocks_to_bound_group_count() {
        let issue_image_size = planned_image_size(455_715_840, 158);
        assert_eq!(issue_image_size, 587_202_560);
        assert_eq!(select_ext4_block_size(issue_image_size), 4096);
        assert_eq!(select_ext4_block_size(256 * 1024 * 1024), 1024);
        assert_eq!(
            ext4_block_group_count(256 * 1024 * 1024, EXT4_BLOCK_SIZE_BYTES),
            EXT4_MAX_1K_BLOCK_GROUPS
        );
        assert_eq!(
            ext4_block_group_count(260 * 1024 * 1024, EXT4_BLOCK_SIZE_BYTES),
            EXT4_MAX_1K_BLOCK_GROUPS + 1
        );
        assert_eq!(
            ext4_block_group_count(587_202_560, EXT4_LARGE_BLOCK_SIZE_BYTES),
            5
        );
    }

    #[test]
    fn size_counter_counts_each_copied_entry_by_logical_size() {
        let mut counter = SizeCounter::default();

        counter.add_file(1);
        counter.add_file(1);
        counter.add_metadata_entry();

        assert_eq!(counter.total(), 2 * EXT4_BLOCK_SIZE_BYTES);
        assert_eq!(counter.entries(), 3);
    }

    #[test]
    fn size_calculation_saturates_instead_of_overflowing() {
        let mut counter = SizeCounter::default();
        counter.add_file(u64::MAX);
        assert_eq!(counter.total(), u64::MAX);
        assert_eq!(planned_image_size(u64::MAX, u64::MAX), u64::MAX);
    }
}
