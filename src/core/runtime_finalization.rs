// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::path::Path;

use anyhow::Result;

use crate::{
    conf::config::Config,
    core::{
        inventory::InventorySummary, ops::executor::ExecutionResult, runtime_state::RuntimeState,
        storage::StorageMode,
    },
};

pub fn build_state(
    config: &Config,
    storage_mode: StorageMode,
    mount_point: &Path,
    result: &ExecutionResult,
    inventory: &InventorySummary,
) -> Result<RuntimeState> {
    crate::scoped_log!(
        info,
        "runtime_finalization",
        "start: storage_mode={}, mount_point={}, overlay_modules={}, magic_modules={}",
        storage_mode.as_str(),
        mount_point.display(),
        result.overlay_module_ids.len(),
        result.magic_module_ids.len()
    );

    let build_timer = crate::utils::StageTimer::start("runtime_finalization", "state_build");
    let state =
        RuntimeState::build_from_execution(config, storage_mode, mount_point, result, inventory)?;
    build_timer.finish();

    Ok(state)
}

pub fn save_state(state: &RuntimeState) -> Result<()> {
    let save_timer = crate::utils::StageTimer::start("runtime_finalization", "state_save");
    state.save()?;
    save_timer.finish();

    crate::scoped_log!(
        info,
        "runtime_finalization",
        "complete: active_mounts={}, skip_mount_modules={}",
        state.active_mounts.len(),
        state.skip_mount_modules.len()
    );

    Ok(())
}
