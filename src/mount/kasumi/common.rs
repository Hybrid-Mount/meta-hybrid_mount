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

use anyhow::{Result, anyhow};

use crate::{conf::config, partitions};

pub(super) fn build_managed_partitions(
    _config: &config::Config,
) -> std::collections::HashSet<String> {
    partitions::managed_partition_set()
}

pub(super) fn effective_stealth_enabled(config: &config::Config) -> bool {
    config.kasumi.enable_stealth || config.kasumi.enable_hidexattr
}

pub(super) fn effective_mount_hide_enabled(config: &config::Config) -> bool {
    config.kasumi.enable_mount_hide
        || config.kasumi.enable_hidexattr
        || config.kasumi.mount_hide.enabled
        || !config.kasumi.mount_hide.path_pattern.as_os_str().is_empty()
}

pub(super) fn effective_maps_spoof_enabled(config: &config::Config) -> bool {
    config.kasumi.enable_maps_spoof
        || config.kasumi.enable_hidexattr
        || !config.kasumi.maps_rules.is_empty()
}

pub(super) fn effective_statfs_spoof_enabled(config: &config::Config) -> bool {
    config.kasumi.enable_statfs_spoof
        || config.kasumi.enable_hidexattr
        || config.kasumi.statfs_spoof.enabled
        || !config.kasumi.statfs_spoof.path.as_os_str().is_empty()
        || config.kasumi.statfs_spoof.spoof_f_type != 0
}

pub(super) fn effective_selinux_fix_enabled(config: &config::Config) -> bool {
    config.kasumi.enable_selinux_fix
}

pub(super) fn has_uname_spoof_config(config: &config::Config) -> bool {
    !config.kasumi.uname.sysname.is_empty()
        || !config.kasumi.uname.nodename.is_empty()
        || !config.kasumi.uname.release.is_empty()
        || !config.kasumi.uname.version.is_empty()
        || !config.kasumi.uname.machine.is_empty()
        || !config.kasumi.uname.domainname.is_empty()
}

pub(super) fn feature_supported(features: Option<i32>, required_feature: i32) -> bool {
    features
        .map(|bits| bits & required_feature != 0)
        .unwrap_or(false)
}

pub(super) fn to_c_ulong(value: u64, field_name: &str) -> Result<libc::c_ulong> {
    libc::c_ulong::try_from(value)
        .map_err(|_| anyhow!("{field_name} value {value} does not fit into c_ulong"))
}

pub(super) fn to_c_long(value: i64, field_name: &str) -> Result<libc::c_long> {
    libc::c_long::try_from(value)
        .map_err(|_| anyhow!("{field_name} value {value} does not fit into c_long"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selinux_fix_requires_the_explicit_toggle() {
        let mut config = config::Config::default();

        config.kasumi.enable_hidexattr = true;
        assert!(effective_stealth_enabled(&config));
        assert!(effective_mount_hide_enabled(&config));
        assert!(effective_maps_spoof_enabled(&config));
        assert!(effective_statfs_spoof_enabled(&config));
        assert!(!effective_selinux_fix_enabled(&config));

        config.kasumi.enable_selinux_fix = true;
        assert!(effective_selinux_fix_enabled(&config));
    }
}
