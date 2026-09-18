// SPDX-License-Identifier: GPL-3.0-only

//! Shared utilities: module id validation, directory creation and SELinux context I/O.

#[cfg(any(target_os = "linux", target_os = "android"))]
use std::io;
use std::path::Path;

#[cfg(any(target_os = "linux", target_os = "android"))]
use rustix::buffer::spare_capacity;
#[cfg(any(target_os = "linux", target_os = "android"))]
use rustix::fs::{XattrFlags, lgetxattr, lsetxattr};

use crate::defs;
use crate::errors::{Error, Result};

pub fn ensure_dir_exists(dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    if dir.is_dir() {
        Ok(())
    } else {
        Err(Error::RegularDirectory {
            path: dir.display().to_string(),
        })
    }
}

/// Reads a path's extended attribute: query the length, then allocate and read it in
/// one pass, which handles long SELinux contexts without a fixed stack buffer.
#[cfg(any(target_os = "linux", target_os = "android"))]
pub(crate) fn read_xattr(path: &Path, name: &str) -> io::Result<Vec<u8>> {
    let mut empty = [0_u8; 0];
    let size = lgetxattr(path, name, &mut empty)?;
    if size == 0 {
        return Ok(Vec::new());
    }

    let mut value = Vec::with_capacity(size);
    let filled = lgetxattr(path, name, spare_capacity(&mut value))?;
    value.truncate(filled);
    Ok(value)
}

/// Sets a path's extended attribute without following a symlink in the final component.
#[cfg(any(target_os = "linux", target_os = "android"))]
pub(crate) fn write_xattr(path: &Path, name: &str, value: &[u8]) -> io::Result<()> {
    Ok(lsetxattr(path, name, value, XattrFlags::empty())?)
}

/// Sets a path's SELinux context.
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn lsetfilecon(path: &Path, context: &str) -> Result<()> {
    log::debug!("file: {}, con: {context}", path.display());
    write_xattr(path, defs::SELINUX_XATTR, context.as_bytes()).map_err(|err| {
        Error::msg(format!(
            "failed to change SELinux context for {}: {err}",
            path.display()
        ))
    })
}

/// Reads a path's SELinux context.
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn lgetfilecon(path: &Path) -> Result<String> {
    let context = read_xattr(path, defs::SELINUX_XATTR).map_err(|err| {
        Error::msg(format!(
            "failed to get SELinux context for {}: {err}",
            path.display()
        ))
    })?;
    Ok(String::from_utf8_lossy(&context).to_string())
}

/// Whether `path` is `prefix` itself or lies below it on a path-segment boundary.
///
/// A plain `starts_with` is wrong here: `/system_ext` is not under `/system`. Rule
/// matching, try-umount partition filtering and sub-mount collection all need that
/// segment check, so they share this implementation.
pub fn is_same_or_below(path: &str, prefix: &str) -> bool {
    path == prefix
        || path
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// Whether the path falls in a partition excluded from try-umount registration (v4.2.0 pairip workaround).
pub fn is_ignored_unmount_partition(path: &str) -> bool {
    defs::IGNORE_UNMOUNT_PARTITIONS
        .iter()
        .any(|ignored| is_same_or_below(path, ignored.trim_end_matches('/')))
}

/// KernelSU try-umount list integration (Linux/Android only).
#[cfg(any(target_os = "linux", target_os = "android"))]
pub mod ksu;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_dir_exists_creates_missing_parents() {
        let dir =
            std::env::temp_dir().join(format!("hybrid-mount-ensure-dir-{}", std::process::id()));
        let nested = dir.join("a").join("b");

        ensure_dir_exists(&nested).unwrap();
        assert!(nested.is_dir());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn same_or_below_matches_only_on_segment_boundaries() {
        assert!(is_same_or_below("/system", "/system"));
        assert!(is_same_or_below("/system/etc", "/system"));
        assert!(is_same_or_below("/system/etc/hosts", "/system"));

        // A shared string prefix is not a path prefix.
        assert!(!is_same_or_below("/system_ext", "/system"));
        assert!(!is_same_or_below("/systemd", "/system"));
        assert!(!is_same_or_below("/systematic/etc", "/system"));
        assert!(!is_same_or_below("/", "/system"));
        assert!(!is_same_or_below("/system", "/system/etc"));
    }

    #[test]
    fn ignored_unmount_partition_matches_exact_path() {
        assert!(is_ignored_unmount_partition("/system/lib"));
        assert!(is_ignored_unmount_partition("/system/lib64"));
        assert!(is_ignored_unmount_partition("/vendor/lib"));
        assert!(is_ignored_unmount_partition("/vendor/lib64"));
    }

    #[test]
    fn ignored_unmount_partition_matches_descendants() {
        assert!(is_ignored_unmount_partition("/system/lib64/foo"));
        assert!(is_ignored_unmount_partition("/vendor/lib/arm/libfoo.so"));
    }

    #[test]
    fn ignored_unmount_partition_rejects_shared_prefix_siblings() {
        assert!(!is_ignored_unmount_partition("/system/lib_extra"));
        assert!(!is_ignored_unmount_partition("/system/lib64_other"));
        assert!(!is_ignored_unmount_partition("/vendor/library"));
        assert!(!is_ignored_unmount_partition("/product"));
        assert!(!is_ignored_unmount_partition("/system/etc"));
        assert!(!is_ignored_unmount_partition(
            "/data/adb/hybrid-mount/run/staging_x"
        ));
    }
}

#[cfg(all(test, target_os = "linux"))]
mod linux_tests {
    use super::*;

    #[test]
    fn xattr_roundtrip_handles_long_contexts() {
        let dir = std::env::temp_dir().join(format!("hybrid-mount-xattr-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("probe");
        std::fs::write(&path, b"data").unwrap();

        let long_value = vec![b'x'; 16 * 1024];
        if let Err(err) = write_xattr(&path, "user.hybrid_mount_test", &long_value) {
            let unsupported = err.raw_os_error().is_some_and(|code| {
                code == rustix::io::Errno::NOTSUP.raw_os_error()
                    || code == rustix::io::Errno::OPNOTSUPP.raw_os_error()
                    || code == rustix::io::Errno::PERM.raw_os_error()
                    || code == rustix::io::Errno::NOSPC.raw_os_error()
            });
            if unsupported {
                eprintln!(
                    "skipping long xattr test: filesystem cannot store the requested xattr: {err}"
                );
                let _ = std::fs::remove_dir_all(&dir);
                return;
            }
            panic!("set long xattr failed: {err}");
        }

        let read_back = read_xattr(&path, "user.hybrid_mount_test").unwrap();
        assert_eq!(read_back, long_value);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
