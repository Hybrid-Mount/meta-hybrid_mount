// SPDX-License-Identifier: GPL-3.0-only

use super::ledger::Ledger;
use crate::config::Config;
use crate::errors::{Error, Result};
use crate::plan::{MountPlan, PlanInput, build_plan};
use crate::scanner::ModuleRecord;
use std::collections::BTreeSet;

pub fn plan_hot_module(
    module: &ModuleRecord,
    config: &Config,
    promoted: &BTreeSet<String>,
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
    let mut requested = module.clone();
    requested.disabled = false;
    let plan = build_plan(&PlanInput {
        modules: &[requested],
        config,
        promoted_partitions: promoted,
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
            }],
        }
    }
    #[test]
    fn temporary_load_preserves_disabled_marker_semantics() {
        let config = Config {
            default_mode: Mode::Vfs,
            ..Config::default()
        };
        let module = module();
        assert!(plan_hot_module(&module, &config, &BTreeSet::new(), &Ledger::default()).is_ok());
        assert!(module.disabled);
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
        assert!(plan_hot_module(&module(), &config, &BTreeSet::new(), &Ledger::default()).is_err());
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
        assert!(plan_hot_module(&module(), &config, &BTreeSet::new(), &saved).is_err());
    }
    #[test]
    fn overlap_respects_path_boundaries() {
        assert!(paths_overlap("/system/etc", "/system/etc/a"));
        assert!(!paths_overlap("/system/etc/a", "/system/etc/ab"));
    }
}
