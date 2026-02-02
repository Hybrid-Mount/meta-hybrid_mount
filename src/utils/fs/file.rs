// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    collections::HashSet,
    ffi::CString,
    fs::{self, File, OpenOptions},
    io::Write,
    os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt, symlink},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};
use rustix::fs::ioctl_ficlone;
use walkdir::WalkDir;

use super::xattr::{internal_apply_system_context, internal_copy_extended_attributes};

pub fn atomic_write<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, content: C) -> Result<()> {
    let path = path.as_ref();
    let dir = path.parent().unwrap_or_else(|| Path::new("."));

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let pid = std::process::id();
    let temp_name = format!(".{}_{}.tmp", pid, now);
    let temp_file = dir.join(temp_name);

    {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_file)?;
        file.write_all(content.as_ref())?;
    }

    if let Err(_e) = fs::rename(&temp_file, path) {
        if let Err(copy_err) = fs::copy(&temp_file, path) {
            let _ = fs::remove_file(&temp_file);
            return Err(copy_err).context("atomic_write copy fallback failed");
        }
        let _ = fs::remove_file(&temp_file);
    }
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
    repair: bool,
    visited: &mut HashSet<(u64, u64)>,
) -> Result<()> {
    if !dst.exists() {
        if src.is_dir() {
            fs::create_dir_all(dst)?;
        }
        if let Ok(src_meta) = src.metadata() {
            let _ = fs::set_permissions(dst, src_meta.permissions());
        }

        if repair && relative.as_os_str().is_empty() {
            let _ = internal_apply_system_context(dst, relative);
        } else if !repair {
            let _ = internal_copy_extended_attributes(src, dst);
        }
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

        if ft.is_dir() {
            if !visited.insert((dev, ino)) {
                continue;
            }
            native_cp_r(&src_path, &dst_path, &next_relative, repair, visited)?;
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

        let _ = internal_copy_extended_attributes(&src_path, &dst_path);

        if repair {
            let _ = internal_apply_system_context(&dst_path, &next_relative);
        }
    }
    Ok(())
}

pub fn sync_dir(src: &Path, dst: &Path, repair_context: bool) -> Result<()> {
    if !src.exists() {
        return Ok(());
    }
    ensure_dir_exists(dst)?;
    let mut visited = HashSet::new();
    native_cp_r(src, dst, Path::new(""), repair_context, &mut visited).with_context(|| {
        format!(
            "Failed to natively sync {} to {}",
            src.display(),
            dst.display()
        )
    })
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
