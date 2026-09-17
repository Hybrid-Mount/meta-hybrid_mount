// SPDX-License-Identifier: GPL-3.0-only

//! KernelSU try-umount list integration.
//! Deduplication and ignored partitions follow this repo's v4.2.0 `umount_mgr` semantics.
//!
//! Note the boundary: this only **registers** mountpoints with the kernel list and never
//! unmounts immediately, which requires the rustix `unmount` syscall.

use std::collections::HashSet;
use std::path::Path;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use ::ksu::{TryUmount, TryUmountFlags};

use crate::errors::{ContextError, Error, Result};
use crate::utils::is_ignored_unmount_partition;

static KSU_ACTIVE: AtomicBool = AtomicBool::new(false);
static UMOUNT_BROKEN: AtomicBool = AtomicBool::new(false);
static TRY_UMOUNT_LIST: OnceLock<Mutex<TryUmount>> = OnceLock::new();
static REGISTERED_PATHS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

/// Detects whether KernelSU is available at boot; major version 4 disables the umount list.
pub fn init() {
    let active = ksu::version().is_some_and(|version| {
        log::info!("KernelSU Version: {version}");
        if version.to_string().starts_with('4') {
            log::warn!(
                "the ioctl function of this KernelSU line is broken, umount list is disabled"
            );
            UMOUNT_BROKEN.store(true, Ordering::Relaxed);
        }
        true
    });

    KSU_ACTIVE.store(active, Ordering::Relaxed);
}

pub fn is_active() -> bool {
    KSU_ACTIVE.load(Ordering::Relaxed)
}

/// Adds a mountpoint to the KernelSU try-umount list, a no-op when unavailable, disabled, ignored or already present.
pub fn send_unmountable(target: impl AsRef<Path>) {
    if !is_active() || UMOUNT_BROKEN.load(Ordering::Relaxed) {
        return;
    }

    let path = target.as_ref();
    let Some(path_str) = path.to_str() else {
        return;
    };

    if is_ignored_unmount_partition(path_str) {
        log::debug!("skip try-umount registration: path={path_str}, reason=ignore_partition");
        return;
    }

    let mut history = REGISTERED_PATHS
        .get_or_init(|| Mutex::new(HashSet::new()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    if !history.insert(path_str.to_owned()) {
        return;
    }
    drop(history);

    TRY_UMOUNT_LIST
        .get_or_init(|| Mutex::new(TryUmount::new()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .add(path);
}

/// Commits the try-umount list (`MNT_DETACH`); called once the pipeline finishes.
pub fn commit_unmount_list() -> Result<()> {
    if crate::sys::faults::should_fail_ksu_commit() {
        return Err(Error::Mount(Box::new(ContextError::new(
            "commit KernelSU try-umount list",
            None,
            "injected KernelSU try-umount commit failure".to_owned(),
        ))));
    }
    if !is_active() {
        return Ok(());
    }

    let mut control = TRY_UMOUNT_LIST
        .get_or_init(|| Mutex::new(TryUmount::new()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    control.flags(TryUmountFlags::MNT_DETACH);
    control.format_msg(|paths| format!("umount {paths:?} successful"));
    control.umount().map_err(|err| {
        Error::Mount(Box::new(ContextError::new(
            "commit KernelSU try-umount list",
            None,
            err.to_string(),
        )))
    })?;
    Ok(())
}

/// Clears the kernel try-umount list and this process's registration history, for failure rollback only.
pub fn clear_unmount_list() -> Result<()> {
    if !is_active() {
        return Ok(());
    }

    TRY_UMOUNT_LIST
        .get_or_init(|| Mutex::new(TryUmount::new()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .wipe()
        .map_err(|err| {
            Error::Mount(Box::new(ContextError::new(
                "wipe KernelSU try-umount list",
                None,
                err.to_string(),
            )))
        })?;

    if let Some(history) = REGISTERED_PATHS.get() {
        history
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
    }
    Ok(())
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn ksu_commit_failure_injection_is_one_shot() {
        let _fault_guard = crate::sys::faults::test_lock();
        crate::sys::faults::enable_ksu_commit_failure();
        let err = commit_unmount_list().unwrap_err();
        assert!(err.to_string().contains("injected KernelSU"), "{err}");
        crate::sys::faults::reset();

        assert!(commit_unmount_list().is_ok());
        assert!(clear_unmount_list().is_ok());
    }
}
