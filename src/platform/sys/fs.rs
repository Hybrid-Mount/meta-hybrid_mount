// SPDX-License-Identifier: GPL-3.0-only

//! Filesystem helpers: path cleanup, kernel config reads and tmpfs xattr probing.

use std::path::Path;

#[cfg(unix)]
use std::fs::{self, OpenOptions};
#[cfg(unix)]
use std::io::{self, Write};
#[cfg(any(target_os = "linux", target_os = "android"))]
use std::os::unix::fs::{MetadataExt, PermissionsExt};
#[cfg(unix)]
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

#[cfg(any(target_os = "linux", target_os = "android"))]
use rustix::fs::{AtFlags, CWD, FileType, Gid, Mode, Uid, chown, chownat, mknodat};
#[cfg(any(target_os = "linux", target_os = "android"))]
use std::sync::atomic::{AtomicU8, Ordering};

#[cfg(unix)]
use crate::errors::Error;
use crate::errors::Result;

#[cfg(unix)]
static ATOMIC_WRITE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Persist the directory entry that names `path`.
#[cfg(unix)]
pub fn sync_parent_directory(path: &Path) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::File::open(parent)?.sync_all()
}

#[cfg(not(unix))]
pub fn sync_parent_directory(_path: &Path) -> std::io::Result<()> {
    // Non-Unix hosts have no directory-entry sync; release targets always take the path above.
    Ok(())
}

/// Replace a file without exposing a truncated intermediate state.
#[cfg(unix)]
pub fn atomic_write(path: &Path, content: &[u8]) -> Result<()> {
    atomic_write_with_sequence(path, content, &ATOMIC_WRITE_SEQUENCE)
}

#[cfg(unix)]
fn atomic_write_with_sequence(path: &Path, content: &[u8], sequence: &AtomicU64) -> Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file");
    let mut attempts = 0;
    let (temporary, mut file) = loop {
        let next = sequence.fetch_add(1, AtomicOrdering::Relaxed);
        let temporary = parent.join(format!(".{file_name}.{}.{next}.tmp", std::process::id()));
        attempts += 1;
        match OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
        {
            Ok(file) => break (temporary, file),
            // A reused PID can collide with crash residue. Only clean up a
            // temporary file after this call has created it successfully.
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists && attempts < 32 => continue,
            Err(err) => return Err(err.into()),
        }
    };

    let result = (|| -> Result<()> {
        file.write_all(content)?;

        match fs::metadata(path) {
            Ok(metadata) => fs::set_permissions(&temporary, metadata.permissions())?,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(err.into()),
        }
        file.sync_all()?;

        fs::rename(&temporary, path)?;
        // A failed parent-directory fsync counts as a failed save. The rename is already visible,
        // but the caller must know the directory entry is not durable and cannot call it an atomic save.
        sync_parent_directory(path)?;
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

/// Removes regular temp files older than 24 hours that match the atomic-write naming scheme.
/// Files belonging to the current process may still be in flight and are kept.
#[cfg(unix)]
pub fn cleanup_stale_atomic_temp_files(dir: &Path) -> Result<()> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => {
            return Err(Error::msg(format!(
                "read directory for temp file cleanup {}: {err}",
                dir.display()
            )));
        }
    };

    let current_pid = std::process::id();
    let stale_threshold = std::time::SystemTime::now()
        .checked_sub(std::time::Duration::from_secs(86400)) // 24 小时
        .unwrap_or(std::time::UNIX_EPOCH);

    let mut cleaned = 0;
    let mut failed = 0;

    for entry in entries {
        let entry = entry?;
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else {
            continue;
        };

        let Some(stem) = name
            .strip_prefix('.')
            .and_then(|name| name.strip_suffix(".tmp"))
        else {
            continue;
        };
        let mut parts = stem.rsplitn(3, '.');
        let (Some(sequence), Some(pid), Some(original_name)) =
            (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        if original_name.is_empty()
            || !pid.bytes().all(|byte| byte.is_ascii_digit())
            || !sequence.bytes().all(|byte| byte.is_ascii_digit())
            || sequence.parse::<u64>().is_err()
        {
            continue;
        }
        let Ok(pid) = pid.parse::<u32>() else {
            continue;
        };
        if pid == current_pid {
            continue;
        }
        let metadata = entry.metadata()?;
        if metadata.is_file() && metadata.modified()? < stale_threshold {
            match fs::remove_file(entry.path()) {
                Ok(()) => {
                    log::info!(
                        "cleaned up stale atomic temp file: {}",
                        entry.path().display()
                    );
                    cleaned += 1;
                }
                Err(err) => {
                    log::warn!(
                        "failed to remove stale atomic temp file {}: {err}",
                        entry.path().display()
                    );
                    failed += 1;
                }
            }
        }
    }

    if cleaned > 0 {
        log::info!(
            "atomic temp file cleanup: cleaned={}, failed={}, dir={}",
            cleaned,
            failed,
            dir.display()
        );
    }

    Ok(())
}

#[cfg(not(unix))]
pub fn atomic_write(path: &Path, content: &[u8]) -> Result<()> {
    // Non-atomic fallback for host tests and development: Windows offers no crash-safe
    // temp-file + rename, and release targets (Android/Linux) always take the path above.
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)?;
    Ok(())
}

#[cfg(not(unix))]
pub fn cleanup_stale_atomic_temp_files(_dir: &Path) -> Result<()> {
    // Nothing to clean up on non-Unix platforms.
    Ok(())
}

/// Deletes a path: directories recursively, anything else directly, and a missing path counts as success.
pub fn remove_path(path: &Path) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_dir() => Ok(std::fs::remove_dir_all(path)?),
        Ok(_) => Ok(std::fs::remove_file(path)?),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.into()),
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub(crate) fn make_device_node(path: &Path, metadata: &fs::Metadata) -> Result<()> {
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
pub(crate) fn clone_entry_metadata(
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
        chownat(
            CWD,
            destination,
            Some(Uid::from_raw(metadata.uid())),
            Some(Gid::from_raw(metadata.gid())),
            AtFlags::SYMLINK_NOFOLLOW,
        )
        .map_err(std::io::Error::from)
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

    if let Ok(context) = crate::platform::lgetfilecon(source)
        && let Err(err) = crate::platform::lsetfilecon(destination, &context)
    {
        log::warn!(
            "copy metadata SELinux context skipped: src={}, dst={}, error={err}",
            source.display(),
            destination.display()
        );
    }
}

/// Make an OverlayFS layer root behave like the stock directory it will cover.
/// Unlike ordinary module-entry staging, every part of this metadata is
/// required: a private 0700 directory or a wrongly labeled OverlayFS root can
/// make an entire Android partition inaccessible after the mount.
#[cfg(unix)]
fn directory_metadata(source: &Path) -> Result<fs::Metadata> {
    // Follow the final symlink: Android partition layouts may expose a stock
    // overlay target such as /system/media as a symlink to the real directory.
    let metadata = fs::metadata(source)?;
    if !metadata.file_type().is_dir() {
        return Err(Error::msg(format!(
            "overlay layer metadata source is not a directory: {}",
            source.display()
        )));
    }
    Ok(metadata)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn clone_directory_metadata(source: &Path, destination: &Path) -> Result<()> {
    let metadata = directory_metadata(source)?;

    chown(
        destination,
        Some(Uid::from_raw(metadata.uid())),
        Some(Gid::from_raw(metadata.gid())),
    )
    .map_err(|err| {
        Error::msg(format!(
            "copy overlay layer ownership {} -> {}: {err}",
            source.display(),
            destination.display()
        ))
    })?;
    fs::set_permissions(destination, metadata.permissions()).map_err(|err| {
        Error::msg(format!(
            "copy overlay layer permissions {} -> {}: {err}",
            source.display(),
            destination.display()
        ))
    })?;

    let context = crate::platform::getfilecon(source)?;
    crate::platform::lsetfilecon(destination, &context).map_err(|err| {
        Error::msg(format!(
            "copy overlay layer SELinux context {} -> {}: {err}",
            source.display(),
            destination.display()
        ))
    })?;

    log::debug!(
        "overlay layer directory metadata cloned: src={}, dst={}, mode={:o}, uid={}, gid={}, context={}",
        source.display(),
        destination.display(),
        metadata.permissions().mode() & 0o7777,
        metadata.uid(),
        metadata.gid(),
        context
    );
    Ok(())
}

/// `/proc/config.gz` decompressed once per process; a failed read is not cached, so
/// the original I/O error is reported again on the next caller.
#[cfg(any(target_os = "linux", target_os = "android"))]
static KERNEL_CONFIG: std::sync::OnceLock<String> = std::sync::OnceLock::new();

#[cfg(any(target_os = "linux", target_os = "android"))]
fn kernel_config() -> Result<&'static str> {
    use std::io::Read;

    use flate2::read::GzDecoder;

    if let Some(config) = KERNEL_CONFIG.get() {
        return Ok(config);
    }

    let file = std::fs::File::open("/proc/config.gz")?;
    let mut config = String::new();
    GzDecoder::new(file).read_to_string(&mut config)?;
    Ok(KERNEL_CONFIG.get_or_init(|| config))
}

/// Reads `/proc/config.gz` and checks whether a `CONFIG_*` is built in as `y` (v4.2.0 behaviour).
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn check_kernel_config(key: &str) -> Result<bool> {
    let config = kernel_config()?;

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
static TMPFS_XATTR_SUPPORT: AtomicU8 = AtomicU8::new(0);

/// An overlay layer on tmpfs needs tmpfs xattr support; the result is cached once (v4.2.0 behaviour).
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn is_overlay_xattr_supported() -> Result<bool> {
    match TMPFS_XATTR_SUPPORT.load(Ordering::Relaxed) {
        1 => return Ok(false),
        2 => return Ok(true),
        _ => {}
    }

    let supported = check_kernel_config("CONFIG_TMPFS_XATTR")?;
    TMPFS_XATTR_SUPPORT.store(if supported { 2 } else { 1 }, Ordering::Relaxed);
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
        let dir = crate::test_support::Fixture::new("remove");
        let file = dir.join("a.txt");
        std::fs::write(&file, "x").unwrap();

        remove_path(&dir.join("missing")).unwrap();
        remove_path(&file).unwrap();
        assert!(!file.exists());
        remove_path(&dir).unwrap();
        assert!(!dir.exists());
    }

    #[test]
    fn atomic_write_creates_and_replaces_content() {
        let dir = crate::test_support::Fixture::new("atomic");
        let path = dir.join("state.txt");

        atomic_write(&path, b"first").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"first");
        atomic_write(&path, b"second").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"second");

        let leftovers = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(std::result::Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
            .count();
        assert_eq!(leftovers, 0);
    }

    #[cfg(unix)]
    #[test]
    fn atomic_write_retries_collisions_without_removing_existing_entries() {
        use std::os::unix::fs::symlink;

        let dir = crate::test_support::Fixture::new("collision");
        let target = dir.join("state.json");
        let candidate =
            |sequence| dir.join(format!(".state.json.{}.{sequence}.tmp", std::process::id()));
        fs::write(candidate(0), b"residue").unwrap();
        fs::create_dir(candidate(1)).unwrap();
        symlink(dir.join("missing"), candidate(2)).unwrap();

        atomic_write_with_sequence(&target, b"new state", &AtomicU64::new(0)).unwrap();

        assert_eq!(fs::read(&target).unwrap(), b"new state");
        assert_eq!(fs::read(candidate(0)).unwrap(), b"residue");
        assert!(candidate(1).is_dir());
        assert!(candidate(2).is_symlink());
        assert!(!candidate(3).exists());
    }

    #[cfg(unix)]
    #[test]
    fn atomic_write_bounds_collision_retries_and_preserves_original_content() {
        let dir = crate::test_support::Fixture::new("collision-limit");
        let target = dir.join("state.json");
        fs::write(&target, b"original").unwrap();
        for sequence in 0..32 {
            fs::write(
                dir.join(format!(".state.json.{}.{sequence}.tmp", std::process::id())),
                b"residue",
            )
            .unwrap();
        }

        let err =
            atomic_write_with_sequence(&target, b"replacement", &AtomicU64::new(0)).unwrap_err();

        let Error::Io(source) = err else {
            panic!("collision must return an I/O error");
        };
        assert_eq!(source.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&target).unwrap(), b"original");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 33);
    }

    #[cfg(unix)]
    #[test]
    fn atomic_write_preserves_existing_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = crate::test_support::Fixture::new("atomic-mode");
        let target = dir.join("config.toml");
        fs::write(&target, b"original").unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();

        atomic_write(&target, b"replacement").unwrap();

        assert_eq!(
            fs::metadata(&target).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[cfg(unix)]
    #[test]
    fn atomic_write_removes_temp_file_when_rename_fails() {
        let dir = crate::test_support::Fixture::new("atomic-fail");
        let target = dir.join("target");
        std::fs::create_dir_all(&target).unwrap();

        let err = atomic_write(&target, b"payload").unwrap_err();
        let crate::errors::Error::Io(source) = err else {
            panic!("atomic_write failure must wrap an I/O error");
        };
        assert_ne!(source.kind(), std::io::ErrorKind::NotFound);

        assert!(target.is_dir());
        let leftovers = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(std::result::Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
            .count();
        assert_eq!(leftovers, 0, "rename 失败后不能遗留临时文件");
    }

    #[test]
    fn atomic_write_fails_when_parent_is_not_a_directory() {
        let dir = crate::test_support::Fixture::new("parent-file");
        let parent_file = dir.join("parent");
        std::fs::write(&parent_file, b"occupied").unwrap();
        let target = parent_file.join("config.toml");

        let err = atomic_write(&target, b"payload").unwrap_err();
        let crate::errors::Error::Io(source) = err else {
            panic!("atomic_write failure must wrap an I/O error");
        };
        assert_ne!(source.kind(), std::io::ErrorKind::NotFound);

        assert_eq!(std::fs::read(&parent_file).unwrap(), b"occupied");
    }

    #[cfg(unix)]
    #[test]
    fn stale_temp_cleanup_preserves_current_process_writes() {
        let dir = crate::test_support::Fixture::new("temp-active");
        let active = dir.join(format!(".state.json.{}.0.tmp", std::process::id()));
        fs::write(&active, b"in progress").unwrap();

        cleanup_stale_atomic_temp_files(&dir).unwrap();

        assert!(active.is_file(), "cleanup must not remove an active write");
    }

    #[cfg(unix)]
    #[test]
    fn stale_temp_cleanup_removes_only_old_atomic_regular_files() {
        use std::fs::FileTimes;
        use std::os::unix::fs::symlink;
        use std::time::{Duration, SystemTime};

        let dir = crate::test_support::Fixture::new("temp-stale");
        let old = SystemTime::now() - Duration::from_secs(2 * 86400);
        let stale = ".state.json.4294967295.1.tmp";
        let unrelated = [
            ".notes.backup.tmp",
            ".state.json.invalid.1.tmp",
            ".state.json.4294967295.invalid.tmp",
            ".4294967295.1.tmp",
        ];
        for name in std::iter::once(stale).chain(unrelated) {
            let file = fs::File::create(dir.join(name)).unwrap();
            file.set_times(FileTimes::new().set_modified(old)).unwrap();
        }
        let recent = dir.join(".state.json.4294967295.2.tmp");
        fs::write(&recent, b"recent").unwrap();
        let link = dir.join(".state.json.4294967295.3.tmp");
        symlink(dir.join(unrelated[0]), &link).unwrap();
        let directory = dir.join(".state.json.4294967295.4.tmp");
        fs::create_dir(&directory).unwrap();

        cleanup_stale_atomic_temp_files(&dir).unwrap();

        assert!(!dir.join(stale).exists());
        for name in unrelated {
            assert!(dir.join(name).is_file(), "unrelated file removed: {name}");
        }
        assert!(recent.is_file());
        assert!(link.is_symlink());
        assert!(directory.is_dir());
    }

    #[cfg(unix)]
    #[test]
    fn directory_metadata_follows_symlink_to_stock_directory() {
        use std::os::unix::fs::symlink;

        let fixture = crate::test_support::Fixture::new("directory-metadata-symlink");
        let stock = fixture.join("stock/media");
        let target = fixture.join("target/media");
        std::fs::create_dir_all(&stock).unwrap();
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        symlink("../stock/media", &target).unwrap();

        assert!(directory_metadata(&target).unwrap().is_dir());
    }
}
