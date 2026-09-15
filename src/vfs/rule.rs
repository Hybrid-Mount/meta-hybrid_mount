// SPDX-License-Identifier: GPL-3.0-only

//! 把共享挂载树里标注为 `vfs` 的节点映射为可下发的规则。
//! 普通目录只作为结构遍历，不产生规则；`.replace` 目录按 Magisk 语义替换整个子树，
//! 因此该子树内的每个目录都要下发一条 opaque 规则。

use std::path::PathBuf;

use crate::config::Mode;
use crate::module_id::ModuleId;
use crate::mount_tree::{MountNode, MountTree, NodeFileType};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VfsAction {
    Inject {
        virtual_path: String,
        real_path: PathBuf,
    },
    Whiteout {
        virtual_path: String,
    },
    /// 目录保持可见，真实条目全部隐藏，只显示注入子项（`.replace`）。
    OpaqueDir {
        virtual_path: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VfsRule {
    pub action: VfsAction,
    pub module_id: ModuleId,
}

pub fn build_vfs_rules(tree: &MountTree) -> Vec<VfsRule> {
    let mut rules = Vec::new();
    collect_node(&tree.root, "", &mut rules, None);
    rules
}

/// `opaque_owner` 非空表示当前节点位于某个 `.replace` 目录的子树内。子树内每个目录都要
/// 下发 opaque 规则：内核的虚拟目录会把没有注入规则的子目录当作不存在，后代目录若不带
/// 规则，即使其文件已注入也不可达。
fn collect_node(
    node: &MountNode,
    target: &str,
    out: &mut Vec<VfsRule>,
    opaque_owner: Option<&ModuleId>,
) {
    let current = if node.name.is_empty() {
        target.to_owned()
    } else if target.is_empty() {
        format!("/{}", node.name)
    } else {
        format!("{target}/{}", node.name)
    };

    let vfs_source = node.source_for(Mode::Vfs);
    let is_vfs_dir = vfs_source.is_some_and(|source| source.file_type == NodeFileType::Directory)
        || node
            .children
            .values()
            .any(|child| child.has_backend(Mode::Vfs));

    // `.replace` 目录夺取整棵子树的归属；后代目录继承它，用于统计与回滚。
    let owner = match vfs_source {
        Some(source) if source.replace => Some(source.module_id.clone()),
        _ => opaque_owner.cloned(),
    };

    if is_vfs_dir {
        if let Some(owner) = &owner {
            out.push(VfsRule {
                action: VfsAction::OpaqueDir {
                    virtual_path: current.clone(),
                },
                module_id: owner.clone(),
            });
        }
    } else if let Some(source) =
        node.sources.iter().rev().find(|source| {
            source.backend == Mode::Vfs && source.file_type != NodeFileType::Directory
        })
    {
        // 每个目标最多一条规则：VFS 内核按虚拟路径索引规则，同目标多条会互相覆盖甚至
        // 整批失败。优先级沿用模块顺序语义——node.sources 按 module_id 升序，最后一个
        // Vfs 来源（模块顺序靠后）获胜，与 Overlay 上层 / 内核 add 覆盖一致。
        let action = match source.file_type {
            NodeFileType::Whiteout => VfsAction::Whiteout {
                virtual_path: current.clone(),
            },
            NodeFileType::RegularFile | NodeFileType::Symlink => VfsAction::Inject {
                virtual_path: current.clone(),
                real_path: source.source_path.clone(),
            },
            NodeFileType::Directory => return,
        };
        out.push(VfsRule {
            action,
            module_id: source.module_id.clone(),
        });
    }

    for child in node.children.values() {
        collect_node(child, &current, out, owner.as_ref());
    }
}

#[cfg(test)]
#[path = "rule_tests.rs"]
mod tests;
