// SPDX-License-Identifier: GPL-3.0-only

//! Apply a planned batch to the bound provider and summarise the result.
//! Rollback belongs to the pipeline: the full batch is registered before any kernel
//! call, so a partial failure can be undone record by record.

use std::collections::BTreeSet;

use crate::errors::Result;
use crate::module_id::ModuleId;
use crate::plan::MountPlan;
use crate::vfs::backend::VfsKernel;
use crate::vfs::protocol::{EncodedRule, ListedRule, encode_rule};
use crate::vfs::rule::{VfsAction, build_vfs_rules};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VfsExecStats {
    pub mounted_module_ids: Vec<String>,
    pub active_targets: Vec<String>,
    pub injected: usize,
    pub whiteouts: usize,
    pub opaque: usize,
    /// A non-strict VFS failure that was deliberately degraded so other backends could boot.
    pub failure: Option<String>,
}

/// The result of one apply: statistics plus the rules needed to roll back each record.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VfsApplied {
    pub stats: VfsExecStats,
    pub rules: Vec<EncodedRule>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VfsPolicyOutcome {
    pub stats: Option<VfsExecStats>,
    pub failure: Option<String>,
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
            failure: None,
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

/// Compatibility wrapper for callers that only need to know whether VFS applied.
#[allow(dead_code)]
pub fn apply_rules_with_policy(
    kernel: &mut dyn VfsKernel,
    applied: &VfsApplied,
    uids: &[u32],
    strict: bool,
) -> Result<Option<VfsExecStats>> {
    apply_rules_with_policy_diagnosed(kernel, applied, uids, strict).map(|outcome| outcome.stats)
}

/// Returns a successful batch or a non-strict failure diagnosis. Both error paths first delete
/// the applied prefix, so a caller can safely continue with other backends.
pub fn apply_rules_with_policy_diagnosed(
    kernel: &mut dyn VfsKernel,
    applied: &VfsApplied,
    uids: &[u32],
    strict: bool,
) -> Result<VfsPolicyOutcome> {
    match apply_rules(kernel, applied, uids) {
        Ok(stats) => Ok(VfsPolicyOutcome {
            stats: Some(stats),
            failure: None,
        }),
        Err(err) => {
            if let Err(cleanup) = kernel.remove_rules(&applied.rules) {
                log::error!("vfs rollback after a failed apply also failed: {cleanup}");
                return Err(crate::errors::Error::VfsProtocol {
                    detail: format!(
                        "vfs apply failed and targeted cleanup failed: {cleanup}; apply error: {err}"
                    ),
                });
            }
            if strict {
                Err(err)
            } else {
                log::warn!("vfs apply failed, continuing without the vfs backend: {err}");
                Ok(VfsPolicyOutcome {
                    stats: None,
                    failure: Some(err.to_string()),
                })
            }
        }
    }
}

/// Virtual paths of `expected` rules the provider did not report back.
///
/// An acknowledged batch is not proof the rules are installed, so the pipeline reads the table
/// back after applying and reports the difference; a device log can then distinguish "VFS is
/// active" from "the kernel accepted the batch and dropped it". Comparison is by virtual path
/// because that is the module's own key: re-adding one shadows the existing rule rather than
/// creating a second, which is why `DEL_RULE` indexes by path too.
pub fn missing_rules(expected: &[EncodedRule], listed: &[ListedRule]) -> Vec<String> {
    let installed: BTreeSet<&str> = listed
        .iter()
        .map(|rule| rule.virtual_path.as_str())
        .collect();
    expected
        .iter()
        .map(|rule| String::from_utf8_lossy(&rule.virtual_path).into_owned())
        .filter(|path| !installed.contains(path.as_str()))
        .collect()
}

/// Decides whether a VFS read-back mismatch may degrade or must fail the boot.
/// Returns `true` only when every expected rule was confirmed.
pub fn evaluate_readback(missing: &[String], strict: bool) -> Result<bool> {
    if missing.is_empty() {
        return Ok(true);
    }
    if strict {
        return Err(crate::errors::Error::VfsProtocol {
            detail: format!(
                "read-back did not confirm {} installed rule(s): {}",
                missing.len(),
                missing.join(",")
            ),
        });
    }
    Ok(false)
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
