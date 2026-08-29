// SPDX-License-Identifier: GPL-3.0-only

//! 通用工具:模块 ID 校验、目录创建、SELinux 上下文读写。

#[cfg(any(target_os = "linux", target_os = "android"))]
use std::io;
use std::path::Path;

#[cfg(any(target_os = "linux", target_os = "android"))]
use rustix::buffer::spare_capacity;
#[cfg(any(target_os = "linux", target_os = "android"))]
use rustix::fs::{XattrFlags, lgetxattr, lsetxattr};

use crate::defs;
use crate::errors::{Error, Result};

/// 创建目录并确认结果是目录(参考项目 `ensure_dir_exists` 行为)。
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

/// 读取路径的扩展属性：先查询长度，再按长度分配并一次读取，
/// 兼容长 SELinux context，不依赖固定栈缓冲。
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

/// 设置路径的扩展属性，不跟随最终路径组件的符号链接。
#[cfg(any(target_os = "linux", target_os = "android"))]
pub(crate) fn write_xattr(path: &Path, name: &str, value: &[u8]) -> io::Result<()> {
    Ok(lsetxattr(path, name, value, XattrFlags::empty())?)
}

/// 设置路径的 SELinux 上下文。
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

/// 读取路径的 SELinux 上下文。
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

/// 是否命中“不注册尝试卸载”的分区(v4.2.0 pairip 规避行为)。
pub fn is_ignored_unmount_partition(path: &str) -> bool {
    defs::IGNORE_UNMOUNT_PARTITIONS.iter().any(|ignored| {
        let ignored = ignored.trim_end_matches('/');
        path == ignored
            || path
                .strip_prefix(ignored)
                .is_some_and(|rest| rest.starts_with('/'))
    })
}

/// KernelSU 尝试卸载列表集成(仅 Linux/Android)。
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
            });
            if unsupported {
                eprintln!(
                    "skipping long xattr test: filesystem does not support user xattrs: {err}"
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
