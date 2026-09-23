// SPDX-License-Identifier: GPL-3.0-only

use serde::{Deserialize, Serialize};

use crate::errors::{Error, Result};
use crate::vfs::protocol::FLAG_VIRTUAL_DIR;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SavedRule {
    pub module_id: String,
    pub virtual_path: String,
    pub real_path: String,
    pub flags: u32,
    pub uid: u32,
}

pub(crate) trait RuleKernel {
    fn list(&mut self) -> Result<Vec<SavedRule>>;
    fn put(&mut self, rules: &[SavedRule]) -> Result<()>;
    fn delete(&mut self, rules: &[SavedRule]) -> Result<()>;
}

// The kernel adds these bits after resolving directory topology; they do not
// describe an owned operation. Keep all other bits, including unknown ones.
const DERIVED_FLAGS: u32 = 1 | FLAG_VIRTUAL_DIR;

pub(crate) fn semantic_equal(expected: &SavedRule, actual: &SavedRule) -> bool {
    same_key(expected, actual)
        && expected.real_path == actual.real_path
        && expected.flags & !DERIVED_FLAGS == actual.flags & !DERIVED_FLAGS
}

fn same_key(left: &SavedRule, right: &SavedRule) -> bool {
    left.uid == right.uid && left.virtual_path == right.virtual_path
}

pub(crate) fn derived_parent(rule: &SavedRule) -> bool {
    rule.flags & FLAG_VIRTUAL_DIR != 0
        && rule.flags & !DERIVED_FLAGS == 0
        && rule.real_path.is_empty()
}

fn ancestor(parent: &str, child: &str) -> bool {
    parent != child
        && child
            .strip_prefix(parent)
            .is_some_and(|suffix| parent == "/" || suffix.starts_with('/'))
}

fn transaction_error(detail: impl Into<String>) -> Error {
    Error::VfsProtocol {
        detail: detail.into(),
    }
}

fn verify_present(current: &[SavedRule], expected: &[SavedRule]) -> Result<()> {
    for rule in expected {
        if !current.iter().any(|actual| semantic_equal(rule, actual)) {
            return Err(transaction_error(format!(
                "owned rule missing or changed: {} (uid {})",
                rule.virtual_path, rule.uid
            )));
        }
    }
    Ok(())
}

pub(crate) fn verify_owned(kernel: &mut impl RuleKernel, before: &[SavedRule]) -> Result<()> {
    verify_present(&kernel.list()?, before)
}

fn validate_unique(rules: &[SavedRule]) -> Result<()> {
    let mut keys = std::collections::HashSet::new();
    for rule in rules {
        if !keys.insert((rule.virtual_path.as_str(), rule.uid)) {
            return Err(transaction_error(format!(
                "duplicate owned rule: {} (uid {})",
                rule.virtual_path, rule.uid
            )));
        }
    }
    Ok(())
}

fn verify_result(
    current: &[SavedRule],
    expected: &[SavedRule],
    obsolete: &[SavedRule],
) -> Result<()> {
    verify_present(current, expected)?;
    for old in obsolete {
        if !expected.iter().any(|rule| same_key(rule, old))
            && current
                .iter()
                .any(|rule| same_key(rule, old) && !derived_parent(rule))
        {
            return Err(transaction_error(format!(
                "obsolete rule remains: {} (uid {})",
                old.virtual_path, old.uid
            )));
        }
    }
    Ok(())
}

fn ordered(rules: impl Iterator<Item = SavedRule>, deepest_first: bool) -> Vec<SavedRule> {
    let mut rules: Vec<_> = rules.collect();
    rules.sort_by(|a, b| {
        let a_depth = a.virtual_path.bytes().filter(|byte| *byte == b'/').count();
        let b_depth = b.virtual_path.bytes().filter(|byte| *byte == b'/').count();
        let depth = if deepest_first {
            b_depth.cmp(&a_depth)
        } else {
            a_depth.cmp(&b_depth)
        };
        depth
            .then_with(|| a.virtual_path.cmp(&b.virtual_path))
            .then(a.uid.cmp(&b.uid))
    });
    rules
}

/// Validate ownership and target conflicts before recording a mutation intent.
/// Reconciliation repeats this check because external writers can change state.
pub(crate) fn preflight(
    kernel: &mut impl RuleKernel,
    before: &[SavedRule],
    after: &[SavedRule],
) -> Result<()> {
    validate_snapshot(&kernel.list()?, before, after).map(|_| ())
}

fn validate_snapshot(
    initial: &[SavedRule],
    before: &[SavedRule],
    after: &[SavedRule],
) -> Result<Vec<SavedRule>> {
    validate_unique(before)?;
    validate_unique(after)?;
    verify_present(initial, before)?;
    let foreign: Vec<_> = initial
        .iter()
        .filter(|actual| !before.iter().any(|owned| same_key(owned, actual)))
        .cloned()
        .collect();
    for actual in &foreign {
        for owned in before.iter().chain(after) {
            if actual.uid != owned.uid {
                continue;
            }
            if derived_parent(actual) && ancestor(&actual.virtual_path, &owned.virtual_path) {
                continue;
            }
            if same_key(actual, owned)
                || ancestor(&actual.virtual_path, &owned.virtual_path)
                || ancestor(&owned.virtual_path, &actual.virtual_path)
            {
                return Err(transaction_error(format!(
                    "foreign rule conflicts with {}: {} (uid {})",
                    owned.virtual_path, actual.virtual_path, actual.uid
                )));
            }
        }
    }
    Ok(foreign)
}

/// Reconcile rules owned by the caller without clearing unrelated kernel state.
///
/// The caller must serialize its writers. The wire ABI cannot atomically publish
/// a batch or exclude external writers between a readback and a mutation. Recovery
/// verifies rule payloads, but cannot recover a deleted source's pinned inode.
pub(crate) fn reconcile(
    kernel: &mut impl RuleKernel,
    before: &[SavedRule],
    after: &[SavedRule],
) -> Result<()> {
    let foreign = validate_snapshot(&kernel.list()?, before, after)?;
    let obsolete = ordered(
        before
            .iter()
            .filter(|old| !after.iter().any(|new| same_key(old, new)))
            .cloned(),
        true,
    );
    let desired = ordered(after.iter().cloned(), false);
    let mut put_attempted = false;
    let apply = (|| {
        if !obsolete.is_empty() {
            kernel.delete(&obsolete)?;
        }
        // Reinsert unchanged payloads too: source path identity can stay the same
        // even though an update replaced its inode.
        if !desired.is_empty() {
            put_attempted = true;
            kernel.put(&desired)?;
        }
        let current = kernel.list()?;
        verify_result(&current, after, before)?;
        let preserved: Vec<_> = foreign
            .iter()
            .filter(|rule| !derived_parent(rule))
            .cloned()
            .collect();
        verify_present(&current, &preserved)
    })();
    if let Err(cause) = apply {
        let attempted = if put_attempted { after } else { &[] };
        let recovery = rollback(kernel, before, attempted, &foreign);
        return Err(transaction_error(match recovery {
            Ok(()) => format!("owned rule transaction failed: {cause}; rollback clean"),
            Err(recovery) => {
                format!("owned rule transaction failed: {cause}; rollback incomplete: {recovery}")
            }
        }));
    }
    Ok(())
}

fn rollback(
    kernel: &mut impl RuleKernel,
    before: &[SavedRule],
    attempted: &[SavedRule],
    foreign: &[SavedRule],
) -> Result<()> {
    let current = kernel.list()?;
    let introduced = ordered(
        attempted
            .iter()
            .filter(|new| {
                !before.iter().any(|old| same_key(old, new))
                    && current.iter().any(|actual| semantic_equal(new, actual))
            })
            .cloned(),
        true,
    );
    // Recovery is best effort per record: one missing source must not prevent
    // restoring unrelated records later in the batch.
    for rule in &introduced {
        let _ = kernel.delete(std::slice::from_ref(rule));
    }
    let current = kernel.list()?;
    let restore = ordered(
        before
            .iter()
            .filter(|old| {
                !current.iter().any(|actual| semantic_equal(old, actual))
                    && current
                        .iter()
                        .filter(|actual| same_key(old, actual))
                        .all(|actual| {
                            derived_parent(actual)
                                || attempted.iter().any(|new| semantic_equal(new, actual))
                        })
            })
            .cloned(),
        false,
    );
    for rule in &restore {
        let _ = kernel.put(std::slice::from_ref(rule));
    }
    // Report recovery according to actual state, even when a syscall returned
    // an error after completing its mutation.
    let current = kernel.list()?;
    verify_result(&current, before, attempted)?;
    let preserved: Vec<_> = foreign
        .iter()
        .filter(|rule| !derived_parent(rule))
        .cloned()
        .collect();
    verify_present(&current, &preserved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vfs::protocol::{FLAG_OPAQUE, FLAG_WHITEOUT};
    use std::collections::VecDeque;

    #[derive(Default)]
    struct Kernel {
        rules: Vec<SavedRule>,
        puts: VecDeque<Option<usize>>,
        deletes: VecDeque<Option<usize>>,
        missing_sources: Vec<String>,
        mutations: Vec<(char, String)>,
        ignore_put: bool,
        external_on_put_failure: Option<SavedRule>,
    }

    fn failure() -> Error {
        Error::VfsProtocol {
            detail: "injected kernel failure".into(),
        }
    }

    impl RuleKernel for Kernel {
        fn list(&mut self) -> Result<Vec<SavedRule>> {
            Ok(self.rules.clone())
        }
        fn put(&mut self, rules: &[SavedRule]) -> Result<()> {
            let fail_after = self.puts.pop_front().flatten();
            for (index, rule) in rules.iter().enumerate() {
                if fail_after == Some(index) || self.missing_sources.contains(&rule.real_path) {
                    if let Some(external) = self.external_on_put_failure.take() {
                        self.rules.retain(|old| {
                            old.virtual_path != external.virtual_path || old.uid != external.uid
                        });
                        self.rules.push(external);
                    }
                    return Err(failure());
                }
                self.mutations.push(('+', rule.virtual_path.clone()));
                if !self.ignore_put {
                    self.rules
                        .retain(|old| old.virtual_path != rule.virtual_path || old.uid != rule.uid);
                    self.rules.push(rule.clone());
                }
            }
            if fail_after == Some(rules.len()) {
                return Err(failure());
            }
            Ok(())
        }
        fn delete(&mut self, rules: &[SavedRule]) -> Result<()> {
            let fail_after = self.deletes.pop_front().flatten();
            for (index, rule) in rules.iter().enumerate() {
                if fail_after == Some(index) {
                    return Err(failure());
                }
                self.mutations.push(('-', rule.virtual_path.clone()));
                self.rules
                    .retain(|old| old.virtual_path != rule.virtual_path || old.uid != rule.uid);
            }
            if fail_after == Some(rules.len()) {
                return Err(failure());
            }
            Ok(())
        }
    }

    fn rule(path: &str, real: &str) -> SavedRule {
        SavedRule {
            module_id: "module".into(),
            virtual_path: path.into(),
            real_path: real.into(),
            flags: 0,
            uid: 0,
        }
    }

    #[test]
    fn replaces_owned_rules_and_preserves_foreign_records() {
        let old = rule("/system/old", "/source/old");
        let new = rule("/system/new", "/source/new");
        let foreign = rule("/vendor/foreign", "/foreign");
        let mut kernel = Kernel {
            rules: vec![old.clone(), foreign.clone()],
            ..Default::default()
        };
        assert!(reconcile(&mut kernel, &[old], std::slice::from_ref(&new)).is_ok());
        assert_eq!(kernel.rules, vec![foreign, new]);
    }

    #[test]
    fn refreshes_identical_rules_to_repin_replaced_sources() {
        let before = vec![rule("/system/a", "/source/a")];
        let mut kernel = Kernel {
            rules: before.clone(),
            ..Default::default()
        };
        assert!(reconcile(&mut kernel, &before, &before).is_ok());
        assert_eq!(kernel.mutations, vec![('+', "/system/a".into())]);
    }

    #[test]
    fn deletes_children_first_and_adds_parents_first() {
        let before = vec![rule("/old", "/source"), rule("/old/child", "/child")];
        let after = vec![rule("/new/child", "/child"), rule("/new", "/source")];
        let mut kernel = Kernel {
            rules: before.clone(),
            ..Default::default()
        };
        assert!(reconcile(&mut kernel, &before, &after).is_ok());
        assert_eq!(
            kernel.mutations,
            vec![
                ('-', "/old/child".into()),
                ('-', "/old".into()),
                ('+', "/new".into()),
                ('+', "/new/child".into())
            ]
        );
    }

    #[test]
    fn restores_before_after_a_partial_put_batch() {
        let before = vec![rule("/system/a", "/old/a")];
        let foreign = rule("/vendor/x", "/foreign/x");
        let after = vec![rule("/system/a", "/new/a"), rule("/system/b", "/new/b")];
        let mut kernel = Kernel {
            rules: vec![before[0].clone(), foreign.clone()],
            puts: VecDeque::from([Some(1)]),
            ..Default::default()
        };
        let error = reconcile(&mut kernel, &before, &after).expect_err("partial batch must fail");
        assert!(error.to_string().contains("rollback clean"), "{error}");
        assert!(kernel.rules.contains(&before[0]));
        assert!(kernel.rules.contains(&foreign));
        assert_eq!(kernel.rules.len(), 2);
    }

    #[test]
    fn removes_only_newly_introduced_rules_during_rollback() {
        let before = vec![rule("/system/old", "/old")];
        let after = vec![rule("/system/a", "/a"), rule("/system/b", "/b")];
        let mut kernel = Kernel {
            rules: before.clone(),
            puts: VecDeque::from([Some(1)]),
            ..Default::default()
        };
        assert!(reconcile(&mut kernel, &before, &after).is_err());
        assert_eq!(kernel.rules, before);
    }

    #[test]
    fn recovers_a_partially_deleted_batch() {
        let before = vec![rule("/system/a", "/a"), rule("/system/b", "/b")];
        let mut kernel = Kernel {
            rules: before.clone(),
            deletes: VecDeque::from([Some(1)]),
            ..Default::default()
        };
        let error = reconcile(&mut kernel, &before, &[]).expect_err("delete should fail");
        assert!(error.to_string().contains("rollback clean"), "{error}");
        assert!(before.iter().all(|r| kernel.rules.contains(r)));
    }

    #[test]
    fn refuses_foreign_equal_ancestor_and_descendant_paths_without_mutation() {
        for foreign in ["/system/a", "/system", "/system/a/child"] {
            let mut kernel = Kernel {
                rules: vec![rule(foreign, "/foreign")],
                ..Default::default()
            };
            assert!(
                reconcile(&mut kernel, &[], &[rule("/system/a", "/a")]).is_err(),
                "foreign={foreign}"
            );
            assert!(kernel.mutations.is_empty());
        }
    }

    #[test]
    fn rejects_missing_or_drifted_owned_rules_without_mutation() {
        let before = vec![rule("/system/a", "/a")];
        for current in [vec![], vec![rule("/system/a", "/foreign")]] {
            let mut kernel = Kernel {
                rules: current,
                ..Default::default()
            };
            assert!(reconcile(&mut kernel, &before, &[]).is_err());
            assert!(kernel.mutations.is_empty());
        }
    }

    #[test]
    fn unloads_saved_rules_after_source_disappears() {
        let before = vec![rule("/system/a", "/deleted")];
        let mut kernel = Kernel {
            rules: before.clone(),
            missing_sources: vec!["/deleted".into()],
            ..Default::default()
        };
        assert!(reconcile(&mut kernel, &before, &[]).is_ok());
        assert!(kernel.rules.is_empty());
    }

    #[test]
    fn reports_incomplete_recovery_when_old_source_cannot_be_restored() {
        let before = vec![rule("/system/old", "/deleted")];
        let mut kernel = Kernel {
            rules: before.clone(),
            missing_sources: vec!["/deleted".into()],
            puts: VecDeque::from([Some(0)]),
            ..Default::default()
        };
        let error = reconcile(&mut kernel, &before, &[rule("/system/new", "/new")])
            .expect_err("apply must fail");
        assert!(error.to_string().contains("rollback incomplete"), "{error}");
    }

    #[test]
    fn matches_semantic_whiteout_and_opaque_flags_and_ignores_owner_labels() {
        for flags in [FLAG_WHITEOUT, FLAG_OPAQUE] {
            let mut saved = rule("/system/a", "");
            saved.flags = flags;
            let mut actual = saved.clone();
            actual.module_id.clear();
            actual.flags |= 1 | FLAG_VIRTUAL_DIR;
            let mut kernel = Kernel {
                rules: vec![actual],
                ..Default::default()
            };
            assert!(reconcile(&mut kernel, &[saved], &[]).is_ok());
            assert!(kernel.rules.is_empty());
        }
    }

    #[test]
    fn allows_derived_parent_but_rejects_foreign_opaque_parent() {
        for flags in [1 | FLAG_VIRTUAL_DIR, 1 | FLAG_VIRTUAL_DIR | FLAG_OPAQUE] {
            let mut parent = rule("/system", "");
            parent.flags = flags;
            let mut kernel = Kernel {
                rules: vec![parent],
                ..Default::default()
            };
            let result = reconcile(&mut kernel, &[], &[rule("/system/a", "/a")]);
            assert_eq!(result.is_ok(), flags & FLAG_OPAQUE == 0);
        }
    }

    #[test]
    fn readback_detects_silent_apply_failure() {
        let mut kernel = Kernel {
            ignore_put: true,
            ..Default::default()
        };
        let error = reconcile(&mut kernel, &[], &[rule("/system/a", "/a")])
            .expect_err("missing readback must fail");
        assert!(error.to_string().contains("rollback clean"), "{error}");
    }

    #[test]
    fn refuses_unload_when_foreign_child_depends_on_owned_parent() {
        let before = vec![rule("/system/a", "/a")];
        let mut kernel = Kernel {
            rules: vec![before[0].clone(), rule("/system/a/foreign", "/foreign")],
            ..Default::default()
        };
        assert!(reconcile(&mut kernel, &before, &[]).is_err());
        assert!(kernel.mutations.is_empty());
    }

    #[test]
    fn differing_uids_and_path_components_do_not_conflict() {
        let mut foreign = rule("/system/a", "/foreign");
        foreign.uid = 1000;
        let sibling = rule("/system/ab", "/sibling");
        let mut kernel = Kernel {
            rules: vec![foreign.clone(), sibling.clone()],
            ..Default::default()
        };
        assert!(reconcile(&mut kernel, &[], &[rule("/system/a", "/a")]).is_ok());
        assert!(kernel.rules.contains(&foreign));
        assert!(kernel.rules.contains(&sibling));
    }

    #[test]
    fn semantic_flag_drift_is_rejected() {
        let saved = rule("/system/a", "");
        for flags in [FLAG_WHITEOUT, FLAG_OPAQUE, 1 << 8] {
            let mut actual = saved.clone();
            actual.flags = flags;
            let mut kernel = Kernel {
                rules: vec![actual],
                ..Default::default()
            };
            assert!(reconcile(&mut kernel, std::slice::from_ref(&saved), &[]).is_err());
            assert!(kernel.mutations.is_empty());
        }
    }

    #[test]
    fn rejects_duplicate_targets_before_mutation() {
        let after = vec![rule("/system/a", "/a"), rule("/system/a", "/b")];
        let mut kernel = Kernel::default();
        assert!(reconcile(&mut kernel, &[], &after).is_err());
        assert!(kernel.mutations.is_empty());
    }

    #[test]
    fn rollback_does_not_delete_a_new_target_replaced_by_external_writer() {
        let after = vec![rule("/system/a", "/a"), rule("/system/b", "/b")];
        let foreign = rule("/system/a", "/external");
        let mut kernel = Kernel {
            puts: VecDeque::from([Some(1)]),
            external_on_put_failure: Some(foreign.clone()),
            ..Default::default()
        };
        let error =
            reconcile(&mut kernel, &[], &after).expect_err("external writer must prevent recovery");
        assert!(error.to_string().contains("rollback incomplete"), "{error}");
        assert_eq!(kernel.rules, vec![foreign]);
        assert!(
            !kernel
                .mutations
                .iter()
                .any(|(operation, _)| *operation == '-')
        );
    }

    #[test]
    fn rollback_does_not_overwrite_an_old_target_replaced_by_external_writer() {
        let before = vec![rule("/system/a", "/old")];
        let after = vec![rule("/system/a", "/new"), rule("/system/b", "/b")];
        let foreign = rule("/system/a", "/external");
        let mut kernel = Kernel {
            rules: before.clone(),
            puts: VecDeque::from([Some(1)]),
            external_on_put_failure: Some(foreign.clone()),
            ..Default::default()
        };
        let error = reconcile(&mut kernel, &before, &after)
            .expect_err("external writer must prevent recovery");
        assert!(error.to_string().contains("rollback incomplete"), "{error}");
        assert_eq!(kernel.rules, vec![foreign]);
        assert_eq!(kernel.mutations, vec![('+', "/system/a".into())]);
    }

    #[test]
    fn preflight_rejects_foreign_new_target_without_changing_live_state() {
        let before = vec![rule("/system/owned", "/old")];
        let foreign = rule("/system/new", "/foreign");
        let initial = vec![before[0].clone(), foreign];
        let mut kernel = Kernel {
            rules: initial.clone(),
            ..Default::default()
        };
        let after = vec![rule("/system/new", "/new")];
        assert!(preflight(&mut kernel, &before, &after).is_err());
        assert_eq!(kernel.rules, initial);
        assert!(kernel.mutations.is_empty());
    }

    #[test]
    fn empty_transaction_does_not_mutate_kernel() {
        let mut kernel = Kernel {
            rules: vec![rule("/foreign", "/source")],
            ..Default::default()
        };
        assert!(reconcile(&mut kernel, &[], &[]).is_ok());
        assert!(kernel.mutations.is_empty());
        assert_eq!(kernel.rules.len(), 1);
    }
}
