// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::{
    collections::HashSet,
    ffi::CString,
    fs::{self, File},
    io::{ErrorKind, Write},
    os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt, symlink},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
#[cfg(any(target_os = "linux", target_os = "android"))]
use rustix::fs::{CWD, FileType, Gid, Mode, Uid, chown, mknodat};
use walkdir::WalkDir;

#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::sys::fs::{lgetfilecon, lsetfilecon};

pub fn atomic_write<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, content: C) -> Result<()> {
    let path = path.as_ref();
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    ensure_dir_exists(parent)?;

    let mut tempfile = tempfile::Builder::new()
        .tempfile_in(parent)
        .with_context(|| {
            format!(
                "failed to create temp file for atomic write in {}",
                parent.display()
            )
        })?;

    tempfile
        .write_all(content.as_ref())
        .with_context(|| format!("failed to write temp file for {}", path.display()))?;
    tempfile
        .flush()
        .with_context(|| format!("failed to flush temp file for {}", path.display()))?;
    tempfile
        .as_file()
        .sync_all()
        .with_context(|| format!("failed to sync temp file for {}", path.display()))?;

    tempfile
        .persist(path)
        .map(|_| ())
        .with_context(|| format!("failed to atomically replace {}", path.display()))?;

    sync_parent_dir(parent)?;

    Ok(())
}

fn sync_parent_dir(parent: &Path) -> Result<()> {
    let dir = File::open(parent)
        .with_context(|| format!("failed to open parent directory {}", parent.display()))?;
    dir.sync_all()
        .with_context(|| format!("failed to sync parent directory {}", parent.display()))?;
    Ok(())
}

pub fn ensure_dir_exists<T: AsRef<Path>>(dir: T) -> Result<()> {
    let dir = dir.as_ref();
    if let Err(err) = fs::create_dir_all(dir) {
        if let Ok(metadata) = fs::metadata(dir)
            && !metadata.is_dir()
        {
            bail!("path exists but is not a directory: {}", dir.display());
        }
        return Err(err).with_context(|| format!("failed to create directory {}", dir.display()));
    }
    let metadata = fs::metadata(dir)
        .with_context(|| format!("failed to inspect directory {}", dir.display()))?;
    if !metadata.is_dir() {
        bail!("path exists but is not a directory: {}", dir.display());
    }
    Ok(())
}

pub fn copy_file(src: &Path, dest: &Path) -> Result<u64> {
    Ok(fs::copy(src, dest)?)
}

pub fn remove_path(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_dir() => Ok(fs::remove_dir_all(path)?),
        Ok(_) => Ok(fs::remove_file(path)?),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.into()),
    }
}

pub struct PreparedDir {
    id: String,
    tmp_dst: PathBuf,
    final_dst: PathBuf,
    cleanup_tmp: bool,
}

impl PreparedDir {
    pub fn new(target_base: &Path, id: &str) -> Result<Self> {
        let tmp_dst = target_base.join(format!(".tmp_{id}"));
        remove_path(&tmp_dst)?;
        Ok(Self {
            id: id.to_string(),
            tmp_dst,
            final_dst: target_base.join(id),
            cleanup_tmp: true,
        })
    }

    pub fn tmp_path(&self) -> &Path {
        &self.tmp_dst
    }

    pub fn final_path(&self) -> &Path {
        &self.final_dst
    }

    pub fn commit(mut self) -> Result<()> {
        let backup_dst = self
            .final_dst
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(format!(".backup_{}", self.id));
        remove_path(&backup_dst)?;

        let mut backup_created = false;
        if self.final_dst.exists() {
            fs::rename(&self.final_dst, &backup_dst).with_context(|| {
                format!(
                    "failed to back up prepared dir {} from {} to {}",
                    self.id,
                    self.final_dst.display(),
                    backup_dst.display()
                )
            })?;
            backup_created = true;
        }

        if let Err(err) = fs::rename(&self.tmp_dst, &self.final_dst).with_context(|| {
            format!(
                "failed to commit prepared dir {} from {} to {}",
                self.id,
                self.tmp_dst.display(),
                self.final_dst.display()
            )
        }) {
            if backup_created {
                fs::rename(&backup_dst, &self.final_dst).with_context(|| {
                    format!(
                        "failed to restore prepared dir {} after commit error: {err:#}",
                        self.id
                    )
                })?;
            }
            return Err(err);
        }

        self.cleanup_tmp = false;
        if backup_created {
            remove_path(&backup_dst)
                .with_context(|| format!("failed to remove prepared backup for {}", self.id))?;
        }

        Ok(())
    }
}

impl Drop for PreparedDir {
    fn drop(&mut self) {
        if self.cleanup_tmp {
            let _ = remove_path(&self.tmp_dst);
        }
    }
}

pub fn prune_orphaned_children<'a, I>(
    target_base: &Path,
    active_names: I,
    preserved_names: &[&str],
    log_scope: &str,
) -> Result<()>
where
    I: IntoIterator<Item = &'a str>,
{
    if !target_base.exists() {
        return Ok(());
    }

    let active_names: HashSet<&str> = active_names.into_iter().collect();

    for entry in target_base.read_dir()? {
        let entry = entry.with_context(|| {
            format!(
                "[{log_scope}] failed to enumerate {}",
                target_base.display()
            )
        })?;
        let path = entry.path();
        let name_os = entry.file_name();
        let name = name_os
            .to_str()
            .with_context(|| format!("[{log_scope}] entry name is not valid UTF-8"))?;

        if name.starts_with('.') || active_names.contains(name) || preserved_names.contains(&name) {
            continue;
        }

        log::info!("[{log_scope}] prune orphan: name={name}");
        remove_path(&path)
            .with_context(|| format!("[{log_scope}] failed to remove orphan {name}"))?;
    }

    Ok(())
}

pub fn ensure_dir_like(src: &Path, dst: &Path) -> Result<()> {
    match fs::symlink_metadata(dst) {
        Ok(metadata) => {
            if metadata.file_type().is_dir() {
                return Ok(());
            }
            bail!("path exists but is not a directory: {}", dst.display());
        }
        Err(err) if err.kind() == ErrorKind::NotFound => {}
        Err(err) => {
            return Err(err).with_context(|| format!("failed to inspect {}", dst.display()));
        }
    }

    fs::create_dir_all(dst)
        .with_context(|| format!("failed to create directory {}", dst.display()))?;
    let src_meta = fs::symlink_metadata(src)
        .with_context(|| format!("failed to inspect source directory {}", src.display()))?;
    fs::set_permissions(dst, src_meta.permissions())
        .with_context(|| format!("failed to copy permissions to {}", dst.display()))?;
    clone_ownership_from_metadata(src, dst, &src_meta)?;
    clone_selinux_context(src, dst)?;
    Ok(())
}

pub fn copy_non_dir_entry(
    src: &Path,
    dst: &Path,
    metadata: &fs::Metadata,
    file_type: &fs::FileType,
) -> Result<u64> {
    remove_path(dst)?;
    if file_type.is_symlink() {
        let link_target = fs::read_link(src)?;
        symlink(&link_target, dst)?;
        clone_ownership_from_metadata(src, dst, metadata)?;
        clone_selinux_context(src, dst)?;
        Ok(0)
    } else if file_type.is_char_device() || file_type.is_block_device() || file_type.is_fifo() {
        let mode = metadata.permissions().mode();
        let rdev = metadata.rdev();
        make_device_node(dst, mode, rdev)?;
        clone_ownership_from_metadata(src, dst, metadata)?;
        clone_selinux_context(src, dst)?;
        Ok(0)
    } else {
        let copied_bytes = copy_file(src, dst)?;
        clone_ownership_from_metadata(src, dst, metadata)?;
        clone_selinux_context(src, dst)?;
        Ok(copied_bytes)
    }
}

pub fn finalize_copied_tree(id: &str, root: &Path, opaque_dirs: &[PathBuf]) -> Result<()> {
    prune_empty_dirs_preserving(root, opaque_dirs)
        .with_context(|| format!("failed to prune copied tree for {id}"))?;

    for opaque_dir in opaque_dirs {
        super::xattr::set_overlay_opaque(opaque_dir).with_context(|| {
            format!(
                "failed to apply overlay opaque metadata for {id} at {}",
                opaque_dir.display()
            )
        })?;
    }
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn clone_selinux_context(src: &Path, dst: &Path) -> Result<()> {
    let context = lgetfilecon(src)
        .with_context(|| format!("failed to read SELinux context from {}", src.display()))?;
    lsetfilecon(dst, &context)
        .with_context(|| format!("failed to set SELinux context on {}", dst.display()))
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn clone_selinux_context(_src: &Path, _dst: &Path) -> Result<()> {
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn clone_ownership_from_metadata(src: &Path, dst: &Path, metadata: &fs::Metadata) -> Result<()> {
    let result = if metadata.file_type().is_symlink() {
        let c_path = CString::new(dst.as_os_str().as_encoded_bytes())
            .with_context(|| format!("destination path contains NUL: {}", dst.display()))?;

        let rc = unsafe {
            libc::lchown(
                c_path.as_ptr(),
                metadata.uid() as libc::uid_t,
                metadata.gid() as libc::gid_t,
            )
        };

        if rc == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    } else {
        chown(
            dst,
            Some(Uid::from_raw(metadata.uid())),
            Some(Gid::from_raw(metadata.gid())),
        )
        .map_err(std::io::Error::from)
    };

    result.with_context(|| {
        format!(
            "failed to clone ownership from {} to {} (uid={}, gid={})",
            src.display(),
            dst.display(),
            metadata.uid(),
            metadata.gid()
        )
    })?;
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn clone_ownership_from_metadata(_src: &Path, _dst: &Path, _metadata: &fs::Metadata) -> Result<()> {
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn make_device_node(path: &Path, mode: u32, rdev: u64) -> Result<()> {
    let file_type = FileType::from_raw_mode(mode);
    if matches!(file_type, FileType::Unknown) {
        bail!("mknod failed for {}: unknown file type", path.display());
    }

    mknodat(
        CWD,
        path,
        file_type,
        Mode::from_raw_mode(mode & 0o7777),
        rdev as _,
    )
    .with_context(|| format!("mknod failed for {}", path.display()))?;
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
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

pub fn prune_empty_dirs<P: AsRef<Path>>(root: P) -> Result<()> {
    prune_empty_dirs_preserving(root.as_ref(), &[])
}

fn prune_empty_dirs_preserving(root: &Path, preserved_dirs: &[PathBuf]) -> Result<()> {
    if !root.exists() {
        return Ok(());
    }

    let preserved_dirs: HashSet<PathBuf> = preserved_dirs.iter().cloned().collect();

    for entry in WalkDir::new(root)
        .min_depth(1)
        .contents_first(true)
        .into_iter()
    {
        let entry = entry.context("failed to enumerate copied tree")?;
        if entry.file_type().is_dir() {
            let path = entry.path();
            if preserved_dirs.contains(path) {
                continue;
            }
            if let Err(err) = fs::remove_dir(path)
                && err.raw_os_error() != Some(libc::ENOTEMPTY)
                && err.raw_os_error() != Some(libc::EEXIST)
            {
                return Err(err)
                    .with_context(|| format!("failed to prune directory {}", path.display()));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_dir_exists_creates_nested_directory() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("a").join("b");

        ensure_dir_exists(&path).unwrap();

        assert!(path.is_dir());
    }

    #[test]
    fn ensure_dir_exists_rejects_existing_file() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("not-a-dir");
        fs::write(&path, b"file").unwrap();

        let err = ensure_dir_exists(&path).unwrap_err();

        assert!(
            format!("{err:#}").contains("not a directory"),
            "unexpected error: {err:#}"
        );
    }

    #[test]
    fn ensure_dir_like_rejects_existing_file() {
        let temp = tempfile::tempdir().unwrap();
        let src = temp.path().join("src");
        let dst = temp.path().join("dst");
        fs::create_dir(&src).unwrap();
        fs::write(&dst, b"file").unwrap();

        let err = ensure_dir_like(&src, &dst).unwrap_err();

        assert!(
            format!("{err:#}").contains("not a directory"),
            "unexpected error: {err:#}"
        );
    }
}
