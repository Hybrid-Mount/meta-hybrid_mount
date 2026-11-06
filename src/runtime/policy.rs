// SPDX-License-Identifier: GPL-3.0-only

use super::ledger::Ledger;
use crate::config::Config;
use crate::errors::{Error, Result};
use crate::plan::{MountPlan, PlanInput, build_plan};
use crate::scanner::ModuleRecord;
#[cfg(test)]
use std::collections::BTreeSet;

pub fn plan_hot_module(
    module: &ModuleRecord,
    config: &Config,
    layout: &crate::partitions::PartitionLayout,
    saved: &Ledger,
) -> Result<MountPlan> {
    if saved.non_vfs_modules.contains(module.id.as_str()) {
        return Err(Error::msg(
            "module has active Magic/Overlay ownership; reboot required",
        ));
    }
    if module.skip_mount
        || config.is_module_blacklisted(module.id.as_str())
        || module.source_path.join("remove").exists()
    {
        return Err(Error::msg(
            "module is skipped, blacklisted, or pending removal",
        ));
    }
    // Temporary load is independent of the persistent disable marker.
    let rules = crate::plan::ModuleRulesView::new(&module.id, config, true);
    let mut relevant = std::collections::BTreeSet::new();
    for entry in &module.entries {
        let mode = rules.resolve_mode(&entry.relative);
        if mode == crate::config::Mode::Ignore {
            continue;
        }
        if mode != crate::config::Mode::Vfs {
            return Err(Error::msg(
                "hot operations require a pure VFS configured module; skipped partitions cannot hide other backends",
            ));
        }
        relevant.insert(layout.logical_partition(&entry.relative).to_owned());
    }
    // Reload must also protect an old rule whose source was removed meanwhile.
    if let Some(baseline) = &saved.partition_layout {
        for rule in saved
            .rules
            .iter()
            .filter(|r| r.module_id == module.id.as_str())
        {
            if let Some((name, _)) = baseline
                .roots
                .iter()
                .filter(|(_, root)| std::path::Path::new(&rule.virtual_path).starts_with(root))
                .max_by_key(|(_, root)| root.components().count())
            {
                relevant.insert(name.clone());
            }
        }
    }
    ensure_layout_unchanged(layout, saved, &relevant)?;
    let mut requested = module.clone();
    requested.disabled = false;
    let plan = build_plan(&PlanInput {
        modules: &[requested],
        config,
        layout,
        vfs_available: true,
    })?;
    if !plan.magic_module_ids.is_empty()
        || !plan.overlay_module_ids.is_empty()
        || plan.vfs_module_ids.is_empty()
    {
        return Err(Error::msg(
            "hot operations require a pure VFS file plan; mixed backends require reboot",
        ));
    }
    for rule in crate::vfs::rule::build_vfs_rules(&plan.tree) {
        let encoded = crate::vfs::protocol::encode_rule(&rule)?;
        let target = String::from_utf8_lossy(&encoded.virtual_path);
        if saved
            .mounts
            .iter()
            .any(|m| paths_overlap(&target, &m.target))
        {
            return Err(Error::msg(format!(
                "VFS target overlaps an active managed mount: {target}"
            )));
        }
    }
    Ok(plan)
}

/// Callers validate every owned mount's live identity before allowing these
/// exact targets to explain expected changes relative to the boot snapshot.
pub fn ensure_layout_unchanged(
    current: &crate::partitions::PartitionLayout,
    saved: &Ledger,
    relevant: &std::collections::BTreeSet<String>,
) -> Result<()> {
    let baseline = saved.partition_layout.as_ref().ok_or_else(|| {
        Error::msg("runtime ledger has no partition layout baseline; reboot before loading rules")
    })?;
    for name in relevant {
        let same_error = match (baseline.errors.get(name), current.errors.get(name)) {
            (None, None) => true,
            (Some(a), Some(b)) => a.code == b.code && a.reason == b.reason,
            _ => false,
        };
        let same_identity = match (baseline.identities.get(name), current.identities.get(name)) {
            (None, None) => true,
            (Some(a), Some(b)) => {
                let clean = |id: &crate::partitions::PartitionIdentity| {
                    id.subtree
                        .iter()
                        .filter(|(rel, _)| {
                            let target = id.canonical.join(rel);
                            !saved
                                .mounts
                                .iter()
                                .any(|m| std::path::Path::new(&m.target) == target)
                        })
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect::<std::collections::BTreeMap<_, _>>()
                };
                a.canonical == b.canonical
                    && a.dev == b.dev
                    && a.ino == b.ino
                    && clean(a) == clean(b)
            }
            _ => false,
        };
        if baseline.roots.get(name) != current.roots.get(name) || !same_error || !same_identity {
            return Err(Error::msg(format!(
                "partition {name} layout changed since boot; reboot before loading rules"
            )));
        }
    }
    Ok(())
}

pub fn paths_overlap(a: &str, b: &str) -> bool {
    crate::utils::is_same_or_below(a, b) || crate::utils::is_same_or_below(b, a)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{Mode, ModuleRule},
        module_id::ModuleId,
        mount_tree::NodeFileType,
        scanner::ModuleEntry,
    };
    use std::{collections::BTreeMap, path::PathBuf};
    fn module() -> ModuleRecord {
        ModuleRecord {
            id: ModuleId::try_from("demo").unwrap(),
            name: "demo".into(),
            version: "1".into(),
            author: "a".into(),
            description: "d".into(),
            disabled: true,
            skip_mount: false,
            has_mount_files: true,
            source_path: PathBuf::from("/nonexistent/demo"),
            entries: vec![ModuleEntry {
                relative: "system/etc/demo".into(),
                file_type: NodeFileType::RegularFile,
                replace: false,
                link_target: None,
                directory_remove: false,
                marker_error: None,
            }],
        }
    }
    #[test]
    fn layout_changes_only_block_the_partitions_a_hot_operation_uses() {
        let mut baseline = crate::partitions::PartitionLayout::default();
        baseline.roots.insert("system".into(), "/system".into());
        baseline.roots.insert("vendor".into(), "/vendor".into());
        let saved = Ledger {
            partition_layout: Some(baseline.clone()),
            ..Ledger::default()
        };
        let mut current = baseline.clone();
        current
            .roots
            .insert("vendor".into(), "/system/vendor".into());
        assert!(
            ensure_layout_unchanged(&current, &saved, &BTreeSet::from(["system".into()])).is_ok()
        );
        assert!(
            ensure_layout_unchanged(&current, &saved, &BTreeSet::from(["vendor".into()])).is_err()
        );
    }

    #[test]
    fn only_validated_exact_owned_mounts_explain_expected_subtree_changes() {
        let mut baseline = crate::partitions::PartitionLayout::default();
        baseline.roots.insert("system".into(), "/system".into());
        baseline.identities.insert(
            "system".into(),
            crate::partitions::PartitionIdentity {
                canonical: "/system".into(),
                dev: 7,
                ino: 42,
                ..crate::partitions::PartitionIdentity::default()
            },
        );
        let mut saved = Ledger {
            partition_layout: Some(baseline.clone()),
            ..Ledger::default()
        };
        let mut current = baseline;
        current
            .identities
            .get_mut("system")
            .unwrap()
            .subtree
            .insert("framework".into(), (8, 88, "0:8".into(), "/".into()));
        let relevant = BTreeSet::from(["system".into()]);
        assert!(ensure_layout_unchanged(&current, &saved, &relevant).is_err());
        saved.mounts.push(super::super::ledger::OwnedMount {
            target: "/system/framework".into(),
            id: 7,
            ..super::super::ledger::OwnedMount::default()
        });
        assert!(ensure_layout_unchanged(&current, &saved, &relevant).is_ok());
        current
            .identities
            .get_mut("system")
            .unwrap()
            .subtree
            .insert(
                "framework/unowned".into(),
                (9, 99, "0:9".into(), "/".into()),
            );
        assert!(ensure_layout_unchanged(&current, &saved, &relevant).is_err());
    }

    #[test]
    fn legacy_ledger_cannot_guess_a_layout_for_hot_load() {
        let config = Config {
            default_mode: Mode::Vfs,
            ..Config::default()
        };
        assert!(
            plan_hot_module(
                &module(),
                &config,
                &crate::partitions::PartitionLayout::default(),
                &Ledger::default()
            )
            .is_err()
        );
    }

    #[test]
    fn temporary_load_preserves_disabled_marker_semantics() {
        let config = Config {
            default_mode: Mode::Vfs,
            ..Config::default()
        };
        let module = module();
        assert!(
            plan_hot_module(
                &module,
                &config,
                &crate::partitions::PartitionLayout::default(),
                &Ledger {
                    partition_layout: Some(crate::partitions::PartitionLayout::default()),
                    ..Ledger::default()
                }
            )
            .is_ok()
        );
    }
    #[test]
    fn rejects_mixed_module_before_any_kernel_work() {
        let mut config = Config {
            default_mode: Mode::Vfs,
            ..Config::default()
        };
        config.rules.insert(
            module().id,
            ModuleRule {
                default_mode: Some(Mode::Magic),
                paths: BTreeMap::new(),
            },
        );
        assert!(
            plan_hot_module(
                &module(),
                &config,
                &crate::partitions::PartitionLayout::default(),
                &Ledger::default()
            )
            .is_err()
        );
    }
    #[test]
    fn saved_non_vfs_ownership_blocks_switching_backend_live() {
        let config = Config {
            default_mode: Mode::Vfs,
            ..Config::default()
        };
        let saved = Ledger {
            non_vfs_modules: BTreeSet::from(["demo".into()]),
            ..Ledger::default()
        };
        assert!(
            plan_hot_module(
                &module(),
                &config,
                &crate::partitions::PartitionLayout::default(),
                &saved
            )
            .is_err()
        );
    }
    #[test]
    fn overlap_respects_path_boundaries() {
        assert!(paths_overlap("/system/etc", "/system/etc/a"));
        assert!(!paths_overlap("/system/etc/a", "/system/etc/ab"));
    }
}
