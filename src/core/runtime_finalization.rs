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
