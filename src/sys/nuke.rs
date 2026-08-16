// ReHybrid-Mount
//
// SPDX-License-Identifier: GPL-3.0-only

//! KernelSU nuke 辅助(仅 Linux/Android):清空刚挂载的 ext4 staging
//! 内容(v4.2.0 `reset_mount_state` 行为)。非 KernelSU 环境跳过。

use std::path::Path;

use ::ksu::NukeExt4Sysfs;

use crate::utils::ksu;

pub fn nuke_path(path: &Path) {
    if !ksu::is_active() {
        log::debug!("nuke skipped: path={}, reason=non_ksu", path.display());
        return;
    }

    let mut nuke = NukeExt4Sysfs::new();
    nuke.add(path);
    if let Err(err) = nuke.execute() {
        log::warn!("nuke failed: path={}, error={err}", path.display());
    } else {
        log::debug!("nuke success: path={}", path.display());
    }
}
