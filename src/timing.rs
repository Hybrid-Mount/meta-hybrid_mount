// SPDX-License-Identifier: GPL-3.0-only

//! 低开销启动阶段计时。
//!
//! 设计约束：
//! - 只使用 `Instant`，不分配聚合结构，每阶段一条 info 日志；
//! - 显式 `finish()` 记录 `status=ok`，因 `?` 提前离开作用域时
//!   `Drop` 记录 `status=aborted`；
//! - 日志只包含阶段名与耗时，不携带路径、环境变量或令牌等敏感内容。

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

/// RAII 阶段计时器。显式 `finish()`/`abort()` 会消费自身；
/// 未消费即离开作用域时由 `Drop` 记录 aborted。
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
