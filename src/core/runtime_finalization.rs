// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::path::Path;

use anyhow::Result;

use crate::{
    conf::config::Config,
    core::{
        module_status, ops::executor::ExecutionResult, runtime_state::RuntimeState,
        storage::StorageMode,
    },
};

pub fn finalize(
    config: &Config,
    storage_mode: StorageMode,
    mount_point: &Path,
    result: &ExecutionResult,
) -> Result<()> {
    crate::scoped_log!(
        info,
        "runtime_finalization",
        "start: storage_mode={}, mount_point={}, overlay_modules={}, magic_modules={}, kasumi_modules={}",
        storage_mode.as_str(),
        mount_point.display(),
        result.overlay_module_ids.len(),
        result.magic_module_ids.len(),
        result.kasumi_count()
    );

    let blacklisted_count = config
        .module_blacklist
        .iter()
        .filter(|id| config.moduledir.join(id).is_dir())
        .count();

    module_status::update_description(
        storage_mode,
        config.kasumi.enabled,
        result.overlay_module_ids.len(),
        result.magic_module_ids.len(),
        result.kasumi_count(),
        blacklisted_count,
    );

    let state = RuntimeState::build_from_execution(config, storage_mode, mount_point, result);
    if let Err(err) = state.save() {
        crate::scoped_log!(
            warn,
            "runtime_finalization",
            "save runtime state failed: {:#}",
            err
        );
    }

    crate::scoped_log!(
        info,
        "runtime_finalization",
        "complete: active_mounts={}, mount_errors={}, skip_mount_modules={}",
        state.active_mounts.len(),
        state.mount_error_modules.len(),
        state.skip_mount_modules.len()
    );

    Ok(())
}
