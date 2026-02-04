use std::path::Path;
#[cfg(any(target_os = "linux", target_os = "android"))]
use std::{os::unix::ffi::OsStrExt, process::Command};

use anyhow::{Context, Result};
#[cfg(any(target_os = "linux", target_os = "android"))]
use extattr::{Flags as XattrFlags, lgetxattr, llistxattr, lsetxattr};

const SELINUX_XATTR: &str = "security.selinux";
const OVERLAY_OPAQUE_XATTR: &str = "trusted.overlay.opaque";

#[cfg(any(target_os = "linux", target_os = "android"))]
fn copy_extended_attributes(src: &Path, dst: &Path) -> Result<()> {
    if let Ok(ctx) = lgetfilecon(src) {
        let _ = lsetfilecon(dst, &ctx);
    }

    if let Ok(opaque) = lgetxattr(src, OVERLAY_OPAQUE_XATTR) {
        let _ = lsetxattr(dst, OVERLAY_OPAQUE_XATTR, &opaque, XattrFlags::empty());
    }
    if let Ok(xattrs) = llistxattr(src) {
        for xattr_name in xattrs {
            let name_bytes = xattr_name.as_bytes();
            let name_str = String::from_utf8_lossy(name_bytes);

            if name_str.starts_with("trusted.overlay.")
                && name_str != OVERLAY_OPAQUE_XATTR
                && let Ok(val) = lgetxattr(src, &xattr_name)
            {
                let _ = lsetxattr(dst, &xattr_name, &val, XattrFlags::empty());
            }
        }
    }
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn copy_extended_attributes(_src: &Path, _dst: &Path) -> Result<()> {
    unimplemented!();
}

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
    unimplemented!();
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn lsetfilecon<P: AsRef<Path>>(path: P, con: &str) -> Result<()> {
    if let Err(e) = lsetxattr(
        path.as_ref(),
        SELINUX_XATTR,
        con.as_bytes(),
        XattrFlags::empty(),
    ) {
        let _ = e;
    }
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn lsetfilecon<P: AsRef<Path>>(_path: P, _con: &str) -> Result<()> {
    unimplemented!();
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

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn is_overlay_xattr_supported() -> Result<bool> {
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
pub fn is_overlay_xattr_supported() -> Result<bool> {
    unimplemented!();
}

pub fn internal_copy_extended_attributes(src: &Path, dst: &Path) -> Result<()> {
    copy_extended_attributes(src, dst)
}
