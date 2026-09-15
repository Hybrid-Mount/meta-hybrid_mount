// SPDX-License-Identifier: GPL-3.0-only

//! 把规划结果应用到当前绑定的 Provider，并汇总统计。
//! 回滚由流水线统一负责：下发前先登记完整批次，失败时对该批次逐条 DEL_RULE。

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

/// 本次下发的结果：统计与逐条回滚所需的规则。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VfsApplied {
    pub stats: VfsExecStats,
    pub rules: Vec<EncodedRule>,
}

/// 构建本次要下发的完整批次与统计，不触碰内核。
///
/// 与下发分离，是为了让调用方在任何内核调用之前就持有完整批次：`apply_rules`
/// 非原子，中途失败时已生效的前缀同样必须回滚，未生效的规则由 DEL_RULE 的
/// ENOENT 容忍。
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

/// 下发已构建的批次；成功时返回统计。非原子：中途失败时已生效的前缀由调用方回滚。
pub fn apply_rules(
    kernel: &mut dyn VfsKernel,
    applied: &VfsApplied,
    uids: &[u32],
) -> Result<VfsExecStats> {
    kernel.apply_rules(&applied.rules)?;
    if !uids.is_empty() {
        kernel.add_uids(uids)?;
    }
    Ok(applied.stats.clone())
}

#[cfg(test)]
#[path = "exec_tests.rs"]
mod tests;
