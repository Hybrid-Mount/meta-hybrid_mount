// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    collections::HashSet,
    ffi::CString,
    fs::{self, File},
    io::Write,
    os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt, symlink},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use rustix::fs::ioctl_ficlone;
use walkdir::WalkDir;

use super::xattr::internal_copy_extended_attributes;
use crate::defs;

#[derive(Debug, Default)]
pub struct SyncDirStats {
    pub has_mount_content: bool,
    pub opaque_dirs: Vec<PathBuf>,
}

fn is_builtin_partition_path(relative: &Path) -> bool {
    relative
        .components()
        .next()
        .and_then(|component| component.as_os_str().to_str())
        .is_some_and(|name| defs::BUILTIN_PARTITIONS.contains(&name))
}

pub fn atomic_write<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, content: C) -> Result<()> {
    let path = path.as_ref();

    let mut tempfile = tempfile::Builder::new().tempfile()?;

    tempfile.write_all(content.as_ref())?;

    fs::rename(tempfile.path(), path)?;

    Ok(())
}

pub fn ensure_dir_exists<T: AsRef<Path>>(dir: T) -> Result<()> {
    if !dir.as_ref().exists() {
        fs::create_dir_all(&dir)?;
    }
    Ok(())
}

pub fn reflink_or_copy(src: &Path, dest: &Path) -> Result<u64> {
    let src_file = File::open(src)?;
    let dest_file = File::create(dest)?;

    if ioctl_ficlone(&dest_file, &src_file).is_ok() {
        let metadata = src_file.metadata()?;
        let len = metadata.len();
        dest_file.set_permissions(metadata.permissions())?;
        return Ok(len);
    }
    drop(dest_file);
    drop(src_file);
    fs::copy(src, dest).map_err(|e| e.into())
}

fn make_device_node(path: &Path, mode: u32, rdev: u64) -> Result<()> {
    let c_path = CString::new(path.as_os_str().as_encoded_bytes())?;
    let dev = rdev as libc::dev_t;
    unsafe {
        if libc::mknod(c_path.as_ptr(), mode as libc::mode_t, dev) != 0 {
            let err = std::io::Error::last_os_error();
            bail!("mknod failed for {}: {}", path.display(), err);
        }
    }
    Ok(())
}

fn native_cp_r(
    src: &Path,
    dst: &Path,
    relative: &Path,
    _repair: bool,
    visited: &mut HashSet<(u64, u64)>,
    stats: &mut SyncDirStats,
) -> Result<()> {
    if !dst.exists() {
        if src.is_dir() {
            fs::create_dir_all(dst)?;
        }
        if let Ok(src_meta) = src.metadata() {
            let _ = fs::set_permissions(dst, src_meta.permissions());
        }
        let _ = internal_copy_extended_attributes(src, dst);
    }

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let file_name = entry.file_name();
        let dst_path = dst.join(&file_name);
        let next_relative = relative.join(&file_name);

        let metadata = entry.metadata()?;
        let ft = metadata.file_type();
        let dev = metadata.dev();
        let ino = metadata.ino();

        if !ft.is_dir() && is_builtin_partition_path(&next_relative) {
            stats.has_mount_content = true;
        }

        if ft.is_dir() {
            if !visited.insert((dev, ino)) {
                continue;
            }
            native_cp_r(
                &src_path,
                &dst_path,
                &next_relative,
                _repair,
                visited,
                stats,
            )?;
        } else if ft.is_symlink() {
            if dst_path.exists() {
                fs::remove_file(&dst_path)?;
            }
            let link_target = fs::read_link(&src_path)?;
            symlink(&link_target, &dst_path)?;
        } else if ft.is_char_device() || ft.is_block_device() || ft.is_fifo() {
            if dst_path.exists() {
                fs::remove_file(&dst_path)?;
            }
            let mode = metadata.permissions().mode();
            let rdev = metadata.rdev();
            make_device_node(&dst_path, mode, rdev)?;
        } else {
            reflink_or_copy(&src_path, &dst_path)?;
        }

        if ft.is_file()
            && file_name.as_os_str() == defs::REPLACE_DIR_FILE_NAME
            && let Some(parent) = dst_path.parent()
        {
            stats.opaque_dirs.push(parent.to_path_buf());
        }

        let _ = internal_copy_extended_attributes(&src_path, &dst_path);
    }
    Ok(())
}

pub fn sync_dir(src: &Path, dst: &Path, repair_context: bool) -> Result<SyncDirStats> {
    if !src.exists() {
        return Ok(SyncDirStats::default());
    }
    ensure_dir_exists(dst)?;
    let mut visited = HashSet::new();
    let mut stats = SyncDirStats::default();
    native_cp_r(
        src,
        dst,
        Path::new(""),
        repair_context,
        &mut visited,
        &mut stats,
    )
    .with_context(|| {
        format!(
            "Failed to natively sync {} to {}",
            src.display(),
            dst.display()
        )
    })?;
    Ok(stats)
}

pub fn prune_empty_dirs<P: AsRef<Path>>(root: P) -> Result<()> {
    let root = root.as_ref();
    if !root.exists() {
        return Ok(());
    }

    for entry in WalkDir::new(root)
        .min_depth(1)
        .contents_first(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_dir() {
            let path = entry.path();
            if fs::remove_dir(path).is_ok() {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::sync_dir;

    #[test]
    fn sync_dir_reports_mount_content_and_replace_directories() {
        let temp = tempdir().expect("failed to create tempdir");
        let src = temp.path().join("src");
        let dst = temp.path().join("dst");

        fs::create_dir_all(src.join("system/bin")).expect("failed to create system/bin");
        fs::write(src.join("system/bin/app_process"), b"bin").expect("failed to write payload");
        fs::write(src.join("system/bin/.replace"), b"1").expect("failed to write .replace");
        fs::write(src.join("module.prop"), b"name=demo").expect("failed to write module.prop");

        let stats = sync_dir(&src, &dst, true).expect("sync_dir should succeed");

        assert!(stats.has_mount_content);
        assert_eq!(stats.opaque_dirs, vec![dst.join("system/bin")]);
    }

    #[test]
    fn sync_dir_ignores_non_partition_root_files_for_mount_content() {
        let temp = tempdir().expect("failed to create tempdir");
        let src = temp.path().join("src");
        let dst = temp.path().join("dst");

        fs::create_dir_all(&src).expect("failed to create src");
        fs::write(src.join("module.prop"), b"name=demo").expect("failed to write module.prop");

        let stats = sync_dir(&src, &dst, true).expect("sync_dir should succeed");

        assert!(!stats.has_mount_content);
        assert!(stats.opaque_dirs.is_empty());
    }
}
