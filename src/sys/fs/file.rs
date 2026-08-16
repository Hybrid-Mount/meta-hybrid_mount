// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::{
    collections::HashSet,
    ffi::CString,
    fs::{self, File},
    io::Write,
    os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt, symlink},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
#[cfg(any(target_os = "linux", target_os = "android"))]
use rustix::fs::ioctl_ficlone;
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

    sync_parent_dir(parent);

    Ok(())
}

fn sync_parent_dir(parent: &Path) {
    if let Ok(dir) = File::open(parent) {
        let _ = dir.sync_all();
    }
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

    #[cfg(any(target_os = "linux", target_os = "android"))]
    if ioctl_ficlone(&dest_file, &src_file).is_ok() {
        let metadata = src_file.metadata()?;
        let len = metadata.len();
        dest_file.set_permissions(metadata.permissions())?;
        return Ok(len);
    }
    drop(dest_file);
    drop(src_file);
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
                let _ = fs::rename(&backup_dst, &self.final_dst);
            }
            return Err(err);
        }

        self.cleanup_tmp = false;
        if backup_created && let Err(err) = remove_path(&backup_dst) {
            crate::scoped_log!(
                warn,
                "fs:copy",
                "cleanup backup failed: id={}, path={}, error={:#}",
                self.id,
                backup_dst.display(),
                err
            );
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

    for entry in target_base.read_dir()?.flatten() {
        let path = entry.path();
        let name_os = entry.file_name();
        let name = name_os.to_string_lossy();

        if name.starts_with('.')
            || active_names.contains(name.as_ref())
            || preserved_names.iter().any(|preserved| *preserved == name)
        {
            continue;
        }

        log::info!("[{log_scope}] prune orphan: name={name}");
        if let Err(err) = remove_path(&path) {
            log::warn!("[{log_scope}] remove orphan failed: name={name}, error={err}");
        }
    }

    Ok(())
}

pub fn ensure_dir_like(src: &Path, dst: &Path) -> Result<()> {
    if !dst.exists() {
        fs::create_dir_all(dst)?;
        if let Ok(src_meta) = src.metadata() {
            let _ = fs::set_permissions(dst, src_meta.permissions());
        }
        clone_ownership(src, dst);
        clone_selinux_context(src, dst);
    }
    Ok(())
}

pub fn copy_non_dir_entry(
    src: &Path,
    dst: &Path,
    metadata: &fs::Metadata,
    file_type: &fs::FileType,
) -> Result<()> {
    remove_path(dst)?;
    if file_type.is_symlink() {
        let link_target = fs::read_link(src)?;
        symlink(&link_target, dst)?;
        clone_ownership(src, dst);
        clone_selinux_context(src, dst);
    } else if file_type.is_char_device() || file_type.is_block_device() || file_type.is_fifo() {
        let mode = metadata.permissions().mode();
        let rdev = metadata.rdev();
        make_device_node(dst, mode, rdev)?;
        clone_ownership(src, dst);
        clone_selinux_context(src, dst);
    } else {
        reflink_or_copy(src, dst)?;
        clone_ownership(src, dst);
        clone_selinux_context(src, dst);
    }
    Ok(())
}

pub fn finalize_copied_tree(id: &str, root: &Path, opaque_dirs: &[PathBuf]) {
    if let Err(err) = prune_empty_dirs_preserving(root, opaque_dirs) {
        crate::scoped_log!(
            warn,
            "fs:copy",
            "prune empty dirs failed: id={}, error={}",
            id,
            err
        );
    }

    for opaque_dir in opaque_dirs {
        if let Err(err) = super::xattr::set_overlay_opaque(opaque_dir) {
            crate::scoped_log!(
                warn,
                "fs:copy",
                "apply overlay opaque failed: id={}, path={}, error={}",
                id,
                opaque_dir.display(),
                err
            );
        } else {
            crate::scoped_log!(
                debug,
                "fs:copy",
                "set overlay opaque: id={}, path={}",
                id,
                opaque_dir.display()
            );
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn clone_selinux_context(src: &Path, dst: &Path) {
    match lgetfilecon(src).and_then(|con| lsetfilecon(dst, &con)) {
        Ok(()) => {}
        Err(err) => {
            crate::scoped_log!(
                warn,
                "fs:copy",
                "clone selinux context skipped: src={}, dst={}, error={:#}",
                src.display(),
                dst.display(),
                err
            );
        }
    }
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn clone_selinux_context(_src: &Path, _dst: &Path) {}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn clone_ownership(src: &Path, dst: &Path) {
    let metadata = match fs::symlink_metadata(src) {
        Ok(metadata) => metadata,
        Err(err) => {
            crate::scoped_log!(
                warn,
                "fs:copy",
                "clone ownership skipped: src={}, dst={}, error={}",
                src.display(),
                dst.display(),
                err
            );
            return;
        }
    };

    let result = if metadata.file_type().is_symlink() {
        let c_path = match CString::new(dst.as_os_str().as_encoded_bytes()) {
            Ok(path) => path,
            Err(err) => {
                crate::scoped_log!(
                    warn,
                    "fs:copy",
                    "clone ownership skipped: src={}, dst={}, error={}",
                    src.display(),
                    dst.display(),
                    err
                );
                return;
            }
        };

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

    if let Err(err) = result {
        crate::scoped_log!(
            warn,
            "fs:copy",
            "clone ownership skipped: src={}, dst={}, uid={}, gid={}, error={}",
            src.display(),
            dst.display(),
            metadata.uid(),
            metadata.gid(),
            err
        );
    }
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn clone_ownership(_src: &Path, _dst: &Path) {}

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
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_dir() {
            let path = entry.path();
            if preserved_dirs.contains(path) {
                continue;
            }
            if fs::remove_dir(path).is_ok() {}
        }
    }
    Ok(())
}
