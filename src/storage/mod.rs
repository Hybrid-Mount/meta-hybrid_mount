// ReHybrid-Mount
//
// SPDX-License-Identifier: GPL-3.0-only

//! Overlay 的 staging 后端:tmpfs 或 ext4 loop 镜像(行为对齐 v4.2.0)。
//!
//! - 非 ext4 强制时先试 tmpfs,要求内核 `CONFIG_TMPFS_XATTR=y`;
//! - 否则创建/格式化/校验 ext4 镜像并 loop 挂载;
//! - 挂载完成后设置 private propagation,并按需注册进 KSU 尝试卸载列表。
//!
//! Stage 3 脚手架:入口在 Stage 5 CLI 接入前暂未被二进制入口使用;
//! 接入完成后移除本豁免,恢复 dead_code 检查。
#![allow(dead_code)]

mod ext4;

use std::fs;
use std::path::{Path, PathBuf};

#[cfg(any(target_os = "linux", target_os = "android"))]
use rustix::mount::{MountPropagationFlags, UnmountFlags, mount_change, unmount};

use crate::defs;
use crate::errors::Result;
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::sys;
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::utils::ksu::send_unmountable;

/// staging 后端模式(与 `config.toml` 的 `overlay_mode` 对应)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageMode {
    Tmpfs,
    Ext4,
}

impl StorageMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Tmpfs => "tmpfs",
            Self::Ext4 => "ext4",
        }
    }
}

/// 已建立的 staging 挂载句柄。
#[derive(Debug)]
pub struct StorageHandle {
    mount_point: PathBuf,
    mode: StorageMode,
}

impl StorageHandle {
    pub fn new(mount_point: &Path, mode: StorageMode) -> Self {
        Self {
            mount_point: mount_point.to_path_buf(),
            mode,
        }
    }

    pub fn mount_point(&self) -> &Path {
        &self.mount_point
    }

    pub const fn mode(&self) -> StorageMode {
        self.mode
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn setup(
    mnt_base: &Path,
    moduledir: &Path,
    force_ext4: bool,
    mount_source: &str,
    disable_umount: bool,
) -> Result<StorageHandle> {
    setup_with_sources(
        mnt_base,
        &[moduledir.to_path_buf()],
        force_ext4,
        mount_source,
        disable_umount,
        Path::new(defs::MODULES_IMG_FILE),
    )
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn setup_with_sources(
    mnt_base: &Path,
    source_paths: &[PathBuf],
    force_ext4: bool,
    mount_source: &str,
    disable_umount: bool,
    img_path: &Path,
) -> Result<StorageHandle> {
    log::info!(
        "storage setup start: mount_point={}, requested_mode={}, sources={}, image={}",
        mnt_base.display(),
        if force_ext4 { "ext4" } else { "tmpfs" },
        source_paths.len(),
        img_path.display()
    );
    reset_image_files(img_path)?;
    detach_existing_mount(mnt_base);

    if !force_ext4 && try_setup_tmpfs(mnt_base, mount_source)? {
        log::info!("storage backend select: mode=tmpfs");
        finalize_mount_setup(mnt_base, disable_umount);
        let handle = StorageHandle::new(mnt_base, StorageMode::Tmpfs);
        log::info!(
            "storage setup complete: mode={}, mount_point={}",
            handle.mode().as_str(),
            handle.mount_point().display()
        );
        return Ok(handle);
    }

    let handle = ext4::setup_ext4_image(mnt_base, img_path, source_paths)?;
    finalize_mount_setup(mnt_base, disable_umount);
    log::info!(
        "storage setup complete: mode={}, mount_point={}",
        handle.mode().as_str(),
        handle.mount_point().display()
    );
    Ok(handle)
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn setup_with_sources(
    _mnt_base: &Path,
    _source_paths: &[PathBuf],
    _force_ext4: bool,
    _mount_source: &str,
    _disable_umount: bool,
    _img_path: &Path,
) -> Result<StorageHandle> {
    Err(crate::errors::Error::msg(
        "storage setup is only supported on linux/android",
    ))
}

fn reset_image_files(img_path: &Path) -> Result<()> {
    let Some(parent) = img_path.parent() else {
        return Ok(());
    };
    let Some(file_name) = img_path.file_name() else {
        return Ok(());
    };
    let prefix = file_name.to_string_lossy();

    let Ok(entries) = fs::read_dir(parent) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        if !entry
            .file_name()
            .to_string_lossy()
            .starts_with(prefix.as_ref())
        {
            continue;
        }
        if let Err(err) = fs::remove_file(entry.path()) {
            log::warn!(
                "remove stale image file failed: path={}, error={err}",
                entry.path().display()
            );
        }
    }
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn detach_existing_mount(mnt_base: &Path) {
    if sys::mount::is_mounted(mnt_base)
        && let Err(err) = unmount(mnt_base, UnmountFlags::DETACH)
    {
        log::warn!(
            "detach existing mount failed at {}: {err}",
            mnt_base.display()
        );
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn finalize_mount_setup(path: &Path, disable_umount: bool) {
    if let Err(err) = mount_change(path, MountPropagationFlags::PRIVATE) {
        log::warn!(
            "set mount propagation private failed at {}: {err}",
            path.display()
        );
    }

    if !disable_umount {
        send_unmountable(path);
    }
}

/// Detach the temporary prepared-tree filesystem after OverlayFS has acquired
/// references to its lower layers.  The overlay mounts remain valid, while the
/// transient mount point and image no longer leak into userspace.
pub fn teardown(handle: &StorageHandle) -> Result<()> {
    log::info!(
        "storage teardown start: mode={}, mount_point={}",
        handle.mode().as_str(),
        handle.mount_point().display()
    );

    #[cfg(any(target_os = "linux", target_os = "android"))]
    if sys::mount::is_mounted(handle.mount_point()) {
        unmount(handle.mount_point(), UnmountFlags::DETACH).map_err(|err| {
            crate::errors::Error::msg(format!(
                "detach storage mount {}: {err}",
                handle.mount_point().display()
            ))
        })?;
    }

    match fs::remove_dir(handle.mount_point()) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => log::warn!(
            "storage mount directory cleanup skipped: path={}, error={err}",
            handle.mount_point().display()
        ),
    }

    cleanup_artifacts(handle.mode())?;
    log::info!("storage teardown complete: mode={}", handle.mode().as_str());
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn try_setup_tmpfs(target: &Path, mount_source: &str) -> Result<bool> {
    match sys::mount::mount_tmpfs(target, mount_source) {
        Ok(()) => match sys::fs::is_overlay_xattr_supported() {
            Ok(true) => return Ok(true),
            Ok(false) => {
                log::warn!(
                    "tmpfs fallback: path={}, reason=overlay_xattr_unsupported",
                    target.display()
                );
            }
            Err(err) => {
                log::warn!(
                    "tmpfs fallback: path={}, reason=overlay_xattr_probe_failed, error={err}",
                    target.display()
                );
            }
        },
        Err(err) => {
            log::warn!(
                "tmpfs mount failed: path={}, source={mount_source}, fallback=ext4, error={err}",
                target.display()
            );
            return Ok(false);
        }
    }

    if let Err(err) = unmount(target, UnmountFlags::DETACH) {
        log::warn!(
            "unmount tmpfs failed after xattr probe at {}: {err}",
            target.display()
        );
    }
    Ok(false)
}

fn should_cleanup_image(storage_mode: StorageMode) -> bool {
    matches!(storage_mode, StorageMode::Ext4)
}

fn remove_image_file(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        #[cfg(unix)]
        Err(err) if err.raw_os_error() == Some(libc::EBUSY) => {
            log::warn!(
                "cleanup skipped: path={}, reason=resource_busy",
                path.display()
            );
            Ok(())
        }
        Err(err) => Err(err.into()),
    }
}

/// ext4 模式下移除镜像文件;tmpfs 模式没有镜像。
pub fn cleanup_artifacts(storage_mode: StorageMode) -> Result<()> {
    if should_cleanup_image(storage_mode) {
        remove_image_file(Path::new(defs::MODULES_IMG_FILE))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_mode_names_match_contract() {
        assert_eq!(StorageMode::Tmpfs.as_str(), "tmpfs");
        assert_eq!(StorageMode::Ext4.as_str(), "ext4");
    }

    #[test]
    fn cleanup_only_applies_to_ext4() {
        assert!(!should_cleanup_image(StorageMode::Tmpfs));
        assert!(should_cleanup_image(StorageMode::Ext4));
    }
}
