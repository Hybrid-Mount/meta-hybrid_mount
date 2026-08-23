// ReHybrid-Mount
//
// SPDX-License-Identifier: GPL-3.0-only

//! 文件系统辅助:路径清理、内核配置读取、tmpfs xattr 能力探测。
//!
//! Stage 3 脚手架:入口在 Stage 5 CLI 接入前暂未被二进制入口使用;
//! 接入完成后移除本豁免,恢复 dead_code 检查。
#![allow(dead_code)]

use std::path::Path;

#[cfg(unix)]
use std::fs::{self, OpenOptions};
#[cfg(unix)]
use std::io::Write;
#[cfg(unix)]
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

#[cfg(any(target_os = "linux", target_os = "android"))]
use std::sync::atomic::{AtomicBool, Ordering};

use crate::errors::Result;

#[cfg(unix)]
static ATOMIC_WRITE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Replace a file without exposing a truncated intermediate state.
#[cfg(unix)]
pub fn atomic_write(path: &Path, content: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file");
    let sequence = ATOMIC_WRITE_SEQUENCE.fetch_add(1, AtomicOrdering::Relaxed);
    let temporary = parent.join(format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        sequence
    ));

    let result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)?;
        file.write_all(content)?;
        file.sync_all()?;

        if let Ok(metadata) = fs::metadata(path) {
            fs::set_permissions(&temporary, metadata.permissions())?;
        }

        fs::rename(&temporary, path)?;
        if let Ok(parent_dir) = fs::File::open(parent) {
            let _ = parent_dir.sync_all();
        }
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(not(unix))]
pub fn atomic_write(path: &Path, content: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)?;
    Ok(())
}

/// 删除路径:目录递归删除,非目录直接删除,不存在视为成功。
pub fn remove_path(path: &Path) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_dir() => Ok(std::fs::remove_dir_all(path)?),
        Ok(_) => Ok(std::fs::remove_file(path)?),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.into()),
    }
}

/// 读取 `/proc/config.gz`,检查 `CONFIG_*` 是否编译为 `y`(v4.2.0 行为)。
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn check_kernel_config(key: &str) -> Result<bool> {
    use std::io::Read;

    use flate2::read::GzDecoder;

    let file = std::fs::File::open("/proc/config.gz")?;
    let mut config = String::new();
    GzDecoder::new(file).read_to_string(&mut config)?;

    let found = config.lines().any(|line| {
        if line.starts_with('#') {
            return false;
        }
        let Some((name, value)) = line.split_once('=') else {
            return false;
        };
        name.trim() == key && value.trim() == "y"
    });

    Ok(found)
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn check_kernel_config(_key: &str) -> Result<bool> {
    Ok(false)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
static TMPFS_XATTR_SUPPORTED: AtomicBool = AtomicBool::new(false);

/// overlay 层落到 tmpfs 时要求 tmpfs 支持 xattr;结果缓存一次(v4.2.0 行为)。
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn is_overlay_xattr_supported() -> Result<bool> {
    if TMPFS_XATTR_SUPPORTED.load(Ordering::Relaxed) {
        return Ok(true);
    }

    let supported = check_kernel_config("CONFIG_TMPFS_XATTR")?;
    TMPFS_XATTR_SUPPORTED.store(supported, Ordering::Relaxed);
    Ok(supported)
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn is_overlay_xattr_supported() -> Result<bool> {
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remove_path_handles_missing_and_files() {
        let dir =
            std::env::temp_dir().join(format!("rehybrid-mount-remove-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("a.txt");
        std::fs::write(&file, "x").unwrap();

        remove_path(&dir.join("missing")).unwrap();
        remove_path(&file).unwrap();
        assert!(!file.exists());
        remove_path(&dir).unwrap();
        assert!(!dir.exists());
    }
}
