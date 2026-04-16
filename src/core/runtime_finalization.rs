// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::Path;

use anyhow::Result;

use crate::{
    conf::config::Config,
    core::{
        module_status,
        ops::{executor::ExecutionResult, planner::MountPlan},
        runtime_state::RuntimeState,
    },
    defs,
    mount::hymofs,
};

pub fn finalize(
    config: &Config,
    storage_mode: &str,
    mount_point: &Path,
    plan: &MountPlan,
    result: &ExecutionResult,
) -> Result<()> {
    module_status::update_description(
        storage_mode,
        result.overlay_module_ids.len(),
        result.magic_module_ids.len(),
        result.hymofs_module_ids.len(),
    );

    let state = build_runtime_state(config, storage_mode, mount_point, plan, result);
    if let Err(err) = state.save() {
        crate::scoped_log!(warn, "finalize", "save runtime state failed: {:#}", err);
    }

    Ok(())
}

fn build_runtime_state(
    config: &Config,
    storage_mode: &str,
    mount_point: &Path,
    plan: &MountPlan,
    result: &ExecutionResult,
) -> RuntimeState {
    let previous_state = RuntimeState::load().unwrap_or_default();
    let hymofs = hymofs::collect_runtime_info(config);
    let mut state = RuntimeState::new(
        storage_mode.to_string(),
        mount_point.to_path_buf(),
        result.overlay_module_ids.clone(),
        result.magic_module_ids.clone(),
        result.hymofs_module_ids.clone(),
        collect_active_mounts(plan),
        result.mount_stats.clone(),
        hymofs,
        defs::DAEMON_LOG_FILE.into(),
    );
    state.mount_error_modules = previous_state.mount_error_modules;
    state
}

fn collect_active_mounts(plan: &MountPlan) -> Vec<String> {
    let mut active_mounts: Vec<String> = plan
        .overlay_ops
        .iter()
        .map(|op| op.partition_name.clone())
        .collect();

    if !plan.hymofs_module_ids.is_empty() {
        active_mounts.push("hymofs".to_string());
    }

    active_mounts.sort();
    active_mounts.dedup();
    active_mounts
}
