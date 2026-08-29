// SPDX-License-Identifier: GPL-3.0-only

//! Generic cleanup journal for mount pipeline side effects.

use crate::errors::{Error, Result};

#[derive(Debug)]
pub struct CleanupFailure {
    pub label: String,
    pub error: Error,
}

#[derive(Debug, Default)]
pub struct RollbackReport {
    pub cleaned: usize,
    pub retained: usize,
    pub failures: Vec<CleanupFailure>,
}

struct CleanupAction<'a> {
    label: String,
    retainable: bool,
    cleanup: Box<dyn FnOnce() -> Result<()> + 'a>,
}

#[derive(Default)]
pub struct MountTransaction<'a> {
    actions: Vec<CleanupAction<'a>>,
    finished: bool,
}

impl<'a> MountTransaction<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<F>(&mut self, label: impl Into<String>, cleanup: F)
    where
        F: FnOnce() -> Result<()> + 'a,
    {
        self.actions.push(CleanupAction {
            label: label.into(),
            retainable: false,
            cleanup: Box::new(cleanup),
        });
    }

    /// Register cleanup that `commit(true)` may retain; `rollback` always runs it.
    #[allow(dead_code)]
    pub fn register_retainable<F>(&mut self, label: impl Into<String>, cleanup: F)
    where
        F: FnOnce() -> Result<()> + 'a,
    {
        self.actions.push(CleanupAction {
            label: label.into(),
            retainable: true,
            cleanup: Box::new(cleanup),
        });
    }

    /// Discard every cleanup while keeping the side effects.
    #[allow(dead_code)]
    pub fn disarm(mut self) {
        self.finished = true;
    }

    /// Run all cleanups in reverse order; `retain_resources` skips only
    /// explicitly retainable actions.
    #[allow(dead_code)]
    pub fn commit(mut self, retain_resources: bool) -> Result<()> {
        let report = self.run_cleanup(true, retain_resources);
        self.finished = true;
        if report.failures.is_empty() {
            Ok(())
        } else {
            let summary = report
                .failures
                .iter()
                .map(|failure| format!("{}: {}", failure.label, failure.error))
                .collect::<Vec<_>>()
                .join("; ");
            Err(Error::msg(format!(
                "mount transaction commit failed: {summary}"
            )))
        }
    }

    /// Run every cleanup and report each failure.
    pub fn rollback(mut self) -> RollbackReport {
        let report = self.run_cleanup(false, false);
        self.finished = true;
        report
    }

    fn run_cleanup(&mut self, commit: bool, retain_resources: bool) -> RollbackReport {
        let mut report = RollbackReport::default();

        for action in self.actions.drain(..).rev() {
            if commit && retain_resources && action.retainable {
                report.retained += 1;
                log::info!(
                    "mount transaction resource retained: label={}, reason=disable_umount",
                    action.label
                );
                continue;
            }

            let cleanup = action.cleanup;
            match cleanup() {
                Ok(()) => report.cleaned += 1,
                Err(error) => {
                    log::error!(
                        "mount transaction cleanup failed: label={}, error={error}",
                        action.label
                    );
                    report.failures.push(CleanupFailure {
                        label: action.label,
                        error,
                    });
                }
            }
        }

        report
    }
}

impl Drop for MountTransaction<'_> {
    fn drop(&mut self) {
        if self.finished {
            return;
        }

        let report = self.run_cleanup(false, false);
        self.finished = true;
        if !report.failures.is_empty() {
            log::error!(
                "mount transaction dropped without explicit commit: cleaned={}, retained={}, failures={}",
                report.cleaned,
                report.retained,
                report.failures.len()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{MountTransaction, RollbackReport};
    use crate::errors::{Error, Result};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    type Calls = Arc<Mutex<Vec<&'static str>>>;

    fn record(
        order: &Calls,
        label: &'static str,
        result: Result<()>,
    ) -> impl FnOnce() -> Result<()> {
        let order = Arc::clone(order);
        move || {
            order.lock().unwrap().push(label);
            result
        }
    }

    #[test]
    fn commit_runs_cleanup_in_reverse_registration_order() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut transaction = MountTransaction::new();
        transaction.register("first", record(&calls, "first", Ok(())));
        transaction.register("second", record(&calls, "second", Ok(())));
        transaction.register("third", record(&calls, "third", Ok(())));

        transaction.commit(false).unwrap();

        assert_eq!(*calls.lock().unwrap(), vec!["third", "second", "first"]);
    }

    #[test]
    fn rollback_report_collects_every_failure() {
        let mut transaction = MountTransaction::new();
        transaction.register("first", || Err(Error::msg("first cleanup failed")));
        transaction.register("second", || Err(Error::msg("second cleanup failed")));
        transaction.register("third", || Ok(()));

        let report: RollbackReport = transaction.rollback();

        assert_eq!(report.cleaned, 1);
        assert_eq!(report.retained, 0);
        assert_eq!(report.failures.len(), 2);
        assert_eq!(report.failures[0].label, "second");
        assert_eq!(report.failures[1].label, "first");
        assert!(
            report.failures[0]
                .error
                .to_string()
                .contains("second cleanup failed")
        );
        assert!(
            report.failures[1]
                .error
                .to_string()
                .contains("first cleanup failed")
        );
    }

    #[test]
    fn disarm_skips_all_cleanup_actions() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut transaction = MountTransaction::new();
        transaction.register("first", record(&calls, "first", Ok(())));
        transaction.register("second", record(&calls, "second", Ok(())));

        transaction.disarm();

        assert!(calls.lock().unwrap().is_empty());
    }

    #[test]
    fn commit_retains_only_explicitly_retainable_actions() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut transaction = MountTransaction::new();
        transaction.register_retainable("storage", record(&calls, "storage", Ok(())));
        transaction.register("magic_staging", record(&calls, "magic_staging", Ok(())));

        transaction.commit(true).unwrap();

        assert_eq!(*calls.lock().unwrap(), vec!["magic_staging"]);
    }

    #[test]
    fn commit_without_retain_runs_retainable_actions_too() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut transaction = MountTransaction::new();
        transaction.register_retainable("storage", record(&calls, "storage", Ok(())));

        transaction.commit(false).unwrap();

        assert_eq!(*calls.lock().unwrap(), vec!["storage"]);
    }

    #[test]
    fn commit_failure_reports_every_failed_action() {
        let mut transaction = MountTransaction::new();
        transaction.register("first", || Err(Error::msg("first cleanup failed")));
        transaction.register("second", || Err(Error::msg("second cleanup failed")));

        let message = transaction.commit(false).unwrap_err().to_string();

        assert!(message.contains("first: first cleanup failed"), "{message}");
        assert!(
            message.contains("second: second cleanup failed"),
            "{message}"
        );
    }

    #[test]
    fn rollback_runs_retainable_actions() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut transaction = MountTransaction::new();
        transaction.register_retainable("storage", record(&calls, "storage", Ok(())));

        let report = transaction.rollback();

        assert_eq!(*calls.lock().unwrap(), vec!["storage"]);
        assert_eq!(report.cleaned, 1);
        assert_eq!(report.retained, 0);
    }

    #[test]
    fn drop_after_rollback_does_not_rerun_cleanup() {
        let count = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&count);
        let mut transaction = MountTransaction::new();
        transaction.register("finished", move || {
            counter.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });

        let report = transaction.rollback();
        assert!(report.failures.is_empty());

        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn drop_after_commit_does_not_rerun_cleanup() {
        let count = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&count);
        let mut transaction = MountTransaction::new();
        transaction.register("finished", move || {
            counter.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });

        transaction.commit(false).unwrap();

        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn drop_runs_unfinished_cleanup_exactly_once() {
        let count = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&count);
        let mut transaction = MountTransaction::new();
        transaction.register("last_defense", move || {
            counter.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });

        drop(transaction);

        assert_eq!(count.load(Ordering::SeqCst), 1);
    }
}
