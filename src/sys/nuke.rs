// SPDX-License-Identifier: GPL-3.0-only

//! KernelSU ext4 sysfs nuke integration (Linux/Android only).
//!
//! This hides the staging ext4 mount from KernelSU's ext4 sysfs surface; it
//! does not clear files from the mounted filesystem. Failures remain
//! best-effort so a successful mount is never rolled back only because the
//! optional concealment ioctl is unavailable.

use std::path::Path;

use ::ksu::NukeExt4Sysfs;

use crate::utils::ksu;

pub fn nuke_ext4_sysfs(path: &Path) {
    if !ksu::is_active() {
        log::info!(
            "ext4 sysfs nuke skipped: path={}, reason=non_ksu",
            path.display()
        );
        return;
    }

    log::info!("ext4 sysfs nuke start: path={}", path.display());
    let mut nuke = NukeExt4Sysfs::new();
    nuke.add(path);
    if let Err(err) = nuke.execute() {
        log::warn!(
            "ext4 sysfs nuke failed: path={}, error={err}",
            path.display()
        );
    } else {
        log::info!("ext4 sysfs nuke complete: path={}", path.display());
    }
}
