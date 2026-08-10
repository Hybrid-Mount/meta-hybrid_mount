// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::time::{Duration, Instant};

/// Measures one coarse-grained stage and emits a structured latency record.
///
/// The timer deliberately keeps only an `Instant` and static labels. Callers
/// should time stages rather than individual files or syscalls so normal boot
/// logging stays inexpensive and readable.
#[must_use = "a stage timer must be finished or kept alive for the measured stage"]
pub struct StageTimer {
    scope: &'static str,
    stage: &'static str,
    started: Instant,
    finished: bool,
}

impl StageTimer {
    #[inline]
    pub fn start(scope: &'static str, stage: &'static str) -> Self {
        Self {
            scope,
            stage,
            started: Instant::now(),
            finished: false,
        }
    }

    #[inline]
    pub fn finish(mut self) -> Duration {
        let elapsed = self.started.elapsed();
        self.finished = true;
        log_latency(self.scope, self.stage, "ok", elapsed);
        elapsed
    }
}

impl Drop for StageTimer {
    fn drop(&mut self) {
        if !self.finished {
            log_latency(self.scope, self.stage, "aborted", self.started.elapsed());
        }
    }
}

#[inline]
fn log_latency(scope: &str, stage: &str, status: &str, elapsed: Duration) {
    crate::scoped_log!(
        info,
        "latency",
        "scope={}, stage={}, status={}, elapsed_us={}",
        scope,
        stage,
        status,
        elapsed.as_micros()
    );
}
