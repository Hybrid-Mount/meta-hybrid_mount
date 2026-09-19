// SPDX-License-Identifier: GPL-3.0-only

//! Shared mount tree: one target per real mount path, carrying every module's contribution.
//!
//! The scanner identifies node types and `.replace` / whiteout semantics read-only, then the
//! planner labels each module contribution overlay, magic, VFS or ignore. Execution only consumes
//! this tree; it never rescans module directories or keeps a second path-filtering protocol.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::FileType;
use std::path::{Path, PathBuf};

use crate::config::Mode;
use crate::module_id::ModuleId;

/// Built-in partition promotion rules: `(partition, requires /system/<partition> to be a symlink)`.
pub const BUILTIN_PARTITIONS: [(&str, bool); 4] = [
    ("vendor", true),
    ("system_ext", true),
    ("product", true),
    ("odm", false),
];

/// Node types recorded by the scanner and consumed by each backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NodeFileType {
    RegularFile,
    Directory,
    Symlink,
    Whiteout,
}

impl NodeFileType {
    /// Convert ordinary filesystem entry types. Whiteouts need device metadata
    /// and are classified by the scanner; other special entries are rejected.
    pub fn from_file_type(value: FileType) -> Option<Self> {
        if value.is_file() {
            Some(Self::RegularFile)
        } else if value.is_dir() {
            Some(Self::Directory)
        } else if value.is_symlink() {
            Some(Self::Symlink)
        } else {
            None
        }
    }
}

impl fmt::Display for NodeFileType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::RegularFile => "RegularFile",
            Self::Directory => "Directory",
            Self::Symlink => "Symlink",
            Self::Whiteout => "Whiteout",
        })
    }
}

/// One module's contribution to a target node. `source_path` always points into the read-only module source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MountSource {
    pub module_id: ModuleId,
    pub relative: String,
    pub source_path: PathBuf,
    pub file_type: NodeFileType,
    pub replace: bool,
    pub backend: Mode,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StructuralSource {
    pub module_id: ModuleId,
    pub source_path: PathBuf,
}

/// A tree keyed by real mount targets. One target can carry several module layers for the same backend.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MountNode {
    pub name: String,
    pub children: BTreeMap<String, MountNode>,
    pub sources: Vec<MountSource>,
    pub structural_sources: Vec<StructuralSource>,
}

impl MountNode {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            children: BTreeMap::new(),
            sources: Vec::new(),
            structural_sources: Vec::new(),
        }
    }

    /// The winning contribution for this backend on this target, ordered like the Overlay lowerdirs.
    pub fn source_for(&self, backend: Mode) -> Option<&MountSource> {
        self.sources.iter().find(|source| source.backend == backend)
    }

    /// A directory source that only carries backend descendants; it does not switch this source to the target backend.
    pub fn structural_path(&self) -> Option<&Path> {
        self.sources
            .iter()
            .find(|source| source.file_type == NodeFileType::Directory)
            .map(|source| source.source_path.as_path())
            .or_else(|| {
                self.structural_sources
                    .first()
                    .map(|source| source.source_path.as_path())
            })
    }

    pub fn has_backend(&self, backend: Mode) -> bool {
        self.source_for(backend).is_some()
            || self
                .children
                .values()
                .any(|child| child.has_backend(backend))
    }

    /// The node type the executor sees. With no contribution of its own but descendants for this backend, it is a structural directory.
    pub fn file_type_for(&self, backend: Mode) -> Option<NodeFileType> {
        self.source_for(backend)
            .map(|source| source.file_type)
            .or_else(|| {
                self.children
                    .values()
                    .any(|child| child.has_backend(backend))
                    .then_some(NodeFileType::Directory)
            })
    }

    /// The backend's own source wins; a structural directory may borrow any module directory's metadata for staging.
    pub fn module_path_for(&self, backend: Mode) -> Option<&Path> {
        self.source_for(backend)
            .map(|source| source.source_path.as_path())
            .or_else(|| self.structural_path())
    }

    pub fn replace_for(&self, backend: Mode) -> bool {
        self.source_for(backend)
            .is_some_and(|source| source.replace)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MountTree {
    pub root: MountNode,
}

impl Default for MountTree {
    fn default() -> Self {
        Self {
            root: MountNode::new(""),
        }
    }
}

impl MountTree {
    pub fn insert(&mut self, target: &str, source: MountSource) {
        let target_components = target
            .trim()
            .trim_start_matches('/')
            .split('/')
            .filter(|component| !component.is_empty())
            .collect::<Vec<_>>();
        let relative_components = source
            .relative
            .split('/')
            .filter(|component| !component.is_empty())
            .collect::<Vec<_>>();
        let source_offset = relative_components
            .len()
            .saturating_sub(target_components.len());

        let mut node = &mut self.root;
        for (index, component) in target_components.iter().enumerate() {
            node = node
                .children
                .entry((*component).to_owned())
                .or_insert_with(|| MountNode::new(component));

            let kept_source_components = source_offset + index + 1;
            let is_structural_directory = kept_source_components < relative_components.len()
                || source.file_type == NodeFileType::Directory;
            if is_structural_directory {
                let mut structural_path = source.source_path.clone();
                for _ in kept_source_components..relative_components.len() {
                    structural_path.pop();
                }
                if !node.structural_sources.iter().any(|structural| {
                    structural.module_id == source.module_id
                        && structural.source_path == structural_path
                }) {
                    node.structural_sources.push(StructuralSource {
                        module_id: source.module_id.clone(),
                        source_path: structural_path,
                    });
                    node.structural_sources.sort_by(|left, right| {
                        left.module_id
                            .cmp(&right.module_id)
                            .then_with(|| left.source_path.cmp(&right.source_path))
                    });
                }
            }
        }
        node.sources.push(source);
        node.sources.sort_by(|left, right| {
            left.module_id
                .cmp(&right.module_id)
                .then_with(|| left.relative.cmp(&right.relative))
        });
    }

    pub fn find(&self, target: &str) -> Option<&MountNode> {
        let mut node = &self.root;
        for component in target
            .trim()
            .trim_start_matches('/')
            .split('/')
            .filter(|component| !component.is_empty())
        {
            node = node.children.get(component)?;
        }
        Some(node)
    }

    /// Module contributions for a successfully mounted target, counting only that node's own sources.
    pub fn module_ids_for_target(&self, backend: Mode, target: &str) -> BTreeSet<&ModuleId> {
        self.find(target)
            .map(|node| {
                node.sources
                    .iter()
                    .filter(|source| source.backend == backend)
                    .map(|source| &source.module_id)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Every contributing module under a directory-level target, used by parent targets such as shallow overlay.
    pub fn module_ids_for_subtree(&self, backend: Mode, target: &str) -> BTreeSet<&ModuleId> {
        let mut ids = BTreeSet::new();
        if let Some(node) = self.find(target) {
            fn collect<'a>(node: &'a MountNode, backend: Mode, ids: &mut BTreeSet<&'a ModuleId>) {
                ids.extend(
                    node.sources
                        .iter()
                        .filter(|source| source.backend == backend)
                        .map(|source| &source.module_id),
                );
                for child in node.children.values() {
                    collect(child, backend, ids);
                }
            }
            collect(node, backend, &mut ids);
        }
        ids
    }

    pub fn has_backend(&self, backend: Mode) -> bool {
        self.root.has_backend(backend)
    }

    /// Total nodes in the shared tree, including the root, for boot performance counts.
    pub fn node_count(&self) -> usize {
        fn count(node: &MountNode) -> usize {
            1 + node.children.values().map(count).sum::<usize>()
        }
        count(&self.root)
    }

    pub fn module_ids_for(&self, backend: Mode) -> BTreeSet<&ModuleId> {
        fn collect<'a>(node: &'a MountNode, backend: Mode, ids: &mut BTreeSet<&'a ModuleId>) {
            ids.extend(
                node.sources
                    .iter()
                    .filter(|source| source.backend == backend)
                    .map(|source| &source.module_id),
            );
            for child in node.children.values() {
                collect(child, backend, ids);
            }
        }

        let mut ids = BTreeSet::new();
        collect(&self.root, backend, &mut ids);
        ids
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(module: &str, relative: &str, file_type: NodeFileType, backend: Mode) -> MountSource {
        crate::test_support::mount_source_at(
            module,
            relative,
            file_type,
            backend,
            PathBuf::from(format!("/modules/{module}/{relative}")),
        )
    }

    #[test]
    fn one_target_keeps_all_overlay_layers_in_module_order() {
        let mut tree = MountTree::default();
        tree.insert(
            "/system/etc/hosts",
            source(
                "z_mod",
                "system/etc/hosts",
                NodeFileType::RegularFile,
                Mode::Overlay,
            ),
        );
        tree.insert(
            "/system/etc/hosts",
            source(
                "a_mod",
                "system/etc/hosts",
                NodeFileType::RegularFile,
                Mode::Overlay,
            ),
        );

        let node = tree.find("/system/etc/hosts").unwrap();
        assert_eq!(node.sources[0].module_id, "a_mod");
        assert_eq!(node.sources[1].module_id, "z_mod");
        assert_eq!(
            node.file_type_for(Mode::Overlay),
            Some(NodeFileType::RegularFile)
        );
    }

    #[test]
    fn backend_descendant_keeps_shared_structural_ancestors() {
        let mut tree = MountTree::default();
        tree.insert(
            "/system/etc",
            source("base", "system/etc", NodeFileType::Directory, Mode::Ignore),
        );
        tree.insert(
            "/system/etc/hosts",
            source(
                "base",
                "system/etc/hosts",
                NodeFileType::Symlink,
                Mode::Magic,
            ),
        );

        let etc = tree.find("/system/etc").unwrap();
        assert!(etc.has_backend(Mode::Magic));
        assert_eq!(
            etc.file_type_for(Mode::Magic),
            Some(NodeFileType::Directory)
        );
        assert_eq!(
            etc.module_path_for(Mode::Magic),
            Some(Path::new("/modules/base/system/etc"))
        );
    }

    #[test]
    fn synthetic_partition_parent_gets_a_structural_source_path() {
        let mut tree = MountTree::default();
        tree.insert(
            "/system/new_dir/file",
            source(
                "base",
                "system/new_dir/file",
                NodeFileType::RegularFile,
                Mode::Magic,
            ),
        );

        assert_eq!(
            tree.find("/system").unwrap().module_path_for(Mode::Magic),
            Some(Path::new("/modules/base/system"))
        );
    }

    #[test]
    fn replace_and_whiteout_are_backend_annotated_node_data() {
        let mut tree = MountTree::default();
        let mut directory = source("m", "system/etc", NodeFileType::Directory, Mode::Overlay);
        directory.replace = true;
        tree.insert("/system/etc", directory);
        tree.insert(
            "/system/etc/removed",
            source(
                "m",
                "system/etc/removed",
                NodeFileType::Whiteout,
                Mode::Overlay,
            ),
        );

        assert!(tree.find("/system/etc").unwrap().replace_for(Mode::Overlay));
        assert_eq!(
            tree.find("/system/etc/removed")
                .unwrap()
                .file_type_for(Mode::Overlay),
            Some(NodeFileType::Whiteout)
        );
        assert_eq!(
            tree.module_ids_for(Mode::Overlay)
                .into_iter()
                .map(ModuleId::as_str)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["m"])
        );
    }

    #[test]
    fn target_module_ids_report_only_that_node_sources() {
        let mut tree = MountTree::default();
        tree.insert(
            "/system/etc",
            source(
                "alpha",
                "system/etc",
                NodeFileType::Directory,
                Mode::Overlay,
            ),
        );
        tree.insert(
            "/system/etc/hosts",
            source(
                "beta",
                "system/etc/hosts",
                NodeFileType::RegularFile,
                Mode::Overlay,
            ),
        );

        assert_eq!(
            tree.module_ids_for_target(Mode::Overlay, "/system/etc")
                .into_iter()
                .map(ModuleId::as_str)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["alpha"])
        );
        assert_eq!(
            tree.module_ids_for_target(Mode::Overlay, "/system/etc/hosts")
                .into_iter()
                .map(ModuleId::as_str)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["beta"])
        );
    }

    #[test]
    fn node_count_includes_root_and_all_descendants() {
        let mut tree = MountTree::default();
        assert_eq!(tree.node_count(), 1);
        tree.insert(
            "/system/etc/hosts",
            source(
                "m",
                "system/etc/hosts",
                NodeFileType::RegularFile,
                Mode::Overlay,
            ),
        );

        assert_eq!(tree.node_count(), 4);
    }

    #[test]
    fn subtree_module_ids_collect_backend_descendants() {
        let mut tree = MountTree::default();
        tree.insert(
            "/system/etc/a",
            source(
                "alpha",
                "system/etc/a",
                NodeFileType::RegularFile,
                Mode::Overlay,
            ),
        );
        tree.insert(
            "/system/etc/b",
            source(
                "beta",
                "system/etc/b",
                NodeFileType::RegularFile,
                Mode::Overlay,
            ),
        );

        assert_eq!(
            tree.module_ids_for_subtree(Mode::Overlay, "/system/etc")
                .into_iter()
                .map(ModuleId::as_str)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["alpha", "beta"])
        );
    }
}
