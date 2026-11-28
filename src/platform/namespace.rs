use std::fs;
use std::os::fd::AsRawFd;
use std::os::unix::fs::MetadataExt;

use crate::errors::Result;

pub fn namespace_at(path: &str) -> Result<String> {
    let meta = fs::metadata(path)?;
    Ok(format!("{}:{}", meta.dev(), meta.ino()))
}

pub fn enter_init_namespace() -> Result<()> {
    if namespace_at("/proc/thread-self/ns/mnt")? != namespace_at("/proc/1/ns/mnt")? {
        let ns = fs::File::open("/proc/1/ns/mnt")?;
        // SAFETY: ns is a live mount namespace descriptor; CLI mutators are single-threaded.
        if unsafe { libc::setns(ns.as_raw_fd(), libc::CLONE_NEWNS) } != 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        std::env::set_current_dir("/")?;
    }
    Ok(())
}

/// Kernel boot identity. Mount ids and mount ownership never cross this boundary.
pub fn boot_id() -> Result<String> {
    Ok(fs::read_to_string("/proc/sys/kernel/random/boot_id")?
        .trim()
        .to_owned())
}
