// SPDX-License-Identifier: GPL-3.0-only

//! 把规划结果应用到当前绑定的 Provider，并汇总统计。
//! 回滚由流水线统一负责：对本次下发的规则逐条 DEL_RULE（见 `VfsApplied::rules`）。

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
}

/// 本次下发的结果：统计与逐条回滚所需的规则。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VfsApplied {
    pub stats: VfsExecStats,
    pub rules: Vec<EncodedRule>,
}

pub fn apply_plan(
    kernel: &mut dyn VfsKernel,
    plan: &MountPlan,
    uids: &[u32],
) -> Result<VfsApplied> {
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
        },
        rules: encoded,
    })
}

#[cfg(test)]
#[path = "exec_tests.rs"]
mod tests;
