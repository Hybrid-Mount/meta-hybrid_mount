// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    collections::HashSet,
    fs,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, bail, ensure};
use jwalk::WalkDir;
use rustix::mount::{MountPropagationFlags, UnmountFlags, mount_change, unmount as umount};

#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::mount::umount_mgr::send_umountable;
use crate::{
    core::backend::StorageBackend,
    defs,
    mount::overlayfs::utils as overlay_utils,
    sys::{
        fs::{ensure_dir_exists, lsetfilecon},
        mount::is_mounted,
        nuke,
    },
    utils,
};

const DEFAULT_SELINUX_CONTEXT: &str = "u:object_r:system_file:s0";

pub struct StorageHandle {
    pub backend: Box<dyn StorageBackend>,
}

impl StorageHandle {
    pub fn commit(&mut self, disable_umount: bool) -> Result<()> {
        self.backend.commit(disable_umount)
    }

    pub fn mount_point(&self) -> &Path {
        self.backend.mount_point()
    }

    pub fn mode(&self) -> &str {
        self.backend.mode()
    }
}

pub struct Ext4Backend {
    pub mount_point: PathBuf,
    pub mode: String,
}

impl StorageBackend for Ext4Backend {
    fn commit(&mut self, _disable_umount: bool) -> Result<()> {
        Ok(())
    }

    fn mount_point(&self) -> &Path {
        &self.mount_point
    }

    fn mode(&self) -> &str {
        &self.mode
    }
}

pub struct TmpfsBackend {
    pub mount_point: PathBuf,
    pub mode: String,
}

impl StorageBackend for TmpfsBackend {
    fn commit(&mut self, _disable_umount: bool) -> Result<()> {
        Ok(())
    }

    fn mount_point(&self) -> &Path {
        &self.mount_point
    }

    fn mode(&self) -> &str {
        &self.mode
    }
}

fn calculate_total_size(path: &Path) -> Result<u64> {
    let mut total_size = 0;
    let mut visited_node_map = HashSet::new();
    let mut stack = vec![path.to_path_buf()];

    while let Some(c) = stack.pop() {
        let md = fs::metadata(&c)?;
        if c.is_file() {
            let dev = md.dev();
            let ino = md.ino();

            if !visited_node_map.insert((dev, ino)) {
                continue;
            }

            let blocks = md.blocks();
            total_size += blocks as u64 * 512;
        } else if c.is_dir() {
            match c.read_dir() {
                Ok(r) => {
                    for i in r.flatten() {
                        stack.push(i.path());
                    }
                }
                Err(_e) => {
                    log::error!("Failed to read dir {}", c.display());
                }
            }
        }
    }
    Ok(total_size)
}

fn check_image<P>(img: P) -> Result<()>
where
    P: AsRef<Path>,
{
    let path = img.as_ref();
    let path_str = path.to_str().context("Invalid path string")?;
    let result = Command::new("e2fsck")
        .args(["-yf", path_str])
        .status()
        .with_context(|| format!("Failed to exec e2fsck {}", path.display()))?;
    let _ = result.code();
    Ok(())
}

pub fn setup(
    mnt_base: &Path,
    moduledir: &Path,
    force_ext4: bool,
    mount_source: &str,
    disable_umount: bool,
) -> Result<StorageHandle> {
    let img_path = PathBuf::from(defs::MODULES_IMG_FILE);

    for p in glob::glob(&format!("{}*", defs::MODULES_IMG_FILE))?.flatten() {
        let _ = fs::remove_file(p);
    }

    if is_mounted(mnt_base) {
        log::trace!("{} was mounted, umounting", mnt_base.display());
        let _ = umount(mnt_base, UnmountFlags::DETACH);
    }

    let try_hide = |path: &Path| {
        log::trace!("trying hide {}", path.display());
        #[cfg(any(target_os = "linux", target_os = "android"))]
        if !disable_umount {
            let _ = send_umountable(path);
        }
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        let _ = path;
    };

    let make_private = |path: &Path| {
        log::trace!("trying make {} is private", path.display());
        let _ = mount_change(path, MountPropagationFlags::PRIVATE);
    };

    if !force_ext4 && try_setup_tmpfs(mnt_base, mount_source)? {
        log::trace!("tmpfs mode is supported");
        make_private(mnt_base);
        try_hide(mnt_base);

        return Ok(StorageHandle {
            backend: Box::new(TmpfsBackend {
                mount_point: mnt_base.to_path_buf(),
                mode: "tmpfs".to_string(),
            }),
        });
    }

    let handle = setup_ext4_image(mnt_base, &img_path, moduledir)?;
    make_private(mnt_base);
    try_hide(mnt_base);

    Ok(StorageHandle {
        backend: Box::new(handle),
    })
}

fn try_setup_tmpfs(target: &Path, mount_source: &str) -> Result<bool> {
    if crate::sys::mount::mount_tmpfs(target, mount_source).is_ok() {
        if crate::sys::fs::is_overlay_xattr_supported().unwrap_or(false) {
            return Ok(true);
        } else {
            let _ = umount(target, UnmountFlags::DETACH);
        }
    }
    Ok(false)
}

fn setup_ext4_image<P>(target: P, img_path: P, moduledir: P) -> Result<Ext4Backend>
where
    P: AsRef<Path>,
{
    let (target, img_path, moduledir) = (target.as_ref(), img_path.as_ref(), moduledir.as_ref());
    log::trace!("using ext4 mode");
    let total_size = calculate_total_size(moduledir)?;
    let min_size = 64 * 1024 * 1024;
    let grow_size = std::cmp::max((total_size as f64 * 1.2) as u64, min_size);

    fs::File::create(img_path)?.set_len(grow_size)?;

    log::trace!("formating image");
    let result = Command::new("mkfs.ext4")
        .arg("-b")
        .arg("1024")
        .arg("-i")
        .arg("4096")
        .arg(img_path)
        .stdout(std::process::Stdio::piped())
        .output()?;

    ensure!(result.status.success(), "Failed to format ext4 image");

    check_image(img_path)?;
    let _ = lsetfilecon(img_path, "u:object_r:ksu_file:s0");
    ensure_dir_exists(target)?;

    if overlay_utils::mount_ext4(img_path, target).is_err() {
        if crate::sys::mount::repair_image(img_path).is_ok() {
            overlay_utils::mount_ext4(img_path, target)?;
        } else {
            bail!("Failed to repair modules.img");
        }
    }

    if utils::KSU.load(std::sync::atomic::Ordering::Relaxed) {
        nuke::nuke_path(target);
    } else {
        umount(target, UnmountFlags::DETACH)?;
    }

    for dir_entry in WalkDir::new(target).parallelism(jwalk::Parallelism::Serial) {
        if let Some(path) = dir_entry.ok().map(|dir_entry| dir_entry.path()) {
            let _ = lsetfilecon(&path, DEFAULT_SELINUX_CONTEXT);
        }
    }

    Ok(Ext4Backend {
        mount_point: target.to_path_buf(),
        mode: "ext4".to_string(),
    })
}
