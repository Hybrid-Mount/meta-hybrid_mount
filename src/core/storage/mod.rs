// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

mod ext4;

use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(feature = "control-plane")]
use anyhow::bail;
use anyhow::{Context, Result};
#[cfg(any(target_os = "linux", target_os = "android"))]
use rustix::mount::{MountPropagationFlags, UnmountFlags, mount_change, unmount as umount};

use crate::defs;
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::mount::umount_mgr::send_umountable;
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::sys::mount::is_mounted;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageMode {
    #[cfg(feature = "control-plane")]
    Tmpfs,
    Ext4,
}

impl StorageMode {
    pub fn as_str(self) -> &'static str {
        match self {
            #[cfg(feature = "control-plane")]
            Self::Tmpfs => "tmpfs",
            Self::Ext4 => "ext4",
        }
    }
}

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

    pub fn mode(&self) -> StorageMode {
        self.mode
    }
}

pub fn setup(
    mnt_base: &Path,
    moduledir: &Path,
    force_ext4: bool,
    mount_source: &str,
    disable_umount: bool,
) -> Result<StorageHandle> {
    let source_paths = vec![moduledir.to_path_buf()];
    let img_path = PathBuf::from(defs::MODULES_IMG_FILE);

    setup_with_sources(
        mnt_base,
        &source_paths,
        force_ext4,
        mount_source,
        disable_umount,
        &img_path,
    )
}

pub fn setup_with_sources(
    mnt_base: &Path,
    source_paths: &[PathBuf],
    force_ext4: bool,
    mount_source: &str,
    disable_umount: bool,
    img_path: &Path,
) -> Result<StorageHandle> {
    reset_image_files(img_path)?;
    detach_existing_mount(mnt_base)?;

    #[cfg(feature = "control-plane")]
    if !force_ext4 {
        setup_tmpfs(mnt_base, mount_source)?;
        crate::scoped_log!(trace, "storage", "backend select: mode=tmpfs");
        finalize_mount_setup(mnt_base, disable_umount)?;
        return Ok(StorageHandle::new(mnt_base, StorageMode::Tmpfs));
    }
    #[cfg(not(feature = "control-plane"))]
    let _ = (force_ext4, mount_source);

    let handle = ext4::setup_ext4_image(mnt_base, img_path, source_paths)?;
    finalize_mount_setup(mnt_base, disable_umount)?;

    Ok(handle)
}

fn reset_image_files(img_path: &Path) -> Result<()> {
    let parent = img_path
        .parent()
        .context("storage image path has no parent directory")?;
    let prefix = img_path
        .file_name()
        .context("storage image path has no file name")?;
    let prefix = prefix.to_string_lossy();
    let entries = match fs::read_dir(parent) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err.into()),
    };

    for entry in entries {
        let entry = entry.context("failed to read stale storage image path")?;
        let file_name = entry.file_name();
        if file_name.to_string_lossy().starts_with(prefix.as_ref()) {
            fs::remove_file(entry.path()).with_context(|| {
                format!(
                    "failed to remove stale image file {}",
                    entry.path().display()
                )
            })?;
        }
    }
    Ok(())
}

pub fn cleanup_artifacts(storage_mode: StorageMode) -> Result<()> {
    if should_cleanup_image(storage_mode) {
        remove_image_file(Path::new(defs::MODULES_IMG_FILE))?;
    }

    Ok(())
}

fn should_cleanup_image(storage_mode: StorageMode) -> bool {
    matches!(storage_mode, StorageMode::Ext4)
}

fn remove_image_file(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(_) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.into()),
    }
}

fn detach_existing_mount(mnt_base: &Path) -> Result<()> {
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        let _ = mnt_base;
        Ok(())
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        if is_mounted(mnt_base)? {
            umount(mnt_base, UnmountFlags::DETACH).with_context(|| {
                format!("failed to detach existing mount at {}", mnt_base.display())
            })?;
        }
        Ok(())
    }
}

fn finalize_mount_setup(path: &Path, disable_umount: bool) -> Result<()> {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    mount_change(path, MountPropagationFlags::PRIVATE).with_context(|| {
        format!(
            "failed to set mount propagation to PRIVATE at {}",
            path.display()
        )
    })?;

    #[cfg(any(target_os = "linux", target_os = "android"))]
    if !disable_umount {
        send_umountable(path)
            .with_context(|| format!("failed to register umountable at {}", path.display()))?;
    }

    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    let _ = (path, disable_umount);

    Ok(())
}

#[cfg(feature = "control-plane")]
fn setup_tmpfs(target: &Path, mount_source: &str) -> Result<()> {
    crate::sys::mount::mount_tmpfs(target, mount_source)?;
    match crate::sys::fs::is_overlay_xattr_supported() {
        Ok(true) => Ok(()),
        Ok(false) => {
            detach_existing_mount(target)?;
            bail!(
                "tmpfs at {} does not support OverlayFS xattrs",
                target.display()
            )
        }
        Err(err) => {
            detach_existing_mount(target)?;
            Err(err).context("failed to probe tmpfs OverlayFS xattr support")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{StorageMode, should_cleanup_image};

    #[test]
    fn storage_mode_as_str_matches_expected_values() {
        #[cfg(feature = "control-plane")]
        assert_eq!(StorageMode::Tmpfs.as_str(), "tmpfs");
        assert_eq!(StorageMode::Ext4.as_str(), "ext4");
    }

    #[test]
    fn cleanup_image_only_for_ext4_mode() {
        #[cfg(feature = "control-plane")]
        assert!(!should_cleanup_image(StorageMode::Tmpfs));
        assert!(should_cleanup_image(StorageMode::Ext4));
    }
}
