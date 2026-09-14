// SPDX-License-Identifier: GPL-3.0-only

//! 把规划结果应用到当前绑定的 Provider，并汇总统计。
//! 回滚由流水线统一负责（`clear_rules`）。

use crate::errors::Result;
use crate::module_id::ModuleId;
use crate::plan::MountPlan;
use crate::vfs::backend::VfsKernel;
use crate::vfs::protocol::encode_rule;
use crate::vfs::rule::{VfsAction, build_vfs_rules};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VfsExecStats {
    pub mounted_module_ids: Vec<String>,
    pub active_targets: Vec<String>,
    pub injected: usize,
    pub whiteouts: usize,
}

pub fn apply_plan(
    kernel: &mut dyn VfsKernel,
    plan: &MountPlan,
    uids: &[u32],
) -> Result<VfsExecStats> {
    let rules = build_vfs_rules(&plan.tree);
    let mut encoded = Vec::with_capacity(rules.len());
    let mut active_targets = Vec::with_capacity(rules.len());
    let mut injected = 0;
    let mut whiteouts = 0;
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
        }
        encoded.push(encode_rule(rule)?);
    }

    kernel.apply_rules(&encoded)?;
    if !uids.is_empty() {
        kernel.add_uids(uids)?;
    }

    Ok(VfsExecStats {
        mounted_module_ids: plan
            .vfs_module_ids
            .iter()
            .map(ModuleId::to_string)
            .collect(),
        active_targets,
        injected,
        whiteouts,
    })
}

#[cfg(test)]
#[path = "exec_tests.rs"]
mod tests;
