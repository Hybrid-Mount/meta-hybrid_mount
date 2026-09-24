// SPDX-License-Identifier: GPL-3.0-only

//! Mount helpers (Linux/Android only): mountpoint probing, tmpfs mounts and image repair.
//!
//! `unmount` semantics: every "unmount" in this file is the rustix `unmount` syscall, meaning
//! it happens immediately. It is not the same as registering with the KernelSU try-umount list.

use std::path::Path;
use std::time::Duration;

use rustix::mount::{
    MountFlags, MountPropagationFlags, UnmountFlags, mount, mount_change, unmount,
};

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

/// Unshares a mount Hybrid Mount created from every peer group it inherited.
///
/// A cloned mount keeps the propagation type of its source, so a bind of a shared mount joins
/// that source's peer group, and a mount attached below a shared parent can be handed a fresh
/// group id. Either one makes our own targets stand out in `/proc/self/mountinfo` as `shared:N`
/// next to the OverlayFS targets, which carry no propagation field at all, and both draw ids from
/// the kernel's global group allocator.
///
/// `MS_PRIVATE` is local to our own mount: peers of the source keep their group, and only this
/// mount leaves it. `recursive` covers the subtree a directory move carried along, while a single
/// bind only needs the non-recursive form.
pub fn normalize_propagation(path: &Path, recursive: bool) -> Result<()> {
    let flags = if recursive {
        MountPropagationFlags::PRIVATE | MountPropagationFlags::REC
    } else {
        MountPropagationFlags::PRIVATE
    };

    mount_change(path, flags).map_err(|source| {
        Error::Mount(Box::new(ContextError::new(
            "make mount target private",
            Some(path.to_path_buf()),
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

    struct TmpfsMount(PathBuf);

    impl TmpfsMount {
        fn new(path: &Path) -> std::io::Result<Self> {
            fs::create_dir_all(path)?;
            mount("hybrid-test", path, c"tmpfs", MountFlags::empty(), None)?;
            Ok(Self(path.to_path_buf()))
        }
    }

    impl Drop for TmpfsMount {
        fn drop(&mut self) {
            let _ = unmount(&self.0, UnmountFlags::DETACH);
        }
    }

    #[test]
    fn rollback_mount_target_removes_nested_mounts_and_confirms() {
        let _fault_guard = faults::test_lock();
        if !crate::test_support::require_mount_namespace() {
            return;
        }
        let root = crate::test_support::Fixture::new("rollback");
        let parent = root.join("parent");
        let child = parent.join("child");
        let _parent_mount = TmpfsMount::new(&parent).unwrap();
        // Create the child after mounting its parent, or the parent hides it.
        let _child_mount = TmpfsMount::new(&child).unwrap();

        assert!(rollback_mount_target(&parent).is_ok());
        let snapshot = MountSnapshot::read().unwrap();
        assert!(!snapshot.contains(&parent));
        assert!(!snapshot.contains(&child));
    }

    #[test]
    fn injected_ebusy_fails_rollback_with_target() {
        let _fault_guard = faults::test_lock();
        if !crate::test_support::require_mount_namespace() {
            return;
        }
        let root = crate::test_support::Fixture::new("ebusy");
        let parent = root.join("parent");
        let _parent_mount = TmpfsMount::new(&parent).unwrap();

        faults::enable_next_unmount_ebusy_failure();
        let err = rollback_mount_target(&parent).unwrap_err();
        let message = err.to_string();
        assert!(message.contains("injected EBUSY"), "{message}");
        assert!(message.contains(&parent.display().to_string()), "{message}");

        faults::reset();
    }

    #[test]
    fn injected_mountinfo_failure_propagates_from_rollback() {
        let _fault_guard = faults::test_lock();
        faults::enable_mountinfo_read_failure();
        let err = rollback_mount_target(Path::new("/unused")).unwrap_err();
        assert!(err.to_string().contains("injected mountinfo"), "{err}");
        faults::reset();
    }
}
