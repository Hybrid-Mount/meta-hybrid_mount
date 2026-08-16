// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::path::Path;
#[cfg(all(
    feature = "control-plane",
    any(target_os = "linux", target_os = "android")
))]
use std::sync::atomic::AtomicBool;

#[cfg(any(target_os = "linux", target_os = "android"))]
use anyhow::Context;
use anyhow::Result;
#[cfg(any(target_os = "linux", target_os = "android"))]
use extattr::{Flags as XattrFlags, lsetxattr};

#[cfg(any(target_os = "linux", target_os = "android"))]
const SELINUX_XATTR: &str = "security.selinux";
#[cfg(any(target_os = "linux", target_os = "android"))]
const OVERLAY_OPAQUE_XATTR: &str = "trusted.overlay.opaque";
#[cfg(all(
    feature = "control-plane",
    any(target_os = "linux", target_os = "android")
))]
static TMPFS_XATTR_SUPPORTED: AtomicBool = AtomicBool::new(false);

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn set_overlay_opaque<P: AsRef<Path>>(path: P) -> Result<()> {
    lsetxattr(
        path.as_ref(),
        OVERLAY_OPAQUE_XATTR,
        b"y",
        XattrFlags::empty(),
    )?;
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn set_overlay_opaque<P: AsRef<Path>>(_path: P) -> Result<()> {
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn lsetfilecon<P: AsRef<Path>>(path: P, con: &str) -> Result<()> {
    lsetxattr(
        path.as_ref(),
        SELINUX_XATTR,
        con.as_bytes(),
        XattrFlags::empty(),
    )
    .with_context(|| {
        format!(
            "Failed to set SELinux context for {} to {}",
            path.as_ref().display(),
            con
        )
    })?;
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn lsetfilecon<P: AsRef<Path>>(_path: P, _con: &str) -> Result<()> {
    anyhow::bail!("SELinux context writes are only supported on linux/android");
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
    anyhow::bail!("SELinux context reads are only supported on linux/android");
}

#[cfg(all(
    feature = "control-plane",
    any(target_os = "linux", target_os = "android")
))]
pub fn is_overlay_xattr_supported() -> Result<bool> {
    if TMPFS_XATTR_SUPPORTED.load(std::sync::atomic::Ordering::Relaxed) {
        return Ok(true);
    }

    let supported = super::check_kernel_config("CONFIG_TMPFS_XATTR")?;

    TMPFS_XATTR_SUPPORTED.store(supported, std::sync::atomic::Ordering::Relaxed);

    Ok(supported)
}

#[cfg(all(
    feature = "control-plane",
    not(any(target_os = "linux", target_os = "android"))
))]
pub fn is_overlay_xattr_supported() -> Result<bool> {
    Ok(false)
}
