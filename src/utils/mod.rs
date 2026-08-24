// SPDX-License-Identifier: GPL-3.0-only

//! 通用工具:模块 ID 校验、目录创建、SELinux 上下文读写。

use std::path::Path;

use crate::defs;
use crate::errors::{Error, Result};

/// 校验 module id:`^[a-zA-Z][a-zA-Z0-9._-]+$`(参考项目行为)。
pub fn validate_module_id(module_id: &str) -> Result<()> {
    let mut chars = module_id.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() => {}
        _ => {
            return Err(Error::InvalidModuleID {
                module_id: module_id.to_owned(),
            });
        }
    }

    if chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' || ch == '-') {
        Ok(())
    } else {
        Err(Error::InvalidModuleID {
            module_id: module_id.to_owned(),
        })
    }
}

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

/// 设置路径的 SELinux 上下文。
#[cfg(unix)]
pub fn lsetfilecon(path: &Path, context: &str) -> Result<()> {
    log::debug!("file: {}, con: {context}", path.display());
    extattr::lsetxattr(path, defs::SELINUX_XATTR, context, extattr::Flags::empty()).map_err(|err| {
        Error::msg(format!(
            "failed to change SELinux context for {}: {err}",
            path.display()
        ))
    })
}

/// 读取路径的 SELinux 上下文。
#[cfg(unix)]
pub fn lgetfilecon(path: &Path) -> Result<String> {
    let context = extattr::lgetxattr(path, defs::SELINUX_XATTR).map_err(|err| {
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
    fn valid_module_ids_accepted() {
        for id in ["ab", "a-b.c_d", "Z9", "module_name"] {
            assert!(validate_module_id(id).is_ok(), "{id}");
        }
    }

    #[test]
    fn invalid_module_ids_rejected() {
        for id in ["", "1abc", "-abc", "_abc", ".abc", "ab/c", "ab cd"] {
            assert!(validate_module_id(id).is_err(), "{id}");
        }
    }

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
