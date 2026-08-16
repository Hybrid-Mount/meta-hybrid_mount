// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::{collections::BTreeSet, path::PathBuf};

use anyhow::Result;

use crate::{conf::config::Config, core::runtime_state::RuntimeState};

/// Detach mounts created by a previous Hybrid Mount run before KernelSU's
/// emulated soft reboot re-runs the metamodule mount script.
///
/// The detection covers every mount family this project creates:
/// - storage tmpfs/ext4 and overlay mounts proven by owned paths/options;
/// - backing/staging trees in a recognized mount workspace;
/// - exact Magic Mount and custom bind targets persisted by the previous run;
/// - custom bind targets in the current config (including newly configured
///   targets which may already have been mounted by another late-load pass).
pub fn detach_stale_mounts(config: &Config) -> Result<usize> {
    if config.disable_umount {
        crate::scoped_log!(debug, "late_load", "cleanup skipped: reason=disable_umount");
        return Ok(0);
    }

    let previous_state = previous_state_or_warn(RuntimeState::load());
    let exact_targets = cleanup_targets(config, previous_state.as_ref());

    crate::sys::mount::unmount_stale_mounts(&config.mountsource, &exact_targets)
}

fn previous_state_or_warn(result: Result<RuntimeState>) -> Option<RuntimeState> {
    match result {
        Ok(state) => Some(state),
        Err(error) => {
            crate::scoped_log!(
                warn,
                "late_load",
                "runtime state unavailable, using current config targets only: error={:#}",
                error
            );
            None
        }
    }
}

fn cleanup_targets(config: &Config, previous_state: Option<&RuntimeState>) -> Vec<PathBuf> {
    let mut targets: BTreeSet<PathBuf> = config
        .custom_mounts
        .iter()
        .map(|mount| mount.target.clone())
        .filter(|target| target.is_absolute())
        .collect();

    if let Some(state) = previous_state {
        targets.extend(
            state
                .magic_mount_targets
                .iter()
                .chain(&state.custom_mounts)
                .map(PathBuf::from)
                .filter(|target| target.is_absolute()),
        );
    }

    targets.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanup_targets_merge_previous_magic_and_removed_custom_targets() {
        let config = Config {
            custom_mounts: vec![crate::conf::schema::CustomBindMount {
                source: PathBuf::from("/data/local/new"),
                target: PathBuf::from("/system/new"),
            }],
            ..Default::default()
        };
        let mut state = RuntimeState::default();
        state.magic_mount_targets = vec![
            "/vendor/etc/magic.conf".to_string(),
            "relative/ignored".to_string(),
        ];
        state.custom_mounts = vec!["/system/removed".to_string(), "/system/new".to_string()];

        assert_eq!(
            cleanup_targets(&config, Some(&state)),
            vec![
                PathBuf::from("/system/new"),
                PathBuf::from("/system/removed"),
                PathBuf::from("/vendor/etc/magic.conf"),
            ]
        );
    }

    #[test]
    fn cleanup_targets_fall_back_to_current_config_without_runtime_state() {
        let config = Config {
            custom_mounts: vec![crate::conf::schema::CustomBindMount {
                source: PathBuf::from("/data/local/current"),
                target: PathBuf::from("/system/current"),
            }],
            ..Default::default()
        };

        assert_eq!(
            cleanup_targets(&config, None),
            vec![PathBuf::from("/system/current")]
        );
    }

    #[test]
    fn damaged_runtime_state_does_not_block_late_load() {
        let state = previous_state_or_warn(Err(anyhow::anyhow!("invalid runtime state json")));

        assert!(state.is_none());
    }
}
