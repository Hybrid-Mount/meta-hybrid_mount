// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::path::{Path, PathBuf};

#[cfg(any(target_os = "linux", target_os = "android"))]
use anyhow::Context;
use anyhow::Result;
#[cfg(not(any(target_os = "linux", target_os = "android")))]
use anyhow::bail;
#[cfg(any(target_os = "linux", target_os = "android"))]
use procfs::process::Process;
#[cfg(any(target_os = "linux", target_os = "android"))]
use rustix::mount::{MountFlags, mount};

#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::sys::fs::ensure_dir_exists;

pub fn detect_mount_source() -> String {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        if ksu::version().is_some() {
            crate::scoped_log!(debug, "sys:mount_source", "complete: source=KSU");
            return "KSU".to_string();
        }
    }
    crate::scoped_log!(debug, "sys:mount_source", "complete: source=APatch");
    "APatch".to_string()
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn is_mounted<P: AsRef<Path>>(path: P) -> Result<bool> {
    let path_str = path
        .as_ref()
        .to_str()
        .context("mount path is not valid UTF-8")?;
    let search = if path_str == "/" {
        "/"
    } else {
        path_str.trim_end_matches('/')
    };
    let mountinfo = Process::myself()?.mountinfo()?;
    Ok(mountinfo
        .into_iter()
        .any(|m| m.mount_point.to_string_lossy() == search))
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[allow(dead_code)]
pub fn mount_tmpfs(target: &Path, source: &str) -> Result<()> {
    crate::scoped_log!(
        info,
        "sys:mount_tmpfs",
        "start: source={}, target={}",
        source,
        target.display()
    );
    ensure_dir_exists(target)?;
    mount(
        source,
        target,
        c"tmpfs",
        MountFlags::empty(),
        Some(c"mode=0755"),
    )
    .context("Failed to mount tmpfs")?;
    crate::scoped_log!(
        info,
        "sys:mount_tmpfs",
        "complete: source={}, target={}",
        source,
        target.display()
    );
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
#[allow(dead_code)]
pub fn mount_tmpfs(_target: &Path, _source: &str) -> Result<()> {
    bail!("tmpfs mounting is only supported on linux/android")
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn is_stale_mount(
    mount_source: Option<&str>,
    mount_point: &Path,
    source: &str,
    moduledir: &Path,
    custom_targets: &[PathBuf],
    managed_partitions: &[String],
) -> bool {
    // Storage tmpfs, overlay mounts and magic-mount tmpfs trees all use the
    // configured mount source namespace (KSU/APatch) as their mount source.
    if mount_source == Some(source) {
        return true;
    }

    // Our backing storage and staging trees always live under /mnt/hm_*.
    if mount_point.starts_with("/mnt/hm_") {
        return true;
    }

    // Custom bind mounts are registered with their exact target path.
    if custom_targets.iter().any(|target| target == mount_point) {
        return true;
    }

    // Magic Mount binds files directly from the module directory onto managed
    // partitions. Restrict to managed partition roots so we never touch
    // unrelated modules' runtime mounts.
    let sourced_from_module_dir = mount_source.is_some_and(|s| Path::new(s).starts_with(moduledir));
    sourced_from_module_dir
        && managed_partitions
            .iter()
            .any(|partition| mount_point.starts_with(Path::new("/").join(partition)))
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn stale_mount_points<'a>(
    mounts: impl IntoIterator<Item = (Option<&'a str>, &'a Path)>,
    source: &str,
    moduledir: &Path,
    custom_targets: &[PathBuf],
    managed_partitions: &[String],
) -> Vec<PathBuf> {
    let mut points: Vec<PathBuf> = mounts
        .into_iter()
        .filter(|(mount_source, mount_point)| {
            is_stale_mount(
                *mount_source,
                mount_point,
                source,
                moduledir,
                custom_targets,
                managed_partitions,
            )
        })
        .map(|(_, mount_point)| mount_point.to_path_buf())
        .collect();

    // Unmount deeper children before their parents (storage tree roots,
    // overlay roots) so a DETACH never leaves children behind.
    points.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    points.dedup();
    points
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn unmount_stale_mounts(
    source: &str,
    moduledir: &Path,
    custom_targets: &[PathBuf],
    managed_partitions: &[String],
) -> Result<usize> {
    use rustix::mount::{UnmountFlags, unmount};

    let mounts = match Process::myself().and_then(|process| process.mountinfo()) {
        Ok(mounts) => mounts,
        Err(err) => {
            crate::scoped_log!(
                warn,
                "sys:mount_source",
                "late_load cleanup skipped: reason=mountinfo_unavailable, error={:#}",
                err
            );
            return Ok(0);
        }
    };
    let points = stale_mount_points(
        mounts
            .iter()
            .map(|mount| (mount.mount_source.as_deref(), mount.mount_point.as_path())),
        source,
        moduledir,
        custom_targets,
        managed_partitions,
    );

    let mut unmounted = 0usize;
    for mount_point in &points {
        crate::scoped_log!(
            debug,
            "sys:mount_source",
            "late_load cleanup: unmount={}, source={}",
            mount_point.display(),
            source
        );
        match unmount(mount_point, UnmountFlags::DETACH) {
            Ok(()) => unmounted += 1,
            Err(err) => {
                crate::scoped_log!(
                    warn,
                    "sys:mount_source",
                    "late_load cleanup unmount failed: path={}, error={:#}",
                    mount_point.display(),
                    err
                );
            }
        }
    }

    crate::scoped_log!(
        info,
        "sys:mount_source",
        "late_load cleanup complete: attempted={}, unmounted={}",
        points.len(),
        unmounted
    );
    Ok(unmounted)
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn unmount_stale_mounts(
    _source: &str,
    _moduledir: &Path,
    _custom_targets: &[PathBuf],
    _managed_partitions: &[String],
) -> Result<usize> {
    bail!("stale mount cleanup is only supported on linux/android")
}

#[cfg(all(test, any(target_os = "linux", target_os = "android")))]
mod tests {
    use super::*;

    #[test]
    fn stale_mount_points_covers_ours_but_not_unrelated() {
        let ksu = Some("KSU");
        let module_file = Some("/data/adb/modules/foo/system/etc/hosts");
        let other_module_runtime = Some("/data/adb/zygisk/bin/libzygisk.so");
        let mounts = [
            (ksu, Path::new("/system")),
            (ksu, Path::new("/mnt/hm_abc")),
            (module_file, Path::new("/system/etc/hosts")),
            (other_module_runtime, Path::new("/system/bin/app_process")),
            (None, Path::new("/data")),
        ];
        let custom = vec![PathBuf::from("/data/adb/hybrid-mount/custom")];
        let partitions = vec!["system".to_string()];

        let points = stale_mount_points(
            mounts,
            "KSU",
            Path::new("/data/adb/modules"),
            &custom,
            &partitions,
        );

        assert_eq!(
            points,
            vec![
                PathBuf::from("/system/etc/hosts"),
                PathBuf::from("/mnt/hm_abc"),
                PathBuf::from("/system"),
            ]
        );
    }

    #[test]
    fn stale_mount_points_includes_exact_custom_targets() {
        let target = PathBuf::from("/data/custom/target");
        let mounts = [(None, target.as_path())];
        let custom = vec![target.clone()];

        let points =
            stale_mount_points(mounts, "KSU", Path::new("/data/adb/modules"), &custom, &[]);

        assert_eq!(points, vec![target]);
    }

    #[test]
    fn stale_mount_points_ignores_module_sources_outside_managed_roots() {
        let mounts = [(
            Some("/data/adb/modules/foo/system/etc/hosts"),
            Path::new("/data/not-managed"),
        )];

        let points = stale_mount_points(
            mounts,
            "KSU",
            Path::new("/data/adb/modules"),
            &[],
            &["system".to_string()],
        );

        assert!(points.is_empty());
    }
}
