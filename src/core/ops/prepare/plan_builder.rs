// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::{
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
};

use super::types::ModulePlanOutcome;
use crate::{core::inventory::Module, domain::MountMode};

pub(super) fn queue_overlay(
    plan: &mut ModulePlanOutcome,
    resolved_target: PathBuf,
    partition_label: &str,
    source: PathBuf,
) {
    crate::scoped_log!(
        debug,
        "prepare",
        "queue overlay: partition={}, layer={}, target={}",
        partition_label,
        source.display(),
        resolved_target.display()
    );
    let (_, layers) = plan
        .overlay_groups
        .entry(resolved_target)
        .or_insert_with(|| (partition_label.to_string(), Vec::new()));
    layers.push(source);
}

pub(super) fn merge_overlay_groups(
    target: &mut BTreeMap<PathBuf, (String, Vec<PathBuf>)>,
    source: BTreeMap<PathBuf, (String, Vec<PathBuf>)>,
) {
    for (target_path, (partition_name, mut layers)) in source {
        let (_, target_layers) = target
            .entry(target_path)
            .or_insert_with(|| (partition_name, Vec::new()));
        target_layers.append(&mut layers);
    }
}

pub(super) fn sorted_ids(ids: HashSet<String>) -> Vec<String> {
    let mut out: Vec<String> = ids.into_iter().collect();
    out.sort();
    out
}

pub(super) fn log_mode_decision(
    module: &Module,
    relative_path: &Path,
    requested_mode: &MountMode,
    effective_mode: &MountMode,
) {
    let relative_display = relative_path.display();
    if requested_mode != effective_mode {
        crate::scoped_log!(
            info,
            "prepare",
            "mode decision: module={}, relative={}, requested={}, effective={}",
            module.id,
            relative_display,
            requested_mode.as_strategy(),
            effective_mode.as_strategy()
        );
    } else {
        crate::scoped_log!(
            debug,
            "prepare",
            "mode decision: module={}, relative={}, requested={}, effective={}",
            module.id,
            relative_display,
            requested_mode.as_strategy(),
            effective_mode.as_strategy()
        );
    }
}
