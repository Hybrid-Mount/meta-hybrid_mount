// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use crate::{
    conf::config,
    core::runtime_state::MountStatistics,
    mount::custom_bind::{self, CustomBindKind},
};

pub(super) fn mount_custom_binds(config: &config::Config) -> (Vec<String>, MountStatistics) {
    let mut stats = MountStatistics::default();

    if config.custom_mounts.is_empty() {
        crate::scoped_log!(debug, "executor:custom_bind", "skip: entries=0");
        return (Vec::new(), stats);
    }

    crate::scoped_log!(
        info,
        "executor:custom_bind",
        "start: entries={}",
        config.custom_mounts.len()
    );

    let report =
        custom_bind::apply_custom_bind_mounts(&config.custom_mounts, config.disable_umount);

    for mounted in &report.mounted {
        match mounted.kind {
            CustomBindKind::File => stats.record_file(),
            CustomBindKind::Directory => stats.record_dir(),
        }
    }

    for failed in &report.failed {
        stats.record_failed();
        crate::scoped_log!(
            warn,
            "executor:custom_bind",
            "entry failed: source={}, target={}, error={}",
            failed.source.display(),
            failed.target.display(),
            failed.error
        );
    }

    let targets = report
        .mounted
        .into_iter()
        .map(|mount| mount.target.display().to_string())
        .collect::<Vec<_>>();

    crate::scoped_log!(
        info,
        "executor:custom_bind",
        "complete: mounted={}, failed={}",
        targets.len(),
        report.failed.len()
    );

    (targets, stats)
}
