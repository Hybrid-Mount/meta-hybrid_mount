// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

mod retry_state;
mod skip_markers;

use anyhow::{Context, Result};

use self::retry_state::{RecoveryDecision, RecoveryState};
use crate::{
    conf::config::Config,
    core::{MountController, recovery::ModuleStageFailure},
    sys, utils,
};

pub fn run(config: Config) -> Result<()> {
    let mut state = RecoveryState::new(&config)?;

    loop {
        let attempt = state.current_attempt();
        let mnt_base = utils::get_mnt();
        sys::fs::ensure_dir_exists(&mnt_base)?;
        crate::scoped_log!(
            info,
            "recovery",
            "attempt start: attempt={}/{}, mount_base={}",
            attempt,
            state.max_restarts(),
            mnt_base.display()
        );

        let daemon_result = (|| -> Result<()> {
            MountController::new(config.clone(), &mnt_base)
                .init_storage(&mnt_base)
                .context("Failed to initialize storage")?
                .scan_and_sync()
                .context("Failed to scan and sync modules")?
                .generate_plan()
                .context("Failed to generate mount plan")?
                .execute()
                .context("Failed to execute mount plan")?
                .finalize()
                .context("Failed to finalize boot sequence")?;
            Ok(())
        })();

        match daemon_result {
            Ok(()) => {
                state.log_completion();
                return Ok(());
            }
            Err(e) => {
                if let Some(module_failure) = e.downcast_ref::<ModuleStageFailure>() {
                    if module_failure.module_ids.is_empty() {
                        match state.handle_unattributed_failure(module_failure.stage.to_string()) {
                            RecoveryDecision::RetryUnattributed => continue,
                            RecoveryDecision::AbortRetryLimit => {
                                return state.abort_on_retry_limit();
                            }
                            RecoveryDecision::InspectModules => {}
                        }
                    } else {
                        crate::scoped_log!(
                            warn,
                            "recovery",
                            "module failure: stage={}, modules={}",
                            module_failure.stage,
                            module_failure.module_ids.join(", ")
                        );
                    }

                    let action = state.mark_failed_modules(
                        &module_failure.stage.to_string(),
                        Some(&module_failure.source.to_string()),
                        &module_failure.module_ids,
                    )?;

                    if !action.already_marked.is_empty() {
                        crate::scoped_log!(
                            debug,
                            "recovery",
                            "already marked: modules={}",
                            action.already_marked.join(", ")
                        );
                    }
                    if !action.unknown_modules.is_empty() {
                        crate::scoped_log!(
                            error,
                            "recovery",
                            "unknown modules: stage={}, attempt={}/{}, modules={}",
                            module_failure.stage,
                            attempt,
                            state.max_restarts(),
                            action.unknown_modules.join(",")
                        );
                    }

                    if !action.newly_marked.is_empty() {
                        crate::scoped_log!(
                            warn,
                            "recovery",
                            "mark skip: stage={}, attempt={}/{}, modules={}",
                            module_failure.stage,
                            attempt,
                            state.max_restarts(),
                            action.newly_marked.join(",")
                        );

                        match state.handle_newly_marked_modules(module_failure.stage.to_string()) {
                            RecoveryDecision::RetryUnattributed => continue,
                            RecoveryDecision::AbortRetryLimit => {
                                return state.abort_on_retry_limit();
                            }
                            RecoveryDecision::InspectModules => continue,
                        }
                    }

                    crate::scoped_log!(
                        error,
                        "recovery",
                        "abort: stage={}, reason=no_newly_marked_modules",
                        module_failure.stage
                    );
                }

                let err_msg = format!("{:#}", e).replace('\n', " -> ");
                crate::scoped_log!(error, "recovery", "unrecoverable: error={}", err_msg);
                crate::core::module_status::update_crash_description(&err_msg);
                return Err(e);
            }
        }
    }
}
