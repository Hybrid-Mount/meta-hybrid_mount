// SPDX-License-Identifier: GPL-3.0-only

//! Apply a planned batch to the bound provider and summarise the result.
//! Rollback belongs to the pipeline: the full batch is registered before any kernel
//! call, so a partial failure can be undone record by record.

use std::collections::BTreeSet;

use crate::errors::Result;
use crate::module_id::ModuleId;
use crate::plan::MountPlan;
use crate::vfs::backend::VfsKernel;
use crate::vfs::protocol::{EncodedRule, encode_rule};
use crate::vfs::rule::{VfsAction, build_vfs_rules};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VfsExecStats {
    pub mounted_module_ids: Vec<String>,
    pub active_targets: Vec<String>,
    pub injected: usize,
    pub whiteouts: usize,
    pub opaque: usize,
}

/// The result of one apply: statistics plus the rules needed to roll back each record.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VfsApplied {
    pub stats: VfsExecStats,
    pub rules: Vec<EncodedRule>,
}

/// Build the full batch and its statistics without touching the kernel.
pub fn plan_rules(plan: &MountPlan) -> Result<VfsApplied> {
    let rules = build_vfs_rules(&plan.tree);
    let mut encoded = Vec::with_capacity(rules.len());
    let mut active_targets = Vec::with_capacity(rules.len());
    let mut injected = 0;
    let mut whiteouts = 0;
    let mut opaque = 0;
    for rule in &rules {
        match &rule.action {
            VfsAction::Inject { virtual_path, .. } => {
                injected += 1;
                active_targets.push(virtual_path.clone());
            }
            VfsAction::Whiteout { virtual_path } => {
                whiteouts += 1;
                active_targets.push(virtual_path.clone());
            }
            VfsAction::OpaqueDir { virtual_path } => {
                opaque += 1;
                active_targets.push(virtual_path.clone());
            }
        }
        encoded.push(encode_rule(rule)?);
    }

    Ok(VfsApplied {
        stats: VfsExecStats {
            mounted_module_ids: plan
                .vfs_module_ids
                .iter()
                .map(ModuleId::to_string)
                .collect(),
            active_targets,
            injected,
            whiteouts,
            opaque,
        },
        rules: encoded,
    })
}

/// Sends the batch. Not atomic: on failure the caller rolls back the applied prefix.
pub fn apply_rules(
    kernel: &mut dyn VfsKernel,
    applied: &VfsApplied,
    uids: &[u32],
) -> Result<VfsExecStats> {
    kernel.apply_rules(&applied.rules)?;
    let uids = dedupe_uids(uids);
    if !uids.is_empty() {
        kernel.add_uids(&uids)?;
    }
    Ok(applied.stats.clone())
}

/// `Ok(Some(stats))` when the batch took effect, `Ok(None)` when a non-strict failure
/// degraded to running without VFS. Both error paths first delete the applied prefix.
pub fn apply_rules_with_policy(
    kernel: &mut dyn VfsKernel,
    applied: &VfsApplied,
    uids: &[u32],
    strict: bool,
) -> Result<Option<VfsExecStats>> {
    match apply_rules(kernel, applied, uids) {
        Ok(stats) => Ok(Some(stats)),
        Err(err) => {
            if let Err(cleanup) = kernel.remove_rules(&applied.rules) {
                log::error!("vfs rollback after a failed apply also failed: {cleanup}");
            }
            if strict {
                Err(err)
            } else {
                log::warn!("vfs apply failed, continuing without the vfs backend: {err}");
                Ok(None)
            }
        }
    }
}

/// Order-preserving dedupe. The kernel rejects an already-isolated uid with `-EEXIST`,
/// which the protocol layer treats as a hard error.
fn dedupe_uids(uids: &[u32]) -> Vec<u32> {
    let mut seen = BTreeSet::new();
    uids.iter()
        .copied()
        .filter(|uid| seen.insert(*uid))
        .collect()
}

#[cfg(test)]
#[path = "exec_tests.rs"]
mod tests;
