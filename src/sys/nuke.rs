// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::Path;

use ksu::NukeExt4Sysfs;

pub fn nuke_path(path: &Path) {
    let mut nuke = NukeExt4Sysfs::new();
    nuke.add(path);
    if let Err(e) = nuke.execute() {
        log::warn!("Failed to nuke {}: {:#}", path.display(), e);
    } else {
        log::debug!("Nuke successful: {}", path.display());
    }
}
