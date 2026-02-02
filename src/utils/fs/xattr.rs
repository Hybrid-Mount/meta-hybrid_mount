// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

#[cfg(any(target_os = "linux", target_os = "android"))]
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
#[cfg(any(target_os = "linux", target_os = "android"))]
use std::process::Command;

use anyhow::{Context, Result};
#[cfg(any(target_os = "linux", target_os = "android"))]
use extattr::{Flags as XattrFlags, lgetxattr, llistxattr, lsetxattr};

const SELINUX_XATTR: &str = "security.selinux";
const OVERLAY_OPAQUE_XATTR: &str = "trusted.overlay.opaque";
const CONTEXT_SYSTEM: &str = "u:object_r:system_file:s0";
const CONTEXT_VENDOR: &str = "u:object_r:vendor_file:s0";
const CONTEXT_HAL: &str = "u:object_r:same_process_hal_file:s0";
const CONTEXT_VENDOR_EXEC: &str = "u:object_r:vendor_file:s0";
const CONTEXT_ROOTFS: &str = "u:object_r:rootfs:s0";

fn copy_extended_attributes(src: &Path, dst: &Path) -> Result<()> {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        if let Ok(mut ctx) = lgetfilecon(src) {
            if ctx.contains("u:object_r:rootfs:s0") {
                ctx = CONTEXT_SYSTEM.to_string();
            }
            let _ = lsetfilecon(dst, &ctx);
        } else {
            let _ = lsetfilecon(dst, CONTEXT_SYSTEM);
        }
        if let Ok(opaque) = lgetxattr(src, OVERLAY_OPAQUE_XATTR) {
            let _ = lsetxattr(dst, OVERLAY_OPAQUE_XATTR, &opaque, XattrFlags::empty());
        }
        if let Ok(xattrs) = llistxattr(src) {
            for xattr_name in xattrs {
                let name_bytes = xattr_name.as_bytes();
                let name_str = String::from_utf8_lossy(name_bytes);

                #[allow(clippy::collapsible_if)]
                if name_str.starts_with("trusted.overlay.") && name_str != OVERLAY_OPAQUE_XATTR {
                    if let Ok(val) = lgetxattr(src, &xattr_name) {
                        let _ = lsetxattr(dst, &xattr_name, &val, XattrFlags::empty());
                    }
                }
            }
        }
    }
    Ok(())
}

pub fn set_overlay_opaque<P: AsRef<Path>>(path: P) -> Result<()> {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        lsetxattr(
            path.as_ref(),
            OVERLAY_OPAQUE_XATTR,
            b"y",
            XattrFlags::empty(),
        )?;
    }
    Ok(())
}

pub fn lsetfilecon<P: AsRef<Path>>(path: P, con: &str) -> Result<()> {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        if let Err(e) = lsetxattr(
            path.as_ref(),
            SELINUX_XATTR,
            con.as_bytes(),
            XattrFlags::empty(),
        ) {
            let _ = e;
        }
    }
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn lgetfilecon<P: AsRef<Path>>(path: P) -> Result<String> {
    let con = extattr::lgetxattr(path.as_ref(), SELINUX_XATTR).with_context(|| {
        format!(
            "Failed to get SELinux context for {}",
            path.as_ref().display()
        )
    })?;
    let con_str = String::from_utf8_lossy(&con).trim_matches('\0').to_string();

    Ok(con_str)
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn lgetfilecon<P: AsRef<Path>>(_path: P) -> Result<String> {
    unimplemented!();
}

pub fn is_overlay_xattr_supported() -> Result<bool> {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        let output = Command::new("zcat")
            .arg("/proc/config.gz")
            .output()
            .context("Failed to read config.gz")
            .unwrap();
        let config = String::from_utf8_lossy(&output.stdout);

        for i in config.lines() {
            if i.starts_with("#") {
                continue;
            }

            let Some((k, v)) = i.split_once('=') else {
                continue;
            };

            if k.trim() == "CONFIG_TMPFS_XATTR" && v.trim() == "y" {
                return Ok(true);
            }
        }

        Ok(false)
    }
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    Ok(true)
}

fn guess_context_by_path(path: &Path) -> &'static str {
    let path_str = path.to_string_lossy();

    if path_str.starts_with("/vendor") || path_str.starts_with("/odm") {
        if path_str.contains("/lib/") || path_str.contains("/lib64/") || path_str.ends_with(".so") {
            return CONTEXT_HAL;
        }

        if path_str.contains("/bin/") {
            return CONTEXT_VENDOR_EXEC;
        }

        if path_str.contains("/firmware") {
            return CONTEXT_VENDOR;
        }

        return CONTEXT_VENDOR;
    }

    CONTEXT_SYSTEM
}

fn apply_system_context(current: &Path, relative: &Path) -> Result<()> {
    if let Some(name) = current.file_name().and_then(|n| n.to_str())
        && (name == "upperdir" || name == "workdir")
        && let Some(parent) = current.parent()
        && let Ok(ctx) = lgetfilecon(parent)
    {
        return lsetfilecon(current, &ctx);
    }

    let current_ctx = lgetfilecon(current).ok();
    if let Some(ctx) = &current_ctx
        && !ctx.is_empty()
        && ctx != CONTEXT_ROOTFS
        && ctx != "u:object_r:unlabeled:s0"
    {
        return Ok(());
    }

    let system_path = Path::new("/").join(relative);
    if system_path.exists() {
        if let Ok(sys_ctx) = lgetfilecon(&system_path) {
            let target_ctx = if sys_ctx == CONTEXT_ROOTFS {
                CONTEXT_SYSTEM
            } else {
                &sys_ctx
            };
            return lsetfilecon(current, target_ctx);
        }
    } else if let Some(parent) = system_path.parent()
        && parent.exists()
        && let Ok(parent_ctx) = lgetfilecon(parent)
        && parent_ctx != CONTEXT_ROOTFS
    {
        let guessed = guess_context_by_path(&system_path);
        if guessed == CONTEXT_HAL && parent_ctx == CONTEXT_VENDOR {
            return lsetfilecon(current, CONTEXT_HAL);
        }
        return lsetfilecon(current, &parent_ctx);
    }

    let target_context = guess_context_by_path(&system_path);
    lsetfilecon(current, target_context)
}

pub(crate) fn internal_copy_extended_attributes(src: &Path, dst: &Path) -> Result<()> {
    copy_extended_attributes(src, dst)
}

pub(crate) fn internal_apply_system_context(current: &Path, relative: &Path) -> Result<()> {
    apply_system_context(current, relative)
}
