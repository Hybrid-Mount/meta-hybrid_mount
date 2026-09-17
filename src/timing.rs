// SPDX-License-Identifier: GPL-3.0-only

//! Low-overhead boot phase timing.
//!
//! Design constraints:
//! - Uses only `Instant`, allocates no aggregation structure, and logs one info line per phase.
//! - An explicit `finish()` records `status=ok`; leaving scope early via `?` records
//!   `status=aborted` from `Drop`.
//! - Logs carry only the phase name and duration, never paths, environment variables or tokens.

use std::fmt;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhaseStatus {
    Ok,
    Aborted,
}

impl PhaseStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Aborted => "aborted",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhaseRecord {
    pub label: &'static str,
    pub elapsed: Duration,
    pub status: PhaseStatus,
}

impl PhaseRecord {
    pub fn elapsed_ms(self) -> f64 {
        self.elapsed.as_secs_f64() * 1_000.0
    }
}

impl fmt::Display for PhaseRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "phase={}, status={}, elapsed_ms={:.1}",
            self.label,
            self.status.as_str(),
            self.elapsed_ms()
        )
    }
}

/// RAII phase timer. An explicit `finish()`/`abort()` consumes it;
/// leaving scope unconsumed records abort from `Drop`.
#[derive(Debug)]
pub struct PhaseTimer {
    label: &'static str,
    started: Instant,
    finished: bool,
}

impl PhaseTimer {
    pub fn start(label: &'static str) -> Self {
        log::info!("phase start: phase={label}");
        Self {
            label,
            started: Instant::now(),
            finished: false,
        }
    }

    fn record(&self, status: PhaseStatus) -> PhaseRecord {
        PhaseRecord {
            label: self.label,
            elapsed: self.started.elapsed(),
            status,
        }
    }

    pub fn finish(mut self) -> PhaseRecord {
        self.finished = true;
        let record = self.record(PhaseStatus::Ok);
        log::info!("phase complete: {record}");
        record
    }

    pub fn abort(mut self) -> PhaseRecord {
        self.finished = true;
        let record = self.record(PhaseStatus::Aborted);
        log::warn!("phase aborted: {record}");
        record
    }
}

impl Drop for PhaseTimer {
    fn drop(&mut self) {
        if !self.finished {
            let record = self.record(PhaseStatus::Aborted);
            log::warn!("phase aborted: {record}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_record_formats_stable_machine_readable_fields() {
        let record = PhaseRecord {
            label: "scan",
            elapsed: Duration::from_millis(7),
            status: PhaseStatus::Ok,
        };

        let text = record.to_string();
        assert!(text.contains("phase=scan"));
        assert!(text.contains("status=ok"));
        assert!(text.contains("elapsed_ms=7.0"));
    }

    #[test]
    fn finish_reports_ok_and_abort_reports_aborted() {
        let finished = PhaseTimer::start("plan").finish();
        assert_eq!(finished.label, "plan");
        assert_eq!(finished.status, PhaseStatus::Ok);
        assert!(finished.elapsed >= Duration::ZERO);

        let aborted = PhaseTimer::start("magic").abort();
        assert_eq!(aborted.label, "magic");
        assert_eq!(aborted.status, PhaseStatus::Aborted);
        assert!(aborted.elapsed >= Duration::ZERO);
    }

    #[test]
    fn timer_without_finish_drops_as_aborted() {
        // Drop path only logs; this test pins the type contract that an
        // unconsumed timer is valid and must not panic.
        let _timer = PhaseTimer::start("state");
    }
}
