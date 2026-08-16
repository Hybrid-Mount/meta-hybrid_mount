// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use anyhow::Result;

use crate::conf::config::Config;

/// Detach mounts created by a previous Hybrid Mount run before KernelSU's
/// emulated soft reboot re-runs the metamodule mount script.
///
/// The detection covers every mount family this project creates:
/// - storage tmpfs/ext4 and overlay mounts (mount source namespace);
/// - backing/staging trees under `/mnt/hm_*`;
/// - Magic Mount file binds sourced from the module directory on managed
///   partition roots.
pub fn detach_stale_mounts(config: &Config) -> Result<usize> {
    if config.disable_umount {
        crate::scoped_log!(debug, "late_load", "cleanup skipped: reason=disable_umount");
        return Ok(0);
    }

    let managed_partitions = crate::partitions::managed_partition_names();

    crate::sys::mount::unmount_stale_mounts(
        &config.mountsource,
        &config.moduledir,
        &[],
        &managed_partitions,
    )
}
