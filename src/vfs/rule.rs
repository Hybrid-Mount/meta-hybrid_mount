// SPDX-License-Identifier: GPL-3.0-only

//! 把共享挂载树里标注为 `vfs` 的节点映射为可下发的规则。
//! 目录只作为结构遍历，不产生规则；实际目录合并由内核虚拟拓扑完成。

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
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VfsRule {
    pub action: VfsAction,
    pub module_id: ModuleId,
}

pub fn build_vfs_rules(tree: &MountTree) -> Vec<VfsRule> {
    let mut rules = Vec::new();
    collect_node(&tree.root, "", &mut rules);
    rules
}

fn collect_node(node: &MountNode, target: &str, out: &mut Vec<VfsRule>) {
    let current = if node.name.is_empty() {
        target.to_owned()
    } else if target.is_empty() {
        format!("/{}", node.name)
    } else {
        format!("{target}/{}", node.name)
    };

    for source in &node.sources {
        if source.backend != Mode::Vfs {
            continue;
        }
        let action = match source.file_type {
            NodeFileType::Directory => continue,
            NodeFileType::Whiteout => VfsAction::Whiteout {
                virtual_path: current.clone(),
            },
            NodeFileType::RegularFile | NodeFileType::Symlink => VfsAction::Inject {
                virtual_path: current.clone(),
                real_path: source.source_path.clone(),
            },
        };
        out.push(VfsRule {
            action,
            module_id: source.module_id.clone(),
        });
    }

    for child in node.children.values() {
        collect_node(child, &current, out);
    }
}

#[cfg(test)]
#[path = "rule_tests.rs"]
mod tests;
