// SPDX-License-Identifier: GPL-3.0-only

//! Mount helpers (Linux/Android only): mountpoint probing, tmpfs mounts and image repair.
//!
//! `unmount` semantics: every "unmount" in this file is the rustix `unmount` syscall, meaning
//! it happens immediately. It is not the same as registering with the KernelSU try-umount list.

use std::path::Path;
use std::time::Duration;

use rustix::mount::{MountFlags, UnmountFlags, mount, unmount};

use crate::errors::{CausalError, ContextError, Error, Result};
use crate::sys::mountinfo::MountSnapshot;
use crate::sys::process::{CaptureMode, CommandSpec, run_command};
use crate::utils::ensure_dir_exists;

/// Repairing a large image with e2fsck can take a while, but a boot-path wait must stay bounded.
const E2FSCK_TIMEOUT: Duration = Duration::from_secs(300);
/// v4.2.0 semantics: exit codes 0..=3 succeed, 4 and above fail, and a signal termination fails.
pub const E2FSCK_COMPATIBLE_EXIT_CODES: &[i32] = &[0, 1, 2, 3];

/// Decides from `/proc/self/mountinfo` whether a path is a mountpoint.
pub fn is_mounted(path: &Path) -> Result<bool> {
    Ok(MountSnapshot::read()?.contains(path))
}

/// Best-effort probe for the Drop cleanup path: a failed lookup records the reason and
/// treats the path as unmounted. Normal paths must use [`is_mounted`], which returns the error.
pub fn is_mounted_best_effort(path: &Path) -> bool {
    match is_mounted(path) {
        Ok(mounted) => mounted,
        Err(err) => {
            log::warn!(
                "mount probe failed, assuming unmounted: path={}, error={err}",
                path.display()
            );
            false
        }
    }
}

/// Rollback one mount target: deepest descendants first, empty-flags unmount
/// with a lazy fallback. Final equivalence against the pre-execution snapshot
/// is verified by the pipeline after all actions have run.
pub fn rollback_mount_target(path: &Path) -> Result<()> {
    let snapshot = MountSnapshot::read()?;
    let mut targets = snapshot.descendants(path);
    targets.push(path);

    for target in targets {
        if crate::sys::faults::should_fail_next_unmount_ebusy() {
            return Err(Error::Mount(Box::new(ContextError::new(
                "unmount mount target",
                Some(target.to_path_buf()),
                CausalError::Message("injected EBUSY".to_owned()),
            ))));
        }
        match unmount(target, UnmountFlags::empty()) {
            Ok(()) => log::info!("mount rollback complete: target={}", target.display()),
            Err(err) if matches!(err, rustix::io::Errno::NOENT | rustix::io::Errno::INVAL) => {
                log::debug!(
                    "mount already gone: target={}, errno={err}",
                    target.display()
                );
            }
            Err(err) => {
                log::warn!(
                    "mount rollback busy, falling back to lazy unmount: target={}, errno={err}",
                    target.display()
                );
                unmount(target, UnmountFlags::DETACH).map_err(|source| {
                    Error::Mount(Box::new(ContextError::new(
                        "lazy unmount mount target",
                        Some(target.to_path_buf()),
                        source,
                    )))
                })?;
            }
        }
    }

    Ok(())
}

/// Mounts tmpfs (`mode=0755`) for overlay staging (v4.2.0 behaviour).
pub fn mount_tmpfs(target: &Path, source: &str) -> Result<()> {
    ensure_dir_exists(target)?;
    mount(
        source,
        target,
        c"tmpfs",
        MountFlags::empty(),
        Some(c"mode=0755"),
    )
    .map_err(|source| {
        Error::Mount(Box::new(ContextError::new(
            "mount tmpfs staging",
            Some(target.to_path_buf()),
            source,
        )))
    })
}

/// Repairs an image with `e2fsck -y -f`; exit codes 0..=3 succeed (v4.2.0 behaviour).
pub fn repair_image(image_path: &Path) -> Result<()> {
    let spec = CommandSpec::new("e2fsck")
        .operation("repair ext4 image")
        .args(["-y", "-f"])
        .arg(image_path.display().to_string())
        .capture(CaptureMode::Both)
        .accepted_exit_codes(E2FSCK_COMPATIBLE_EXIT_CODES)
        .timeout(E2FSCK_TIMEOUT);

    let outcome = run_command(&spec).map_err(|err| {
        Error::Storage(Box::new(ContextError::new(
            "e2fsck repair ext4 image",
            Some(image_path.to_path_buf()),
            CausalError::from(err),
        )))
    })?;
    if let Some(stderr) = outcome
        .stderr_text()
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
    {
        log::debug!("e2fsck output: {stderr}");
    }
    Ok(())
}

/// `emulated-soft-reboot`: immediately unmounts every mountpoint whose source matches,
/// simulating the mount cleanup before a soft reboot.
pub fn emulated_soft_reboot(source: &str) -> Result<()> {
    let entries = crate::sys::mountinfo::mount_entries()?;

    let mut mount_points = entries
        .into_iter()
        .filter(|entry| entry.mount_source.as_deref() == Some(source))
        .filter(|entry| entry.fs_type != "overlay")
        .map(|entry| entry.mount_point)
        .collect::<Vec<_>>();
    crate::sys::mountinfo::deepest_first(&mut mount_points);

    for mount_point in mount_points {
        log::debug!(
            "unmounting {} from {source} in emulated-soft-reboot",
            mount_point.display()
        );
        unmount(&mount_point, UnmountFlags::DETACH).map_err(|source| {
            Error::Mount(Box::new(ContextError::new(
                "unmount in emulated soft reboot",
                Some(mount_point.to_path_buf()),
                source,
            )))
        })?;
    }
    Ok(())
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use crate::sys::faults;
    use std::fs;
    use std::path::{Path, PathBuf};

    #[test]
    fn e2fsck_exit_code_contract_accepts_zero_through_three() {
        for code in 0..=3 {
            assert!(E2FSCK_COMPATIBLE_EXIT_CODES.contains(&code));
        }
        for code in [4, 8, 255] {
            assert!(!E2FSCK_COMPATIBLE_EXIT_CODES.contains(&code));
        }
    }

    fn unshare_mount_namespace() -> bool {
        match unsafe { rustix::thread::unshare_unsafe(rustix::thread::UnshareFlags::NEWNS) } {
            Ok(()) => true,
            Err(err) => {
                eprintln!("skipping mount namespace test: unshare failed: {err}");
                false
            }
        }
    }

    fn test_root(tag: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("hybrid-mount-mount-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root)
            .unwrap_or_else(|err| panic!("create test root {}: {err}", root.display()));
        root
    }

    fn cleanup_root(root: &Path) {
        let _ = unmount(root, UnmountFlags::DETACH);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rollback_mount_target_removes_nested_mounts_and_confirms() {
        if !unshare_mount_namespace() {
            return;
        }
        let root = test_root("rollback");
        let parent = root.join("parent");
        let child = parent.join("child");
        fs::create_dir_all(&child).unwrap();

        if mount("hybrid-test", &parent, c"tmpfs", MountFlags::empty(), None).is_err()
            || mount("hybrid-test", &child, c"tmpfs", MountFlags::empty(), None).is_err()
        {
            eprintln!("skipping rollback test: nested tmpfs mounts are unavailable");
            cleanup_root(&root);
            return;
        }

        assert!(rollback_mount_target(&parent).is_ok());
        let snapshot = MountSnapshot::read().unwrap();
        assert!(!snapshot.contains(&parent));
        assert!(!snapshot.contains(&child));

        cleanup_root(&root);
    }

    #[test]
    fn injected_ebusy_fails_rollback_with_target() {
        let _fault_guard = faults::test_lock();
        if !unshare_mount_namespace() {
            return;
        }
        let root = test_root("ebusy");
        let parent = root.join("parent");
        fs::create_dir_all(&parent).unwrap();
        if mount("hybrid-test", &parent, c"tmpfs", MountFlags::empty(), None).is_err() {
            eprintln!("skipping EBUSY injection test: tmpfs mount unavailable");
            cleanup_root(&root);
            return;
        }

        faults::enable_next_unmount_ebusy_failure();
        let err = rollback_mount_target(&parent).unwrap_err();
        let message = err.to_string();
        assert!(message.contains("injected EBUSY"), "{message}");
        assert!(message.contains(&parent.display().to_string()), "{message}");

        faults::reset();
        cleanup_root(&root);
    }

    #[test]
    fn injected_mountinfo_failure_propagates_from_rollback() {
        let _fault_guard = faults::test_lock();
        if !unshare_mount_namespace() {
            return;
        }
        faults::enable_mountinfo_read_failure();
        let err = rollback_mount_target(Path::new("/unused")).unwrap_err();
        assert!(err.to_string().contains("injected mountinfo"), "{err}");
        faults::reset();
    }
}
