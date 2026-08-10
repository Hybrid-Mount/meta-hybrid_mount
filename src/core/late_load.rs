// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::path::PathBuf;

use anyhow::Result;

use crate::conf::config::Config;

/// Detach mounts created by a previous Hybrid Mount run before KernelSU's
/// emulated soft reboot re-runs the metamodule mount script.
///
/// The detection covers every mount family this project creates:
/// - storage tmpfs/ext4 and overlay mounts (mount source namespace);
/// - backing/staging trees under `/mnt/hm_*`;
/// - Magic Mount file binds sourced from the module directory on managed
///   partition roots;
/// - configured custom bind targets.
pub fn detach_stale_mounts(config: &Config) -> Result<usize> {
    if config.disable_umount {
        crate::scoped_log!(debug, "late_load", "cleanup skipped: reason=disable_umount");
        return Ok(0);
    }

    let custom_targets: Vec<PathBuf> = config
        .custom_mounts
        .iter()
        .map(|mount| mount.target.clone())
        .collect();
    let managed_partitions = crate::partitions::managed_partition_names();

    crate::sys::mount::unmount_stale_mounts(
        &config.mountsource,
        &config.moduledir,
        &custom_targets,
        &managed_partitions,
    )
}
