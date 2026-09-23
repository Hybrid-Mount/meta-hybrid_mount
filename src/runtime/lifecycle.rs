// SPDX-License-Identifier: GPL-3.0-only
//! Pure lifecycle checks shared by initial boot and soft-reboot preparation.

use crate::errors::{Error, Result};
use crate::state::RunState;
use std::collections::BTreeSet;

pub fn reject_legacy_mounts(
    state: &RunState,
    current: &[(String, String, Option<String>)],
) -> Result<()> {
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
    for target in targets {
        let matches: Vec<_> = current.iter().filter(|m| m.0 == target).collect();
        // A single ordinary partition mount can survive across physical boots.
        // Nested targets and stacks are ambiguous without an ownership ledger.
        let partition = target == "/system"
            || crate::defs::MANAGED_PARTITIONS
                .iter()
                .any(|p| target.strip_prefix('/') == Some(*p));
        let stock_partition = partition
            && matches.len() == 1
            && matches!(matches[0].1.as_str(), "ext4" | "erofs")
            && matches[0]
                .2
                .as_deref()
                .is_some_and(|s| s.starts_with("/dev/block/"));
        if !matches.is_empty() && !stock_partition {
            return Err(Error::msg(format!(
                "untracked mount at {target}; a full reboot is required before runtime management"
            )));
        }
    }
    Ok(())
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
    fn old_overlay_without_ledger_must_not_be_cleared_as_clean() {
        let state = RunState {
            overlay_active_mounts: vec!["/system".into()],
            ..RunState::default()
        };
        assert!(
            reject_legacy_mounts(
                &state,
                &[("/system".into(), "overlay".into(), Some("KSU".into()))]
            )
            .is_err()
        );
    }

    #[test]
    fn old_magic_bind_with_block_source_is_still_untracked() {
        let state = RunState {
            magic_active_mounts: vec!["/system/etc/hosts".into()],
            ..RunState::default()
        };
        assert!(
            reject_legacy_mounts(
                &state,
                &[(
                    "/system/etc/hosts".into(),
                    "ext4".into(),
                    Some("/dev/block/dm-1".into())
                )]
            )
            .is_err()
        );
    }

    #[test]
    fn physical_boot_stock_partition_is_not_legacy_overlay() {
        let state = RunState {
            overlay_active_mounts: vec!["/system".into()],
            ..RunState::default()
        };
        assert!(
            reject_legacy_mounts(
                &state,
                &[(
                    "/system".into(),
                    "erofs".into(),
                    Some("/dev/block/dm-0".into())
                )]
            )
            .is_ok()
        );
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
