// SPDX-License-Identifier: GPL-3.0-only

//! Maps the nodes marked `vfs` in the shared mount tree to sendable rules.
//! Plain directories are traversed for structure only; a `.replace` directory replaces
//! its whole subtree, so every directory inside it needs an opaque rule.

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
    /// The directory stays visible, real entries are hidden, only injected children
    /// show through (`.replace`).
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

/// A set `opaque_owner` means this node is inside a `.replace` subtree. Every directory
/// in that subtree needs an opaque rule: the kernel treats a directory without one as
/// nonexistent, which would make deeper injected files unreachable.
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

    // A `.replace` directory claims its whole subtree; descendants inherit the owner.
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
        // At most one rule per target: the kernel indexes rules by virtual path, so
        // duplicates overwrite each other or fail the batch. The last source in module
        // order wins, matching how an upper overlay layer takes precedence.
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
