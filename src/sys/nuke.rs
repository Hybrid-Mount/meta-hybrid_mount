// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::path::Path;

#[cfg(any(target_os = "linux", target_os = "android"))]
use ksu::NukeExt4Sysfs;

pub fn nuke_path(path: &Path) {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        if !crate::utils::KSU.load(std::sync::atomic::Ordering::Relaxed) {
            crate::scoped_log!(
                debug,
                "nuke",
                "execute skipped: path={}, reason=non_ksu",
                path.display()
            );
            return;
        }

        let mut nuke = NukeExt4Sysfs::new();
        nuke.add(path);
        if let Err(e) = nuke.execute() {
            crate::scoped_log!(
                warn,
                "nuke",
                "execute failed: path={}, error={:#}",
                path.display(),
                e
            );
        } else {
            crate::scoped_log!(debug, "nuke", "execute success: path={}", path.display());
        }
    }

    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    let _ = path;
}
