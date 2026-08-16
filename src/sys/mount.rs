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

#[derive(Debug, Default)]
pub struct MountRollback {
    targets: Vec<PathBuf>,
}

impl MountRollback {
    pub fn record(&mut self, target: impl Into<PathBuf>) {
        self.targets.push(target.into());
    }

    pub fn extend(&mut self, targets: impl IntoIterator<Item = PathBuf>) {
        self.targets.extend(targets);
    }

    pub fn into_targets(self) -> Vec<PathBuf> {
        self.targets
    }

    pub fn rollback(&mut self) -> Result<()> {
        self.rollback_with(detach_mount)
    }

    pub fn attach_rollback(&mut self, original: anyhow::Error) -> anyhow::Error {
        match self.rollback() {
            Ok(()) => original,
            Err(rollback_error) => original.context(format!(
                "additionally failed to roll back mounted targets: {rollback_error:#}"
            )),
        }
    }

    pub(crate) fn rollback_with<F>(&mut self, mut detach: F) -> Result<()>
    where
        F: FnMut(&Path) -> Result<()>,
    {
        let targets = std::mem::take(&mut self.targets);
        let mut failures = Vec::new();

        for target in targets.iter().rev() {
            if let Err(error) = detach(target) {
                failures.push(format!("{}: {error:#}", target.display()));
            }
        }

        if failures.is_empty() {
            Ok(())
        } else {
            anyhow::bail!(failures.join("; "))
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn detach_mount(target: &Path) -> Result<()> {
    use rustix::mount::{UnmountFlags, unmount};

    unmount(target, UnmountFlags::DETACH)
        .with_context(|| format!("failed to detach mount {}", target.display()))
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn detach_mount(_target: &Path) -> Result<()> {
    bail!("mount detaching is only supported on linux/android")
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn overlay_options_have_hybrid_path(
    options: &std::collections::HashMap<String, Option<String>>,
) -> bool {
    ["lowerdir", "upperdir", "workdir"]
        .into_iter()
        .filter_map(|key| options.get(key).and_then(Option::as_deref))
        .flat_map(|value| value.split(':'))
        .map(Path::new)
        .any(|path| {
            crate::utils::is_mount_workspace_path(path)
                || path.starts_with(crate::defs::HYBRID_MOUNT_DIR)
        })
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn is_stale_mount(mount_point: &Path, hybrid_overlay: bool, exact_targets: &[PathBuf]) -> bool {
    // Overlay ownership is established by an option pointing into our
    // workspace/data root, never by the shared KSU/APatch source label.
    if hybrid_overlay {
        return true;
    }

    if crate::utils::is_mount_workspace_path(mount_point) {
        return true;
    }

    exact_targets.iter().any(|target| target == mount_point)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn stale_mount_points<'a>(
    mounts: impl IntoIterator<Item = (&'a Path, bool)>,
    exact_targets: &[PathBuf],
) -> Vec<PathBuf> {
    let mut points: Vec<PathBuf> = mounts
        .into_iter()
        .filter(|(mount_point, hybrid_overlay)| {
            is_stale_mount(mount_point, *hybrid_overlay, exact_targets)
        })
        .map(|(mount_point, _)| mount_point.to_path_buf())
        .collect();

    // Unmount deeper children before their parents (storage tree roots,
    // overlay roots) so a DETACH never leaves children behind. Do not dedupe:
    // repeated mountinfo entries at one target are stacked mounts and require
    // one detach per layer.
    points.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    points
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn unmount_stale_mounts(source: &str, exact_targets: &[PathBuf]) -> Result<usize> {
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
        mounts.iter().map(|mount| {
            (
                mount.mount_point.as_path(),
                mount.fs_type == "overlay"
                    && overlay_options_have_hybrid_path(&mount.super_options),
            )
        }),
        exact_targets,
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
        match detach_mount(mount_point) {
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
pub fn unmount_stale_mounts(_source: &str, _exact_targets: &[PathBuf]) -> Result<usize> {
    bail!("stale mount cleanup is only supported on linux/android")
}

#[cfg(all(test, any(target_os = "linux", target_os = "android")))]
mod tests {
    use super::*;

    #[test]
    fn stale_mount_points_covers_ours_but_not_unrelated() {
        let mounts = [
            (Path::new("/system"), true),
            (Path::new("/mnt/hm_a1B2c3D4e5"), false),
            (Path::new("/system/etc/hosts"), false),
            (Path::new("/system/bin/app_process"), false),
            (Path::new("/data"), false),
        ];
        let exact = vec![PathBuf::from("/system/etc/hosts")];

        let points = stale_mount_points(mounts, &exact);

        assert_eq!(
            points,
            vec![
                PathBuf::from("/system/etc/hosts"),
                PathBuf::from("/mnt/hm_a1B2c3D4e5"),
                PathBuf::from("/system"),
            ]
        );
    }

    #[test]
    fn stale_mount_points_includes_exact_custom_targets() {
        let target = PathBuf::from("/data/custom/target");
        let mounts = [(target.as_path(), false)];
        let exact = vec![target.clone()];

        let points = stale_mount_points(mounts, &exact);

        assert_eq!(points, vec![target]);
    }

    #[test]
    fn stale_mount_points_preserves_stacked_exact_mounts() {
        let target = PathBuf::from("/system/etc/hosts");
        let mounts = [(target.as_path(), false), (target.as_path(), false)];

        let points = stale_mount_points(mounts, std::slice::from_ref(&target));

        assert_eq!(points, vec![target.clone(), target]);
    }

    #[test]
    fn stale_mount_points_ignores_unpersisted_targets() {
        let mounts = [(Path::new("/data/not-managed"), false)];

        let points = stale_mount_points(mounts, &[]);

        assert!(points.is_empty());
    }

    #[test]
    fn stale_mount_points_does_not_guess_partition_ownership() {
        let mounts = [
            (Path::new("/system"), false),
            (Path::new("/system/etc/other-module.conf"), false),
        ];

        let points = stale_mount_points(mounts, &[]);

        assert!(points.is_empty());
    }

    #[test]
    fn overlay_marker_requires_a_hybrid_owned_path() {
        let mut options = std::collections::HashMap::new();
        options.insert(
            "lowerdir".to_string(),
            Some("/mnt/hm_a1B2c3D4e5/module/system:/system".to_string()),
        );
        assert!(overlay_options_have_hybrid_path(&options));

        options.insert(
            "lowerdir".to_string(),
            Some("/data/adb/other-module/system:/system".to_string()),
        );
        assert!(!overlay_options_have_hybrid_path(&options));
    }

    #[test]
    fn mount_rollback_detaches_in_reverse_and_keeps_trying() -> Result<()> {
        let mut rollback = MountRollback::default();
        rollback.record("/system");
        rollback.record("/system/etc/hosts");
        rollback.record("/vendor");
        let mut attempted = Vec::new();

        let error = rollback
            .rollback_with(|path| {
                attempted.push(path.to_path_buf());
                if path == Path::new("/system/etc/hosts") {
                    anyhow::bail!("busy")
                }
                Ok(())
            })
            .unwrap_err();

        assert_eq!(
            attempted,
            vec![
                PathBuf::from("/vendor"),
                PathBuf::from("/system/etc/hosts"),
                PathBuf::from("/system"),
            ]
        );
        assert!(error.to_string().contains("/system/etc/hosts"));

        attempted.clear();
        rollback.rollback_with(|path| {
            attempted.push(path.to_path_buf());
            Ok(())
        })?;
        assert!(attempted.is_empty());
        Ok(())
    }
}
