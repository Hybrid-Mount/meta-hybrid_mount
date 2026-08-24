// ReHybrid-Mount
//
// SPDX-License-Identifier: GPL-3.0-only

//! 文件系统辅助:路径清理、内核配置读取、tmpfs xattr 能力探测。
//!
//! Stage 3 脚手架:入口在 Stage 5 CLI 接入前暂未被二进制入口使用;
//! 接入完成后移除本豁免,恢复 dead_code 检查。
#![allow(dead_code)]

use std::path::Path;

#[cfg(any(target_os = "linux", target_os = "android"))]
use std::ffi::CString;
#[cfg(unix)]
use std::fs::{self, OpenOptions};
#[cfg(unix)]
use std::io::Write;
#[cfg(any(target_os = "linux", target_os = "android"))]
use std::os::unix::ffi::OsStrExt;
#[cfg(any(target_os = "linux", target_os = "android"))]
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt, symlink};
#[cfg(unix)]
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

#[cfg(any(target_os = "linux", target_os = "android"))]
use rustix::fs::{CWD, FileType, Gid, Mode, Uid, chown, mknodat};
#[cfg(any(target_os = "linux", target_os = "android"))]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::errors::Error;
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

#[cfg(any(target_os = "linux", target_os = "android"))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CopyTreeStats {
    pub directories: usize,
    pub files: usize,
    pub symlinks: usize,
    pub special_entries: usize,
    pub opaque_directories: usize,
    pub bytes: u64,
}

/// Copy a module tree onto the selected overlay storage without following
/// symlinks.  `.replace` markers become OverlayFS opaque xattrs, matching the
/// prepared-tree behavior used by v4.2.0.
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn copy_module_tree(source: &Path, destination: &Path) -> Result<CopyTreeStats> {
    remove_path(destination)?;
    let mut stats = CopyTreeStats::default();
    copy_tree_entry(source, destination, &mut stats)?;
    Ok(stats)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn copy_tree_entry(source: &Path, destination: &Path, stats: &mut CopyTreeStats) -> Result<()> {
    let metadata = fs::symlink_metadata(source)?;
    let file_type = metadata.file_type();

    if file_type.is_dir() {
        fs::create_dir_all(destination)?;
        stats.directories += 1;

        for entry in fs::read_dir(source)? {
            let entry = entry?;
            if entry
                .file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case(crate::defs::REPLACE_DIR_FILE_NAME)
            {
                set_overlay_opaque(destination)?;
                stats.opaque_directories += 1;
                continue;
            }
            copy_tree_entry(&entry.path(), &destination.join(entry.file_name()), stats)?;
        }

        clone_entry_metadata(source, destination, &metadata, false);
        return Ok(());
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }

    if file_type.is_symlink() {
        symlink(fs::read_link(source)?, destination)?;
        stats.symlinks += 1;
    } else if file_type.is_file() {
        fs::copy(source, destination)?;
        stats.files += 1;
        stats.bytes = stats.bytes.saturating_add(metadata.len());
    } else if file_type.is_char_device() || file_type.is_block_device() || file_type.is_fifo() {
        make_device_node(destination, &metadata)?;
        stats.special_entries += 1;
    } else {
        return Err(Error::msg(format!(
            "unsupported module entry type: {}",
            source.display()
        )));
    }

    clone_entry_metadata(source, destination, &metadata, file_type.is_symlink());
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn set_overlay_opaque(path: &Path) -> Result<()> {
    extattr::lsetxattr(
        path,
        crate::defs::REPLACE_DIR_XATTR,
        b"y",
        extattr::Flags::empty(),
    )
    .map_err(|err| {
        Error::msg(format!(
            "set overlay opaque xattr on {}: {err}",
            path.display()
        ))
    })?;
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn make_device_node(path: &Path, metadata: &fs::Metadata) -> Result<()> {
    let raw_mode = metadata.permissions().mode();
    let file_type = FileType::from_raw_mode(raw_mode);
    if matches!(file_type, FileType::Unknown) {
        return Err(Error::msg(format!(
            "cannot recreate special module entry {}: unknown type",
            path.display()
        )));
    }

    mknodat(
        CWD,
        path,
        file_type,
        Mode::from_raw_mode(raw_mode & 0o7777),
        metadata.rdev() as _,
    )?;
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn clone_entry_metadata(
    source: &Path,
    destination: &Path,
    metadata: &fs::Metadata,
    is_symlink: bool,
) {
    if !is_symlink && let Err(err) = fs::set_permissions(destination, metadata.permissions()) {
        log::warn!(
            "copy metadata permissions skipped: src={}, dst={}, error={err}",
            source.display(),
            destination.display()
        );
    }

    let ownership_result = if is_symlink {
        match CString::new(destination.as_os_str().as_bytes()) {
            Ok(path) => {
                let result = unsafe {
                    libc::lchown(
                        path.as_ptr(),
                        metadata.uid() as libc::uid_t,
                        metadata.gid() as libc::gid_t,
                    )
                };
                if result == 0 {
                    Ok(())
                } else {
                    Err(std::io::Error::last_os_error())
                }
            }
            Err(err) => Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, err)),
        }
    } else {
        chown(
            destination,
            Some(Uid::from_raw(metadata.uid())),
            Some(Gid::from_raw(metadata.gid())),
        )
        .map_err(std::io::Error::from)
    };

    if let Err(err) = ownership_result {
        log::warn!(
            "copy metadata ownership skipped: src={}, dst={}, uid={}, gid={}, error={err}",
            source.display(),
            destination.display(),
            metadata.uid(),
            metadata.gid()
        );
    }

    if let Ok(context) = crate::utils::lgetfilecon(source)
        && let Err(err) = crate::utils::lsetfilecon(destination, &context)
    {
        log::warn!(
            "copy metadata SELinux context skipped: src={}, dst={}, error={err}",
            source.display(),
            destination.display()
        );
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
