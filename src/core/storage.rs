// Copyright 2025 Meta-Hybrid Mount Authors
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use rustix::{
    fs::Mode,
    mount::{UnmountFlags, unmount},
};
use serde::Serialize;

#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::try_umount::send_unmountable;
use crate::{core::state::RuntimeState, utils};

const DEFAULT_SELINUX_CONTEXT: &str = "u:object_r:system_file:s0";

pub struct StorageHandle {
    pub mount_point: PathBuf,
    pub mode: String,
    pub backing_image: Option<PathBuf>,
}

impl StorageHandle {
    pub fn commit(&mut self, disable_umount: bool) -> Result<()> {
        if self.mode == "erofs_staging" {
            let image_path = self
                .backing_image
                .as_ref()
                .context("EROFS backing image path missing")?;

            utils::create_erofs_image(&self.mount_point, image_path)
                .context("Failed to pack EROFS image")?;

            unmount(&self.mount_point, UnmountFlags::DETACH)
                .context("Failed to unmount staging tmpfs")?;

            utils::mount_erofs_image(image_path, &self.mount_point)
                .context("Failed to mount finalized EROFS image")?;

            #[cfg(any(target_os = "linux", target_os = "android"))]
            if !disable_umount {
                let _ = send_unmountable(&self.mount_point);
            }

            self.mode = "erofs".to_string();
        }

        Ok(())
    }
}

#[derive(Serialize)]
struct StorageStatus {
    #[serde(rename = "type")]
    mode: String,
    mount_point: String,
    usage_percent: u8,
    total_size: u64,
    used_size: u64,
}

pub fn get_usage(path: &Path) -> (u64, u64, u8) {
    if let Ok(stat) = rustix::fs::statvfs(path) {
        let total = stat.f_blocks * stat.f_frsize;

        let free = stat.f_bfree * stat.f_frsize;

        let used = total - free;

        let percent = if total > 0 {
            (used * 100 / total) as u8
        } else {
            0
        };

        (total, used, percent)
    } else {
        (0, 0, 0)
    }
}

pub fn setup(
    mnt_base: &Path,
    img_path: &Path,
    _moduledir: &Path,
    force_ext4: bool,
    use_erofs: bool,
    mount_source: &str,
    disable_umount: bool,
) -> Result<StorageHandle> {
    if utils::is_mounted(mnt_base) {
        let _ = unmount(mnt_base, UnmountFlags::DETACH);
    }

    let try_hide = |path: &Path| {
        #[cfg(any(target_os = "linux", target_os = "android"))]
        if !disable_umount {
            let _ = send_unmountable(path);
        }

        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        let _ = path;
    };

    if use_erofs && utils::is_erofs_supported() {
        let erofs_path = img_path.with_extension("erofs");

        utils::mount_tmpfs(mnt_base, mount_source)?;

        try_hide(mnt_base);

        return Ok(StorageHandle {
            mount_point: mnt_base.to_path_buf(),
            mode: "erofs_staging".to_string(),
            backing_image: Some(erofs_path),
        });
    }

    if !force_ext4 && try_setup_tmpfs(mnt_base, mount_source)? {
        try_hide(mnt_base);

        let erofs_path = img_path.with_extension("erofs");

        if erofs_path.exists() {
            let _ = fs::remove_file(erofs_path);
        }

        return Ok(StorageHandle {
            mount_point: mnt_base.to_path_buf(),
            mode: "tmpfs".to_string(),
            backing_image: None,
        });
    }

    let handle = setup_ext4_image(mnt_base, img_path)?;

    try_hide(mnt_base);

    Ok(handle)
}

fn try_setup_tmpfs(target: &Path, mount_source: &str) -> Result<bool> {
    if utils::mount_tmpfs(target, mount_source).is_ok() {
        if utils::is_overlay_xattr_supported(target).is_ok() {
            return Ok(true);
        } else {
            let _ = unmount(target, UnmountFlags::DETACH);
        }
    }

    Ok(false)
}

fn setup_ext4_image(target: &Path, img_path: &Path) -> Result<StorageHandle> {
    if !img_path.exists() {
        bail!(
            "Modules image not found at {} and automatic creation (mkfs.ext4) has been disabled.",
            img_path.display()
        );
    }

    if utils::mount_image(img_path, target).is_err() {
        if utils::repair_image(img_path).is_ok() {
            utils::mount_image(img_path, target)
                .context("Failed to mount modules.img after repair")?;
        } else {
            bail!("Failed to repair modules.img");
        }
    }

    Ok(StorageHandle {
        mount_point: target.to_path_buf(),
        mode: "ext4".to_string(),
        backing_image: Some(img_path.to_path_buf()),
    })
}

#[allow(dead_code)]
pub fn finalize_storage_permissions(target: &Path) {
    if let Err(e) = rustix::fs::chmod(target, Mode::from(0o755)) {
        log::warn!("Failed to chmod storage root: {}", e);
    }

    if let Err(e) = rustix::fs::chown(
        target,
        Some(rustix::fs::Uid::from_raw(0)),
        Some(rustix::fs::Gid::from_raw(0)),
    ) {
        log::warn!("Failed to chown storage root: {}", e);
    }

    if let Err(e) = utils::lsetfilecon(target, DEFAULT_SELINUX_CONTEXT) {
        log::warn!("Failed to set SELinux context: {}", e);
    }
}

pub fn print_status() -> Result<()> {
    let state = RuntimeState::load().ok();
    let fallback_mnt = crate::conf::config::Config::load_default()
        .map(|c| c.hybrid_mnt_dir)
        .unwrap_or_else(|_| crate::defs::DEFAULT_HYBRID_MNT_DIR.to_string());
    let (mnt_base, expected_mode) = if let Some(ref s) = state {
        (s.mount_point.clone(), s.storage_mode.clone())
    } else {
        (PathBuf::from(fallback_mnt), "unknown".to_string())
    };

    let mut mode = "unknown".to_string();

    let mut total = 0;

    let mut used = 0;

    let mut percent = 0;

    if utils::is_mounted(&mnt_base)
        && let Ok(stat) = rustix::fs::statvfs(&mnt_base)
    {
        mode = if expected_mode != "unknown" {
            expected_mode
        } else {
            "active".to_string()
        };

        total = stat.f_blocks * stat.f_frsize;

        let free = stat.f_bfree * stat.f_frsize;

        used = total - free;

        if total > 0 {
            percent = (used * 100 / total) as u8;
        }
    }

    let status = StorageStatus {
        mode,
        mount_point: mnt_base.to_string_lossy().to_string(),
        usage_percent: percent,
        total_size: total,
        used_size: used,
    };

    println!("{}", serde_json::to_string(&status)?);

    Ok(())
}
