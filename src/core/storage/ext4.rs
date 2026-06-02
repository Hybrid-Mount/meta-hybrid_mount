// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::{
    collections::{BTreeMap, HashSet},
    fs,
    io::ErrorKind,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, bail, ensure};

use crate::{
    core::storage::{StorageHandle, StorageMode},
    mount::overlayfs::utils as overlay_utils,
    sys::{
        fs::{ensure_dir_exists, lsetfilecon},
        nuke,
    },
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
    if let Err(e) = lsetfilecon(img_path, MODULES_IMG_SELINUX_CONTEXT) {
        crate::scoped_log!(
            warn,
            "storage",
            "selinux context set failed: path={}, error={:#}",
            img_path.display(),
            e
        );
    }
    ensure_dir_exists(target)?;

    mount_ext4_with_repair(img_path, target)?;
    reset_mount_state(target);

    Ok(StorageHandle::new(target, StorageMode::Ext4))
}

fn calculate_total_size(paths: &[PathBuf]) -> Result<u64> {
    let mut total_size = 0;
    let mut visited_node_map = HashSet::new();
    let mut symlink_stats = SizeScanSymlinkStats::default();
    let mut stack: Vec<PathBuf> = paths.iter().filter(|path| path.exists()).cloned().collect();

    while let Some(current) = stack.pop() {
        let metadata = match fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(err) if err.raw_os_error() == Some(libc::ELOOP) => {
                symlink_stats.record_loop(&current);
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

            total_size += metadata.blocks() * STAT_BLOCK_SIZE_BYTES;
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
            symlink_stats.record_skip(&current);
        }
    }
    symlink_stats.log();
    Ok(total_size)
}

#[derive(Default)]
struct SizeScanSymlinkStats {
    skipped: BTreeMap<String, usize>,
    loops: BTreeMap<String, usize>,
}

impl SizeScanSymlinkStats {
    fn record_skip(&mut self, path: &Path) {
        *self.skipped.entry(module_log_key(path)).or_default() += 1;
    }

    fn record_loop(&mut self, path: &Path) {
        *self.loops.entry(module_log_key(path)).or_default() += 1;
    }

    fn log(&self) {
        for (module, count) in &self.skipped {
            crate::scoped_log!(
                debug,
                "storage:ext4",
                "size skip summary: module={}, reason=symlink, count={}",
                module,
                count
            );
        }

        for (module, count) in &self.loops {
            crate::scoped_log!(
                warn,
                "storage:ext4",
                "size skip summary: module={}, reason=symlink_loop, count={}",
                module,
                count
            );
        }
    }
}

fn module_log_key(path: &Path) -> String {
    crate::utils::extract_module_id(path).unwrap_or_else(|| "<unknown>".to_string())
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

fn reset_mount_state(target: &Path) {
    nuke::nuke_path(target);
}
