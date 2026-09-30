// SPDX-License-Identifier: GPL-3.0-only
//! Pure lifecycle checks shared by initial boot and soft-reboot preparation.

use crate::errors::{Error, Result};
use crate::state::RunState;
use std::collections::BTreeSet;

/// A live mount sitting on a target a pre-ledger snapshot recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyConflict {
    pub target: String,
    pub fs_type: String,
    pub source: Option<String>,
}

/// Reports live mounts on targets that `run/state.json` recorded before the ownership ledger
/// existed.
///
/// The snapshot stores targets only, never mount ids or a boot identity, so it cannot prove that a
/// live mount belongs to Hybrid Mount. Callers report these conflicts and keep going: deciding
/// "mine" aborts the whole mount pipeline for the boot (APatch and KernelSU stop after a
/// metamodule script exits non-zero), while deciding "foreign" only leaves a stale mount that the
/// next physical boot removes. Boot-scoped ownership stays in the runtime ledger, and unattributable
/// mounts are never detached.
pub fn legacy_mount_conflicts(
    state: &RunState,
    current: &[(String, String, Option<String>)],
) -> Vec<LegacyConflict> {
    let mut targets: BTreeSet<&str> = state
        .overlay_active_mounts
        .iter()
        .chain(&state.magic_active_mounts)
        .chain(&state.leftover_mount_targets)
        .map(String::as_str)
        .collect();
    if let Some(path) = state.mount_point.to_str().filter(|p| !p.is_empty()) {
        targets.insert(path);
    }
    let mut conflicts = Vec::new();
    for target in targets {
        let matches: Vec<_> = current.iter().filter(|m| m.0 == target).collect();
        // A single ordinary partition mount can survive across physical boots, so only mounts the
        // pre-ledger snapshot could have created are reported. Nested targets and stacks are
        // ambiguous without an ownership ledger.
        let partition = target == "/system"
            || crate::defs::MANAGED_PARTITIONS
                .iter()
                .any(|p| target.strip_prefix('/') == Some(*p));
        for mount in matches {
            let stock_partition = partition
                && matches!(mount.1.as_str(), "ext4" | "erofs")
                && mount
                    .2
                    .as_deref()
                    .is_some_and(|s| s.starts_with("/dev/block/"));
            if !stock_partition {
                conflicts.push(LegacyConflict {
                    target: target.to_owned(),
                    fs_type: mount.1.clone(),
                    source: mount.2.clone(),
                });
            }
        }
    }
    conflicts
}

/// Whether a pre-ledger snapshot can still describe live mounts.
///
/// Mounts never survive a physical boot, so a snapshot written before the ownership ledger cannot
/// outlive the kernel boot that wrote it. The persisted ledger records the boot id of the last
/// Hybrid Mount session, and a different id therefore proves that every target in the snapshot is
/// unreachable. No ledger at all proves nothing, so the caller reports instead of deciding.
pub fn legacy_snapshot_reachable(persisted_boot_id: Option<&str>, current_boot_id: &str) -> bool {
    match persisted_boot_id {
        Some(boot_id) => boot_id == current_boot_id,
        None => true,
    }
}

pub fn owned_uids(requested: &[u32], original: &[u32], observed: &[u32]) -> Vec<u32> {
    observed
        .iter()
        .copied()
        .filter(|uid| requested.contains(uid) && !original.contains(uid))
        .collect()
}

/// A degraded boot has no advertised VFS targets. Otherwise every advertised
/// rule must still match at the final ownership commit.
pub fn confirm_boot_rules(
    planned: &[super::rules::SavedRule],
    active_targets: &[String],
    observed: &[super::rules::SavedRule],
) -> Result<()> {
    for target in active_targets {
        let expected = planned
            .iter()
            .find(|r| &r.virtual_path == target)
            .ok_or_else(|| Error::msg(format!("unplanned active VFS target: {target}")))?;
        if !observed
            .iter()
            .any(|r| super::rules::semantic_equal(expected, r))
        {
            return Err(Error::msg(format!(
                "VFS rule changed before boot ownership commit: {target}"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_advertised_rule_cannot_commit_ready_boot() {
        let rule = super::super::rules::SavedRule {
            module_id: "hosts".into(),
            virtual_path: "/system/etc/hosts".into(),
            real_path: "/data/adb/modules/hosts/system/etc/hosts".into(),
            flags: 0,
            uid: 0,
        };
        assert!(
            confirm_boot_rules(
                std::slice::from_ref(&rule),
                std::slice::from_ref(&rule.virtual_path),
                &[]
            )
            .is_err()
        );
        assert!(confirm_boot_rules(std::slice::from_ref(&rule), &[], &[]).is_ok());
        assert!(
            confirm_boot_rules(
                std::slice::from_ref(&rule),
                std::slice::from_ref(&rule.virtual_path),
                std::slice::from_ref(&rule)
            )
            .is_ok()
        );
    }

    #[test]
    fn pre_ledger_overlay_is_reported_without_failing_the_boot() {
        let state = RunState {
            overlay_active_mounts: vec!["/system".into()],
            ..RunState::default()
        };
        let conflicts = legacy_mount_conflicts(
            &state,
            &[("/system".into(), "overlay".into(), Some("KSU".into()))],
        );
        assert_eq!(
            conflicts,
            vec![LegacyConflict {
                target: "/system".into(),
                fs_type: "overlay".into(),
                source: Some("KSU".into()),
            }]
        );
    }

    #[test]
    fn pre_ledger_magic_bind_with_block_source_is_reported() {
        let state = RunState {
            magic_active_mounts: vec!["/system/etc/hosts".into()],
            ..RunState::default()
        };
        let conflicts = legacy_mount_conflicts(
            &state,
            &[(
                "/system/etc/hosts".into(),
                "ext4".into(),
                Some("/dev/block/dm-1".into()),
            )],
        );
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].target, "/system/etc/hosts");
    }

    #[test]
    fn physical_boot_stock_partition_is_not_legacy_overlay() {
        let state = RunState {
            overlay_active_mounts: vec!["/system".into()],
            ..RunState::default()
        };
        assert!(
            legacy_mount_conflicts(
                &state,
                &[(
                    "/system".into(),
                    "erofs".into(),
                    Some("/dev/block/dm-0".into())
                )]
            )
            .is_empty()
        );
    }

    #[test]
    fn platform_overlay_on_a_recorded_target_is_reported_and_never_reaches_another_boot() {
        // Regression: a snapshot that listed the platform's own `/product/overlay` made a cold
        // boot fail with `untracked mount at /product/overlay`, which stops every module from
        // mounting. The conflict is now informational, and a ledger from an earlier boot proves
        // the recorded targets are unreachable.
        let state = RunState {
            overlay_active_mounts: vec!["/product/overlay".into()],
            ..RunState::default()
        };
        let conflicts = legacy_mount_conflicts(
            &state,
            &[(
                "/product/overlay".into(),
                "overlay".into(),
                Some("overlay-overlay".into()),
            )],
        );
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].source.as_deref(), Some("overlay-overlay"));

        assert!(!legacy_snapshot_reachable(
            Some("previous-boot"),
            "current-boot"
        ));
        assert!(legacy_snapshot_reachable(
            Some("current-boot"),
            "current-boot"
        ));
        assert!(legacy_snapshot_reachable(None, "current-boot"));
    }

    #[test]
    fn a_stack_over_a_stock_partition_reports_only_the_extra_mount() {
        let state = RunState {
            overlay_active_mounts: vec!["/system".into()],
            ..RunState::default()
        };
        let conflicts = legacy_mount_conflicts(
            &state,
            &[
                (
                    "/system".into(),
                    "erofs".into(),
                    Some("/dev/block/dm-0".into()),
                ),
                ("/system".into(), "overlay".into(), Some("overlay".into())),
            ],
        );
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].fs_type, "overlay");
    }

    #[test]
    fn absent_provider_never_owns_requested_isolation_uids() {
        assert!(owned_uids(&[10000], &[], &[]).is_empty());
        assert_eq!(
            owned_uids(&[10000, 10001], &[10000], &[10000, 10001, 10002]),
            vec![10001]
        );
    }
}
