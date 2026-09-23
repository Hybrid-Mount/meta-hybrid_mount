// SPDX-License-Identifier: GPL-3.0-only

use super::ledger::Ledger;
use super::rules::{self, RuleKernel, SavedRule};
use crate::errors::{Error, Result};
use std::collections::BTreeSet;

pub(crate) trait UidKernel {
    fn uids(&mut self) -> Result<Vec<u32>>;
    fn add_uids(&mut self, uids: &[u32]) -> Result<()>;
    fn remove_uids(&mut self, uids: &[u32]) -> Result<()>;
}

/// Persist intent and apply one module transaction. Successful ownership is
/// committed by the caller together with its updated module snapshot.
pub(crate) fn reconcile(
    kernel: &mut (impl RuleKernel + UidKernel),
    saved: &mut Ledger,
    before: &[SavedRule],
    after: &[SavedRule],
    requested_uids: &[u32],
    mut persist: impl FnMut(&Ledger) -> Result<()>,
) -> Result<()> {
    rules::preflight(kernel, before, after)?;
    let original_uids = kernel.uids()?;
    verify_uids(&original_uids, &saved.isolated_uids)?;
    let introduced_uids = requested_uids
        .iter()
        .copied()
        .filter(|uid| !original_uids.contains(uid))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let original_owned_uids = saved.isolated_uids.clone();
    let expected_uids = original_uids
        .iter()
        .chain(&introduced_uids)
        .copied()
        .collect::<Vec<_>>();
    let mut desired = saved
        .rules
        .iter()
        .filter(|rule| !before.iter().any(|old| old.module_id == rule.module_id))
        .cloned()
        .collect::<Vec<_>>();
    desired.extend_from_slice(after);
    saved.phase = "applying".into();
    saved.pending_rules = desired.clone();
    // In applying/error this field includes mutation intent. Those phases cannot
    // be cleaned up by guessing which operations completed before interruption.
    saved.isolated_uids = original_owned_uids
        .iter()
        .chain(&introduced_uids)
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    saved.error = None;
    persist(saved)?;
    let mut rules_applied = false;
    let applied = (|| {
        // Isolation must be confirmed before publishing global file rules.
        if !introduced_uids.is_empty() {
            kernel.add_uids(&introduced_uids)?;
        }
        verify_uids(&kernel.uids()?, &expected_uids)?;
        rules::reconcile(kernel, before, after)?;
        rules_applied = true;
        verify_uids(&kernel.uids()?, &expected_uids)
    })();
    if let Err(err) = applied {
        if rules_applied {
            // A final isolation readback can fail after the rules committed.
            let _ = rules::reconcile(kernel, after, before);
        }
        let rules_recovered = rules::verify_owned(kernel, &saved.rules).is_ok()
            && kernel.list().is_ok_and(|actual| {
                after
                    .iter()
                    .filter(|new| !before.iter().any(|old| same_key(old, new)))
                    .all(|new| {
                        !actual
                            .iter()
                            .any(|rule| same_key(rule, new) && !rules::derived_parent(rule))
                    })
            });
        // Retain isolation when new rules may remain live. Removing it would
        // expose an incompletely rolled-back injection to the protected UID.
        if rules_recovered {
            for uid in &introduced_uids {
                let _ = kernel.remove_uids(std::slice::from_ref(uid));
            }
        }
        let recovered = rules_recovered
            && kernel.uids().is_ok_and(|actual| {
                original_uids.iter().all(|uid| actual.contains(uid))
                    && introduced_uids.iter().all(|uid| !actual.contains(uid))
            });
        let err = Error::msg(format!(
            "hot VFS transaction failed: {err}; rollback {}",
            if recovered { "clean" } else { "incomplete" }
        ));
        saved.phase = if recovered { "ready" } else { "error" }.into();
        saved.error = Some(err.to_string());
        if recovered {
            saved.pending_rules.clear();
            saved.isolated_uids = original_owned_uids;
        }
        persist(saved)?;
        return Err(err);
    }
    saved.rules = desired;
    saved.pending_rules.clear();
    Ok(())
}

fn verify_uids(actual: &[u32], expected: &[u32]) -> Result<()> {
    if let Some(uid) = expected.iter().find(|uid| !actual.contains(uid)) {
        return Err(Error::msg(format!("VFS isolation UID missing: {uid}")));
    }
    Ok(())
}

fn same_key(left: &SavedRule, right: &SavedRule) -> bool {
    left.uid == right.uid && left.virtual_path == right.virtual_path
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::Error;
    use std::collections::BTreeSet;

    #[derive(Default)]
    struct Kernel {
        rules: Vec<SavedRule>,
        uids: BTreeSet<u32>,
        fail_add_after: Option<usize>,
        ignore_add: bool,
        fail_remove: bool,
        fail_put: bool,
        fail_after_put: bool,
        fail_rule_delete: bool,
        uid_reads: usize,
        fail_uid_reads: BTreeSet<usize>,
        required_before_put: Vec<u32>,
    }

    fn failure() -> Error {
        Error::msg("injected device failure")
    }

    impl UidKernel for Kernel {
        fn uids(&mut self) -> Result<Vec<u32>> {
            let read = self.uid_reads;
            self.uid_reads += 1;
            if self.fail_uid_reads.contains(&read) {
                return Err(failure());
            }
            Ok(self.uids.iter().copied().collect())
        }
        fn add_uids(&mut self, uids: &[u32]) -> Result<()> {
            for (index, uid) in uids.iter().enumerate() {
                if self.fail_add_after == Some(index) {
                    return Err(failure());
                }
                if !self.ignore_add {
                    self.uids.insert(*uid);
                }
            }
            Ok(())
        }
        fn remove_uids(&mut self, uids: &[u32]) -> Result<()> {
            if self.fail_remove {
                return Err(failure());
            }
            for uid in uids {
                self.uids.remove(uid);
            }
            Ok(())
        }
    }

    impl RuleKernel for Kernel {
        fn list(&mut self) -> Result<Vec<SavedRule>> {
            Ok(self.rules.clone())
        }
        fn put(&mut self, rules: &[SavedRule]) -> Result<()> {
            if self.fail_put
                || self
                    .required_before_put
                    .iter()
                    .any(|uid| !self.uids.contains(uid))
            {
                return Err(failure());
            }
            for rule in rules {
                self.rules.retain(|old| !same_key(old, rule));
                self.rules.push(rule.clone());
            }
            if self.fail_after_put {
                return Err(failure());
            }
            Ok(())
        }
        fn delete(&mut self, rules: &[SavedRule]) -> Result<()> {
            if self.fail_rule_delete {
                return Err(failure());
            }
            self.rules
                .retain(|old| !rules.iter().any(|rule| same_key(old, rule)));
            Ok(())
        }
    }

    fn saved() -> Ledger {
        Ledger {
            phase: "ready".into(),
            ..Ledger::default()
        }
    }

    fn rule() -> SavedRule {
        SavedRule {
            module_id: "demo".into(),
            virtual_path: "/system/etc/demo".into(),
            real_path: "/modules/demo/system/etc/demo".into(),
            flags: 0,
            uid: 0,
        }
    }

    #[test]
    fn first_hot_load_isolates_before_rules_and_owns_only_new_uids() {
        let mut kernel = Kernel {
            uids: BTreeSet::from([1000, 2000]),
            required_before_put: vec![1001],
            ..Default::default()
        };
        let mut saved = saved();
        let mut journal = Vec::new();
        reconcile(
            &mut kernel,
            &mut saved,
            &[],
            &[rule()],
            &[1000, 1001, 1001],
            |ledger| {
                journal.push(ledger.clone());
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(kernel.uids, BTreeSet::from([1000, 1001, 2000]));
        assert_eq!(saved.isolated_uids, vec![1001]);
        assert_eq!(journal[0].phase, "applying");
        assert_eq!(journal[0].isolated_uids, vec![1001]);
        assert_eq!(saved.rules, vec![rule()]);
    }

    #[test]
    fn adding_configured_isolation_keeps_existing_owned_uids() {
        let mut kernel = Kernel {
            uids: BTreeSet::from([1000, 2000]),
            ..Default::default()
        };
        let mut saved = saved();
        saved.isolated_uids = vec![1000];
        reconcile(&mut kernel, &mut saved, &[], &[rule()], &[1001], |_| Ok(())).unwrap();
        assert_eq!(saved.isolated_uids, vec![1000, 1001]);
        assert_eq!(kernel.uids, BTreeSet::from([1000, 1001, 2000]));
    }

    #[test]
    fn partial_uid_failure_rolls_back_only_new_isolation() {
        let mut kernel = Kernel {
            uids: BTreeSet::from([1000, 2000]),
            fail_add_after: Some(1),
            ..Default::default()
        };
        let mut saved = saved();
        saved.isolated_uids = vec![1000];
        assert!(
            reconcile(
                &mut kernel,
                &mut saved,
                &[],
                &[rule()],
                &[1001, 1002],
                |_| Ok(())
            )
            .is_err()
        );
        assert_eq!(kernel.uids, BTreeSet::from([1000, 2000]));
        assert!(kernel.rules.is_empty());
        assert_eq!(saved.isolated_uids, vec![1000]);
        assert_eq!(saved.phase, "ready");
        assert!(saved.pending_rules.is_empty());
    }

    #[test]
    fn uid_readback_detects_silently_ignored_addition() {
        let mut kernel = Kernel {
            ignore_add: true,
            ..Default::default()
        };
        let mut saved = saved();
        assert!(reconcile(&mut kernel, &mut saved, &[], &[rule()], &[1000], |_| Ok(())).is_err());
        assert!(kernel.rules.is_empty());
        assert_eq!(saved.phase, "ready");
    }

    #[test]
    fn rule_failure_restores_uid_baseline() {
        let mut kernel = Kernel {
            uids: BTreeSet::from([2000]),
            fail_put: true,
            ..Default::default()
        };
        let mut saved = saved();
        assert!(reconcile(&mut kernel, &mut saved, &[], &[rule()], &[1000], |_| Ok(())).is_err());
        assert_eq!(kernel.uids, BTreeSet::from([2000]));
        assert!(saved.isolated_uids.is_empty());
        assert_eq!(saved.phase, "ready");
    }

    #[test]
    fn failed_uid_rollback_keeps_error_phase_and_intent() {
        let mut kernel = Kernel {
            fail_put: true,
            fail_remove: true,
            ..Default::default()
        };
        let mut saved = saved();
        let mut journal = Vec::new();
        assert!(
            reconcile(&mut kernel, &mut saved, &[], &[rule()], &[1000], |ledger| {
                journal.push(ledger.clone());
                Ok(())
            })
            .is_err()
        );
        assert_eq!(kernel.uids, BTreeSet::from([1000]));
        assert_eq!(saved.phase, "error");
        assert_eq!(saved.isolated_uids, vec![1000]);
        assert_eq!(journal.last().unwrap().phase, "error");
    }

    #[test]
    fn journal_failure_prevents_all_kernel_mutation() {
        let mut kernel = Kernel::default();
        let mut saved = saved();
        assert!(
            reconcile(&mut kernel, &mut saved, &[], &[rule()], &[1000], |_| Err(
                failure()
            ))
            .is_err()
        );
        assert!(kernel.rules.is_empty());
        assert!(kernel.uids.is_empty());
    }

    #[test]
    fn final_uid_readback_failure_rolls_back_rules_and_isolation() {
        let mut kernel = Kernel {
            fail_uid_reads: BTreeSet::from([2]),
            ..Default::default()
        };
        let mut saved = saved();
        assert!(reconcile(&mut kernel, &mut saved, &[], &[rule()], &[1000], |_| Ok(())).is_err());
        assert!(kernel.rules.is_empty());
        assert!(kernel.uids.is_empty());
        assert_eq!(saved.phase, "ready");
        assert!(saved.isolated_uids.is_empty());
    }

    #[test]
    fn unverified_uid_recovery_never_reports_ready() {
        let mut kernel = Kernel {
            fail_put: true,
            fail_uid_reads: BTreeSet::from([2]),
            ..Default::default()
        };
        let mut saved = saved();
        assert!(reconcile(&mut kernel, &mut saved, &[], &[rule()], &[1000], |_| Ok(())).is_err());
        assert_eq!(saved.phase, "error");
        assert_eq!(saved.isolated_uids, vec![1000]);
    }

    #[test]
    fn incomplete_rule_rollback_retains_isolation_for_live_rules() {
        let mut kernel = Kernel {
            fail_after_put: true,
            fail_rule_delete: true,
            ..Default::default()
        };
        let mut saved = saved();
        assert!(reconcile(&mut kernel, &mut saved, &[], &[rule()], &[1000], |_| Ok(())).is_err());
        assert_eq!(kernel.rules, vec![rule()]);
        assert_eq!(kernel.uids, BTreeSet::from([1000]));
        assert_eq!(saved.phase, "error");
        assert_eq!(saved.isolated_uids, vec![1000]);
    }

    #[test]
    fn missing_owned_isolation_blocks_mutation_before_journaling() {
        let mut kernel = Kernel::default();
        let mut saved = saved();
        saved.isolated_uids = vec![1000];
        let mut journal = Vec::new();
        assert!(
            reconcile(&mut kernel, &mut saved, &[], &[rule()], &[1001], |ledger| {
                journal.push(ledger.clone());
                Ok(())
            })
            .is_err()
        );
        assert!(journal.is_empty());
        assert!(kernel.uids.is_empty());
        assert!(kernel.rules.is_empty());
    }
}
