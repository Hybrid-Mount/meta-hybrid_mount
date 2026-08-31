// SPDX-License-Identifier: GPL-3.0-only

//! Overlay 的 staging 后端:tmpfs 或 ext4 loop 镜像(行为对齐 v4.2.0)。
//!
//! - 非 ext4 强制时先试 tmpfs,要求内核 `CONFIG_TMPFS_XATTR=y`;
//! - 否则创建/格式化/校验 ext4 镜像并 loop 挂载;
//! - 挂载完成后设置 private propagation,并按需注册进 KSU 尝试卸载列表。

mod ext4;

use std::fs;
use std::path::{Path, PathBuf};

#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::overlayfs::utils::Ext4LoopMount;
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
    #[cfg(any(target_os = "linux", target_os = "android"))]
    loop_mount: Option<Ext4LoopMount>,
}

impl StorageHandle {
    pub fn new(mount_point: &Path, mode: StorageMode) -> Self {
        Self {
            mount_point: mount_point.to_path_buf(),
            mode,
            #[cfg(any(target_os = "linux", target_os = "android"))]
            loop_mount: None,
        }
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub fn new_ext4(mount_point: &Path, mode: StorageMode, loop_mount: Ext4LoopMount) -> Self {
        Self {
            mount_point: mount_point.to_path_buf(),
            mode,
            loop_mount: Some(loop_mount),
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
    sizing_paths: &[PathBuf],
    force_ext4: bool,
    mount_source: &str,
    disable_umount: bool,
) -> Result<StorageHandle> {
    setup_with_sources(
        mnt_base,
        sizing_paths,
        force_ext4,
        mount_source,
        disable_umount,
        Path::new(defs::MODULES_IMG_FILE),
    )
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn setup_with_sources(
    mnt_base: &Path,
    sizing_paths: &[PathBuf],
    force_ext4: bool,
    mount_source: &str,
    disable_umount: bool,
    img_path: &Path,
) -> Result<StorageHandle> {
    log::info!(
        "storage setup start: mount_point={}, requested_mode={}, sizing_paths={}, image={}",
        mnt_base.display(),
        if force_ext4 { "ext4" } else { "tmpfs" },
        sizing_paths.len(),
        img_path.display()
    );
    reset_image_files(img_path)?;
    detach_existing_mount(mnt_base)?;

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

    let handle = ext4::setup_ext4_image(mnt_base, img_path, sizing_paths)?;
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
    _sizing_paths: &[PathBuf],
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

    let entries = match fs::read_dir(parent) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err.into()),
    };
    for entry in entries {
        let entry = entry?;
        // 只删除项目自有的精确文件名；modules.img.bak 等用户备份必须保留。
        if entry.file_name() != file_name {
            continue;
        }
        match fs::remove_file(entry.path()) {
            Ok(()) => log::debug!("stale image file removed: path={}", entry.path().display()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                return Err(crate::errors::Error::Storage(Box::new(
                    crate::errors::ContextError::new(
                        "remove stale ext4 image",
                        Some(entry.path()),
                        err,
                    ),
                )));
            }
        }
    }
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn detach_existing_mount(mnt_base: &Path) -> Result<()> {
    if !sys::mount::is_mounted(mnt_base)? {
        return Ok(());
    }

    unmount(mnt_base, UnmountFlags::DETACH).map_err(|err| {
        crate::errors::Error::msg(format!(
            "detach existing mount failed at {}: {err}",
            mnt_base.display()
        ))
    })
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
    if sys::mount::is_mounted(handle.mount_point())? {
        unmount(handle.mount_point(), UnmountFlags::DETACH).map_err(|err| {
            crate::errors::Error::msg(format!(
                "detach storage mount {}: {err}",
                handle.mount_point().display()
            ))
        })?;
    }
    #[cfg(any(target_os = "linux", target_os = "android"))]
    if sys::mount::is_mounted(handle.mount_point())? {
        return Err(crate::errors::Error::Storage(Box::new(
            crate::errors::ContextError::new(
                "verify storage mount detached",
                Some(handle.mount_point().to_path_buf()),
                "storage mount still present after teardown".to_owned(),
            ),
        )));
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    if let Some(loop_mount) = &handle.loop_mount {
        loop_mount.detach().map_err(|err| {
            crate::errors::Error::Storage(Box::new(crate::errors::ContextError::new(
                "detach ext4 loop device",
                Some(handle.mount_point().to_path_buf()),
                err,
            )))
        })?;
        log::debug!("ext4 loop device detached: mode=ext4");
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
        // tmpfs 回退前必须确认 tmpfs 已卸载；卸载失败 fail-closed，
        // 不能继续尝试把 ext4 挂到同一个仍被占用的目标上。
        return Err(crate::errors::Error::Storage(Box::new(
            crate::errors::ContextError::new(
                "unmount tmpfs before ext4 fallback",
                Some(target.to_path_buf()),
                err,
            ),
        )));
    }
    if sys::mount::is_mounted(target)? {
        return Err(crate::errors::Error::Storage(Box::new(
            crate::errors::ContextError::new(
                "verify tmpfs unmounted before ext4 fallback",
                Some(target.to_path_buf()),
                "target is still mounted after tmpfs teardown".to_owned(),
            ),
        )));
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
        #[cfg(any(target_os = "linux", target_os = "android"))]
        Err(err) if err.raw_os_error() == Some(rustix::io::Errno::BUSY.raw_os_error()) => {
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

    #[test]
    fn reset_image_files_removes_only_the_exact_image_name() {
        let dir =
            std::env::temp_dir().join(format!("hybrid-mount-reset-image-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let image = dir.join("modules.img");
        let backup = dir.join("modules.img.bak");
        let old = dir.join("modules.img.old");
        let unrelated = dir.join("notes.txt");
        for path in [&image, &backup, &old, &unrelated] {
            fs::write(path, b"data").unwrap();
        }

        reset_image_files(&image).unwrap();

        assert!(!image.exists());
        assert!(backup.exists(), "user backup must be preserved");
        assert!(old.exists(), "user backup must be preserved");
        assert!(unrelated.exists());
        fs::remove_dir_all(&dir).ok();
    }
}
