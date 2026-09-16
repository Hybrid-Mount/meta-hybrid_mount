// SPDX-License-Identifier: GPL-3.0-only

//! 把规划结果应用到当前绑定的 Provider，并汇总统计。
//! 回滚由流水线统一负责：下发前先登记完整批次，失败时对该批次逐条 DEL_RULE。

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
    let uids = dedupe_uids(uids);
    if !uids.is_empty() {
        kernel.add_uids(&uids)?;
    }
    Ok(applied.stats.clone())
}

/// 带降级策略的下发：VFS 自身的失败按 `vfs_strict` 决定是否致命。
///
/// 返回 `Ok(Some(stats))` 表示批次已生效；`Ok(None)` 表示非 strict 下降级为「本次不
/// 使用 VFS」。非 strict 时 VFS 是可选后端，单条坏规则（例如源路径在开机早期尚不可
/// 解析）不该拖垮已经成功的 Overlay / Magic 挂载。两条错误路径都会先按批次定向删除
/// 已生效的前缀，再决定是降级还是把错误交回调用方。
pub fn apply_rules_with_policy(
    kernel: &mut dyn VfsKernel,
    applied: &VfsApplied,
    uids: &[u32],
    strict: bool,
) -> Result<Option<VfsExecStats>> {
    match apply_rules(kernel, applied, uids) {
        Ok(stats) => Ok(Some(stats)),
        Err(err) => {
            // 批次非原子：失败时可能已有前缀生效，必须定向删除。未生效的规则由
            // DEL_RULE 的 ENOENT 容忍。
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

/// 保序去重。内核的 ADD_UID 对已存在的 UID 回 -EEXIST，而 `ensure_status` 把任何负值
/// 当硬错误，会把整条挂载流水线拖进回滚；配置里写重、或同一次启动内第二次运行流水线
/// （UID 表在重启前不清空）都会命中这条路径，因此在用户态先收敛。
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
