// SPDX-License-Identifier: GPL-3.0-only

//! KernelSU try-umount list integration.
//! Deduplication and ignored partitions follow this repo's v4.2.0 `umount_mgr` semantics.
//!
//! Note the boundary: this only **registers** mountpoints with the kernel list and never
//! unmounts immediately, which requires the rustix `unmount` syscall.

use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::path::Path;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use super::ksu_umount::{self, Registrations, UmountCommand};
use crate::errors::{ContextError, Error, Result};
use crate::utils::is_ignored_unmount_partition;

static KSU_ACTIVE: AtomicBool = AtomicBool::new(false);
static UMOUNT_BROKEN: AtomicBool = AtomicBool::new(false);
static REGISTERED_PATHS: OnceLock<Mutex<Registrations>> = OnceLock::new();

fn registered_paths() -> &'static Mutex<Registrations> {
    REGISTERED_PATHS.get_or_init(|| Mutex::new(Registrations::default()))
}

/// Use the documented ioctl directly: ksu 0.2.0's del() mistakenly sends WIPE.
/// Keep the descriptor owned and never treat a failed installation as a valid fd.
fn open_driver() -> std::io::Result<OwnedFd> {
    let mut fd: libc::c_int = -1;
    // SAFETY: KernelSU intercepts these magic reboot arguments and writes a new fd
    // into the valid output pointer. Other kernels reject the invalid magic values.
    unsafe {
        libc::syscall(libc::SYS_reboot, 0xDEADBEEFu32, 0xCAFEBABEu32, 0, &mut fd);
    }
    if fd < 0 {
        return Err(std::io::Error::other(
            "KernelSU driver descriptor unavailable",
        ));
    }
    // SAFETY: KernelSU returned a newly installed descriptor owned by this process.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

fn kernel_command(command: &UmountCommand) -> std::io::Result<()> {
    let fd = open_driver()?;
    // The ioctl has an empty encoded size despite carrying the UAPI struct.
    let request = libc::_IOW::<()>(u32::from(b'K'), 18);
    // SAFETY: fd is live; command's path pointer stays valid through this call.
    let ret = unsafe { libc::ioctl(fd.as_raw_fd(), request as _, command) };
    if ret < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub fn committed_unmounts() -> Vec<String> {
    registered_paths()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .committed()
}

/// Release persisted registrations in a new runtime process, without touching foreign paths.
pub fn release_unmounts(paths: &[String]) -> Result<()> {
    if paths.is_empty() {
        return Ok(());
    }
    ksu_umount::release(paths, &mut kernel_command).map_err(|err| {
        Error::Mount(Box::new(ContextError::new(
            "release owned KernelSU try-umount entries",
            None,
            err,
        )))
    })
}

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

    registered_paths()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .queue(path_str);
}

/// Removes a mountpoint again, for a transient mount that was torn down before the rest of
/// the list is replayed for the remainder of the boot.
///
/// The kernel replays every registered path on each later `umount`, so an entry whose mount
/// point no longer exists keeps logging `KernelSU: ksu_handle_umount: unmounting: <path>`
/// for the whole boot.
pub fn withdraw_unmountable(target: impl AsRef<Path>) {
    if !is_active() || UMOUNT_BROKEN.load(Ordering::Relaxed) {
        return;
    }

    let path = target.as_ref();
    let Some(path_str) = path.to_str() else {
        return;
    };

    if let Err(err) = registered_paths()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .withdraw(path_str, &mut kernel_command)
    {
        // Keep failed deletions in the ownership snapshot so cleanup can retry them.
        log::warn!("withdraw KernelSU try-umount entry failed: path={path_str}, error={err}");
    }
}

/// Commits the try-umount list (`MNT_DETACH`); called once the pipeline finishes.
/// Paths withdrawn before this point are not registered.
pub fn commit_unmount_list() -> Result<()> {
    if crate::sys::faults::should_fail_ksu_commit() {
        return Err(Error::Mount(Box::new(ContextError::new(
            "commit KernelSU try-umount list",
            None,
            "injected KernelSU try-umount commit failure".to_owned(),
        ))));
    }
    if !is_active() || UMOUNT_BROKEN.load(Ordering::Relaxed) {
        return Ok(());
    }

    registered_paths()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .commit(&mut kernel_command)
        .map_err(|err| {
            Error::Mount(Box::new(ContextError::new(
                "commit KernelSU try-umount list",
                None,
                err.to_string(),
            )))
        })?;
    Ok(())
}

/// Roll back only registrations this process successfully installed.
pub fn clear_unmount_list() -> Result<()> {
    registered_paths()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .rollback(&mut kernel_command)
        .map_err(|err| {
            Error::Mount(Box::new(ContextError::new(
                "rollback owned KernelSU try-umount entries",
                None,
                err.to_string(),
            )))
        })?;

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
