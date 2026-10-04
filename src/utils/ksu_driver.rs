// Copyright (C) 2026 meta-magic_mount-rs developers
// SPDX-License-Identifier: GPL-3.0-only

//! Owned KernelSU driver transport. Prefer an inherited su-session descriptor.

use std::fs;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::sync::Mutex;

use super::ksu_protocol::{self, GetInfo, MANAGE_TRY_UMOUNT};
use super::ksu_umount::UmountCommand;

// Cache only successful discovery. Failed attempts remain retryable.
static DRIVER: Mutex<Option<OwnedFd>> = Mutex::new(None);

fn scan_driver_fd(dir: &Path) -> io::Result<Option<RawFd>> {
    let mut driver = None;
    for entry in fs::read_dir(dir)?.flatten() {
        let Ok(fd) = entry.file_name().to_string_lossy().parse::<RawFd>() else {
            continue;
        };
        if fd < 0 {
            continue;
        }
        let Ok(target) = fs::read_link(entry.path()) else {
            continue;
        };
        if target == Path::new("anon_inode:[ksu_driver_su]") {
            return Ok(Some(fd));
        }
        if target == Path::new("anon_inode:[ksu_driver]") {
            driver = Some(fd);
        }
    }
    Ok(driver)
}

fn duplicate_driver(fd: RawFd) -> io::Result<OwnedFd> {
    // SAFETY: fcntl duplicates the descriptor without taking ownership of the inherited fd.
    let duplicate = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 0) };
    if duplicate < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: fcntl returned a new descriptor owned by this process.
    Ok(unsafe { OwnedFd::from_raw_fd(duplicate) })
}

fn open_driver() -> io::Result<OwnedFd> {
    if let Some(fd) = scan_driver_fd(Path::new("/proc/self/fd")).ok().flatten() {
        // Preserve su-session permissions and the original descriptor's owner.
        return duplicate_driver(fd);
    }
    let mut fd: RawFd = -1;
    // SAFETY: KernelSU intercepts these magic reboot arguments and writes a new fd
    // to the valid output pointer. Other kernels reject the invalid magic values.
    unsafe {
        libc::syscall(libc::SYS_reboot, 0xDEADBEEFu32, 0xCAFEBABEu32, 0, &mut fd);
    }
    if fd < 0 {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "KernelSU driver descriptor unavailable",
        ));
    }
    // SAFETY: KernelSU installed a new descriptor owned by this process.
    let owned = unsafe { OwnedFd::from_raw_fd(fd) };
    // SAFETY: owned is live. The driver is private to this process, including across exec.
    if unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(owned)
}

fn with_driver<T>(
    cache: &Mutex<Option<OwnedFd>>,
    open: impl FnOnce() -> io::Result<OwnedFd>,
    call: impl FnOnce(&OwnedFd) -> io::Result<T>,
) -> io::Result<T> {
    let mut cached = cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if cached.is_none() {
        *cached = Some(open()?);
    }
    match cached.as_ref() {
        Some(fd) => call(fd),
        None => Err(io::Error::other("KernelSU driver cache unavailable")),
    }
}

fn ioctl<T>(request: u32, cmd: &mut T) -> io::Result<()> {
    with_driver(&DRIVER, open_driver, |fd| {
        // SAFETY: private callers pass the matching repr(C) UAPI buffer. The descriptor,
        // buffer and any nested CString remain live until the synchronous ioctl returns.
        let result = unsafe { libc::ioctl(fd.as_raw_fd(), request as _, cmd as *mut T) };
        if result < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    })
}

pub(super) fn info() -> io::Result<GetInfo> {
    ksu_protocol::get_info_with(ioctl)
}

pub(super) fn kpm_enabled() -> Option<bool> {
    ksu_protocol::kpm_with(ioctl)
}

pub(super) fn manage_umount(command: &UmountCommand) -> io::Result<()> {
    let mut command = *command;
    ioctl(MANAGE_TRY_UMOUNT, &mut command)
}

pub(super) fn nuke_ext4_sysfs(path: &Path) -> io::Result<()> {
    ksu_protocol::nuke_with(path.as_os_str().as_bytes(), ioctl)
}

#[cfg(test)]
#[path = "ksu_driver_tests.rs"]
mod tests;
