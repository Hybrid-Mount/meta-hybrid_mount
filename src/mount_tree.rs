// SPDX-License-Identifier: GPL-3.0-only

//! OverlayFS 与 Magic Mount 共享的挂载树。
//!
//! scanner 只读识别节点类型与 `.replace` / whiteout 语义，planner 再把每个
//! 模块贡献标注为 overlay、magic 或 ignore。执行阶段只消费这棵树，不再重新
//! 扫描模块目录或维护第二套路径过滤协议。

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::FileType;
use std::path::{Path, PathBuf};

use crate::config::Mode;

/// 内建分区提升规则:`(分区名, 是否要求 /system/<分区> 是符号链接)`。
pub const BUILTIN_PARTITIONS: [(&str, bool); 4] = [
    ("vendor", true),
    ("system_ext", true),
    ("product", true),
    ("odm", false),
];

/// 两个挂载后端共同关心的节点类型。
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

/// 一个模块对目标节点的贡献。`source_path` 始终指向只读模块源目录。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MountSource {
    pub module_id: String,
    pub relative: String,
    pub source_path: PathBuf,
    pub file_type: NodeFileType,
    pub replace: bool,
    pub backend: Mode,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StructuralSource {
    pub module_id: String,
    pub source_path: PathBuf,
}

/// 以真实挂载目标为层级的一棵树。一个目标可以有多个同后端模块层。
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

    /// 当前目标上该后端的优先贡献。排序与 Overlay lowerdir 顺序一致。
    pub fn source_for(&self, backend: Mode) -> Option<&MountSource> {
        self.sources.iter().find(|source| source.backend == backend)
    }

    /// 仅用于承载后端子节点的目录来源，不会把该来源本身切换到目标后端。
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

    /// 执行器看到的节点类型。没有自身贡献、但有该后端子孙时是结构目录。
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

    /// 后端自己的来源优先；结构目录可借用任意模块目录的元数据来建 staging。
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

    #[cfg(test)]
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

    pub fn has_backend(&self, backend: Mode) -> bool {
        self.root.has_backend(backend)
    }

    pub fn module_ids_for(&self, backend: Mode) -> BTreeSet<&str> {
        fn collect<'a>(node: &'a MountNode, backend: Mode, ids: &mut BTreeSet<&'a str>) {
            ids.extend(
                node.sources
                    .iter()
                    .filter(|source| source.backend == backend)
                    .map(|source| source.module_id.as_str()),
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
        MountSource {
            module_id: module.to_owned(),
            relative: relative.to_owned(),
            source_path: PathBuf::from(format!("/modules/{module}/{relative}")),
            file_type,
            replace: false,
            backend,
        }
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
        assert_eq!(tree.module_ids_for(Mode::Overlay), BTreeSet::from(["m"]));
    }
}
