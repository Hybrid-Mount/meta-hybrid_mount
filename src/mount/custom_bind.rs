// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
#[cfg(any(target_os = "linux", target_os = "android"))]
use rustix::mount::{MountFlags, mount_bind, mount_remount};

use crate::conf::schema::CustomBindMount;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomBindKind {
    File,
    Directory,
}

#[derive(Debug, Clone)]
pub struct AppliedCustomBind {
    pub source: PathBuf,
    pub target: PathBuf,
    pub kind: CustomBindKind,
}

pub fn apply_custom_bind_mounts(
    mounts: &[CustomBindMount],
    disable_umount: bool,
) -> Result<Vec<AppliedCustomBind>> {
    let mut applied_mounts = Vec::with_capacity(mounts.len());

    for mount in mounts {
        let applied = apply_one(mount, disable_umount).with_context(|| {
            format!(
                "failed to apply custom bind {} -> {}",
                mount.source.display(),
                mount.target.display()
            )
        })?;
        crate::scoped_log!(
            info,
            "custom_bind",
            "mounted: source={}, target={}",
            applied.source.display(),
            applied.target.display()
        );
        applied_mounts.push(applied);
    }

    Ok(applied_mounts)
}

fn apply_one(mount: &CustomBindMount, disable_umount: bool) -> Result<AppliedCustomBind> {
    let kind = validate_mount_paths(&mount.source, &mount.target)?;
    bind_mount_checked(&mount.source, &mount.target, disable_umount)?;

    Ok(AppliedCustomBind {
        source: mount.source.clone(),
        target: mount.target.clone(),
        kind,
    })
}

fn validate_mount_paths(source: &Path, target: &Path) -> Result<CustomBindKind> {
    if !source.is_absolute() {
        bail!("custom bind source must be an absolute path");
    }
    if !target.is_absolute() {
        bail!("custom bind target must be an absolute path");
    }
    if source == target {
        bail!("custom bind source and target must differ");
    }

    let source_meta = fs::metadata(source)
        .with_context(|| format!("failed to inspect source {}", source.display()))?;
    let target_meta = fs::metadata(target)
        .with_context(|| format!("failed to inspect target {}", target.display()))?;

    match (source_meta.is_dir(), target_meta.is_dir()) {
        (true, true) => Ok(CustomBindKind::Directory),
        (false, false) => Ok(CustomBindKind::File),
        (true, false) => bail!("custom bind source is a directory but target is not"),
        (false, true) => bail!("custom bind source is not a directory but target is"),
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn bind_mount_checked(source: &Path, target: &Path, disable_umount: bool) -> Result<()> {
    mount_bind(source, target).with_context(|| {
        format!(
            "failed to bind mount {} to {}",
            source.display(),
            target.display()
        )
    })?;

    mount_remount(target, MountFlags::RDONLY | MountFlags::BIND, "").with_context(|| {
        format!(
            "failed to remount custom bind readonly: {}",
            target.display()
        )
    })?;

    if !disable_umount {
        crate::mount::umount_mgr::send_umountable(target).with_context(|| {
            format!(
                "failed to register custom bind target as umountable: {}",
                target.display()
            )
        })?;
    }

    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn bind_mount_checked(_source: &Path, _target: &Path, _disable_umount: bool) -> Result<()> {
    bail!("custom bind mounts are only supported on linux/android")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_rejects_relative_paths() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("target");
        fs::write(&target, b"").unwrap();

        assert!(validate_mount_paths(Path::new("relative"), &target).is_err());
        assert!(validate_mount_paths(&target, Path::new("relative")).is_err());
    }

    #[test]
    fn validate_rejects_type_mismatch() {
        let temp = tempfile::tempdir().unwrap();
        let source_dir = temp.path().join("source");
        let target_file = temp.path().join("target");
        fs::create_dir(&source_dir).unwrap();
        fs::write(&target_file, b"").unwrap();

        assert!(validate_mount_paths(&source_dir, &target_file).is_err());
    }

    #[test]
    fn validate_accepts_file_to_file() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        fs::write(&source, b"").unwrap();
        fs::write(&target, b"").unwrap();

        assert_eq!(
            validate_mount_paths(&source, &target).unwrap(),
            CustomBindKind::File
        );
    }

    #[test]
    fn validate_accepts_dir_to_dir() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&target).unwrap();

        assert_eq!(
            validate_mount_paths(&source, &target).unwrap(),
            CustomBindKind::Directory
        );
    }
}
