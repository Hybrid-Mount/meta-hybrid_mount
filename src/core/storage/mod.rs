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

mod ext4;

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::Result;
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
    detach_existing_mount(mnt_base);

    #[cfg(feature = "control-plane")]
    if !force_ext4 && try_setup_tmpfs(mnt_base, mount_source)? {
        crate::scoped_log!(trace, "storage", "backend select: mode=tmpfs");
        finalize_mount_setup(mnt_base, disable_umount);
        return Ok(StorageHandle::new(mnt_base, StorageMode::Tmpfs));
    }
    #[cfg(not(feature = "control-plane"))]
    let _ = (force_ext4, mount_source);

    let handle = ext4::setup_ext4_image(mnt_base, img_path, source_paths)?;
    finalize_mount_setup(mnt_base, disable_umount);

    Ok(handle)
}

fn reset_image_files(img_path: &Path) -> Result<()> {
    let pattern = format!("{}*", img_path.display());
    for path in glob::glob(&pattern)?.flatten() {
        if let Err(e) = fs::remove_file(&path) {
            crate::scoped_log!(
                warn,
                "storage",
                "failed to remove stale image file {}: {:#}",
                path.display(),
                e
            );
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
        Err(err) if err.raw_os_error() == Some(libc::EBUSY) => {
            crate::scoped_log!(
                warn,
                "storage",
                "cleanup skipped: path={}, reason=resource_busy",
                path.display()
            );
            Ok(())
        }
        Err(err) => Err(err.into()),
    }
}

fn detach_existing_mount(mnt_base: &Path) {
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        let _ = mnt_base;
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    if is_mounted(mnt_base)
        && let Err(e) = umount(mnt_base, UnmountFlags::DETACH)
    {
        crate::scoped_log!(
            warn,
            "storage",
            "failed to detach existing mount at {}: {:#}",
            mnt_base.display(),
            e
        );
    }
}

fn finalize_mount_setup(path: &Path, disable_umount: bool) {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    if let Err(e) = mount_change(path, MountPropagationFlags::PRIVATE) {
        crate::scoped_log!(
            warn,
            "storage",
            "failed to set mount propagation to PRIVATE at {}: {:#}",
            path.display(),
            e
        );
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    if !disable_umount && let Err(e) = send_umountable(path) {
        crate::scoped_log!(
            warn,
            "storage",
            "failed to register umountable at {}: {:#}",
            path.display(),
            e
        );
    }

    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    let _ = (path, disable_umount);
}

#[cfg(feature = "control-plane")]
fn try_setup_tmpfs(target: &Path, mount_source: &str) -> Result<bool> {
    match crate::sys::mount::mount_tmpfs(target, mount_source) {
        Ok(()) => match crate::sys::fs::is_overlay_xattr_supported() {
            Ok(true) => return Ok(true),
            Ok(false) => {
                crate::scoped_log!(
                    warn,
                    "storage",
                    "tmpfs fallback: path={}, reason=overlay_xattr_unsupported",
                    target.display()
                );
                #[cfg(any(target_os = "linux", target_os = "android"))]
                if let Err(e) = umount(target, UnmountFlags::DETACH) {
                    crate::scoped_log!(
                        warn,
                        "storage",
                        "failed to umount tmpfs at {} after xattr check: {:#}",
                        target.display(),
                        e
                    );
                }
            }
            Err(err) => {
                crate::scoped_log!(
                    warn,
                    "storage",
                    "tmpfs fallback: path={}, reason=overlay_xattr_probe_failed, error={:#}",
                    target.display(),
                    err
                );
                #[cfg(any(target_os = "linux", target_os = "android"))]
                if let Err(e) = umount(target, UnmountFlags::DETACH) {
                    crate::scoped_log!(
                        warn,
                        "storage",
                        "failed to umount tmpfs at {} after xattr probe failure: {:#}",
                        target.display(),
                        e
                    );
                }
            }
        },
        Err(err) => {
            crate::scoped_log!(
                warn,
                "storage",
                "tmpfs mount failed: path={}, source={}, fallback=ext4, error={:#}",
                target.display(),
                mount_source,
                err
            );
        }
    }
    Ok(false)
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
