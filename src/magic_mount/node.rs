// ReHybrid-Mount
//
// SPDX-License-Identifier: GPL-3.0-only

//! Magic Mount 的 Node 树:纯数据 + 纯算法,不依赖平台。
//!
//! 语义对齐参考项目 `8b85c9e`:
//! - 同名冲突时,先收集模块的节点保留原属性,子节点继续向下合并;
//! - 目录只有自身 `replace` 或收集到文件时才参与挂载;
//! - 内建分区(vendor / system_ext / product / odm)按提升规则从
//!   `/system` 下提升到根,避免 Android 动态分区下的路径错位。
//!
//! Stage 1/2 脚手架:树 API 在 Stage 5 CLI 接入前暂未被二进制入口使用;
//! 接入完成后移除本豁免,恢复 dead_code 检查。
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fmt;
use std::fs::FileType;
use std::path::PathBuf;

use crate::defs;

/// Magic Mount 关心的四种文件类型。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NodeFileType {
    RegularFile,
    Directory,
    Symlink,
    Whiteout,
}

impl From<FileType> for NodeFileType {
    fn from(value: FileType) -> Self {
        if value.is_file() {
            Self::RegularFile
        } else if value.is_dir() {
            Self::Directory
        } else if value.is_symlink() {
            Self::Symlink
        } else {
            Self::Whiteout
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

/// 一颗挂载树的节点。`module_path` 是拥有该节点的模块源路径;
/// 根节点与分区提升后的父链没有 `module_path`。
#[derive(Clone)]
pub struct Node {
    pub name: String,
    pub file_type: NodeFileType,
    pub children: BTreeMap<String, Node>,
    /// 拥有该节点的模块源路径(只读,绝不写回模块目录)。
    pub module_path: Option<PathBuf>,
    pub replace: bool,
    pub skip: bool,
}

/// 扫描器生成的新节点描述,交给 [`Node::insert_child`] 合并。
pub struct NodeSource {
    pub name: String,
    pub file_type: NodeFileType,
    pub module_path: Option<PathBuf>,
    pub replace: bool,
    pub skip: bool,
}

impl From<NodeSource> for Node {
    fn from(source: NodeSource) -> Self {
        Self {
            name: source.name,
            file_type: source.file_type,
            children: BTreeMap::new(),
            module_path: source.module_path,
            replace: source.replace,
            skip: source.skip,
        }
    }
}

impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.write_tree(f, 0)
    }
}

impl Node {
    pub fn new_root(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            file_type: NodeFileType::Directory,
            children: BTreeMap::new(),
            module_path: None,
            replace: false,
            skip: false,
        }
    }

    /// 合并一个新节点:已存在时保留先到者的属性(参考项目 occupied 语义),
    /// 只返回可变引用供扫描器继续递归合并子目录。
    pub fn insert_child(&mut self, source: NodeSource) -> &mut Node {
        match self.children.entry(source.name.clone()) {
            std::collections::btree_map::Entry::Vacant(entry) => entry.insert(source.into()),
            std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
        }
    }

    fn write_tree(&self, f: &mut fmt::Formatter<'_>, indent: usize) -> fmt::Result {
        let pad = "  ".repeat(indent);

        write!(f, "{pad}{} ({})", self.name, self.file_type)?;
        if let Some(path) = &self.module_path {
            write!(f, " [{}]", path.display())?;
        }
        if self.replace {
            write!(f, " [REPLACE]")?;
        }
        if self.skip {
            write!(f, " [SKIP]")?;
        }
        writeln!(f)?;

        for child in self.children.values() {
            child.write_tree(f, indent + 1)?;
        }
        Ok(())
    }
}

/// 内建分区提升规则:`(分区名, 是否要求 /system/<分区> 是符号链接)`。
pub const BUILTIN_PARTITIONS: [(&str, bool); 4] = [
    ("vendor", true),
    ("system_ext", true),
    ("product", true),
    ("odm", false),
];

pub fn is_builtin_partition(partition: &str) -> bool {
    BUILTIN_PARTITIONS
        .iter()
        .any(|(name, _)| *name == partition)
}

/// 内建分区是否要求 `/system/<分区>` 为符号链接(参考项目行为)。
pub fn builtin_partition_requires_symlink(partition: &str) -> Option<bool> {
    BUILTIN_PARTITIONS
        .iter()
        .find(|(name, _)| *name == partition)
        .map(|(_, require_symlink)| *require_symlink)
}

/// 把 `/system/<分区>` 下的节点提升到根。路径存在性判断由扫描器完成,
/// 这里只做纯树操作,便于跨平台测试。
pub fn promote_partition(root: &mut Node, system: &mut Node, partition: &str) -> bool {
    if let Some(node) = system.children.remove(partition) {
        root.children.insert(partition.to_owned(), node);
        true
    } else {
        false
    }
}

/// 路径本身以 `.replace` 结尾(参考项目 skip 规则)。
pub fn is_replaced_path_suffix(path: &str) -> bool {
    path.ends_with(defs::REPLACE_DIR_FILE_NAME)
}

/// 路径命中自定义 ignore 列表(由 Stage 4 parser/planner 注入)。
pub fn is_ignored_source(path: &str, ignore_sources: &[String]) -> bool {
    ignore_sources.iter().any(|source| source == path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file_source(name: &str, module: &str) -> NodeSource {
        NodeSource {
            name: name.to_owned(),
            file_type: NodeFileType::RegularFile,
            module_path: Some(PathBuf::from(module)),
            replace: false,
            skip: false,
        }
    }

    #[test]
    fn file_type_display_names() {
        assert_eq!(NodeFileType::RegularFile.to_string(), "RegularFile");
        assert_eq!(NodeFileType::Directory.to_string(), "Directory");
        assert_eq!(NodeFileType::Symlink.to_string(), "Symlink");
        assert_eq!(NodeFileType::Whiteout.to_string(), "Whiteout");
    }

    #[test]
    fn insert_child_creates_new_node() {
        let mut root = Node::new_root("");
        let child = root.insert_child(file_source("etc", "/mod-a/system/etc"));

        assert_eq!(child.name, "etc");
        assert_eq!(child.file_type, NodeFileType::RegularFile);
        assert_eq!(child.module_path, Some(PathBuf::from("/mod-a/system/etc")));
    }

    #[test]
    fn insert_child_occupied_keeps_first_module_attributes() {
        let mut root = Node::new_root("");

        let first = NodeSource {
            name: "etc".to_owned(),
            file_type: NodeFileType::Directory,
            module_path: Some(PathBuf::from("/mod-a/system/etc")),
            replace: true,
            skip: false,
        };
        root.insert_child(first);

        let second = NodeSource {
            name: "etc".to_owned(),
            file_type: NodeFileType::Directory,
            module_path: Some(PathBuf::from("/mod-b/system/etc")),
            replace: false,
            skip: true,
        };
        let occupied = root.insert_child(second);

        assert_eq!(
            occupied.module_path,
            Some(PathBuf::from("/mod-a/system/etc"))
        );
        assert!(occupied.replace);
        assert!(!occupied.skip);
    }

    #[test]
    fn promote_partition_moves_system_child_to_root() {
        let mut root = Node::new_root("");
        let mut system = Node::new_root("system");
        system.insert_child(file_source("vendor", "/mod/system/vendor"));

        assert!(promote_partition(&mut root, &mut system, "vendor"));
        assert!(root.children.contains_key("vendor"));
        assert!(!system.children.contains_key("vendor"));
    }

    #[test]
    fn promote_partition_without_child_returns_false() {
        let mut root = Node::new_root("");
        let mut system = Node::new_root("system");

        assert!(!promote_partition(&mut root, &mut system, "vendor"));
        assert!(root.children.is_empty());
    }

    #[test]
    fn builtin_partition_rules_match_reference() {
        assert!(is_builtin_partition("vendor"));
        assert!(is_builtin_partition("system_ext"));
        assert!(is_builtin_partition("product"));
        assert!(is_builtin_partition("odm"));
        assert!(!is_builtin_partition("system"));

        assert_eq!(builtin_partition_requires_symlink("vendor"), Some(true));
        assert_eq!(builtin_partition_requires_symlink("odm"), Some(false));
        assert_eq!(builtin_partition_requires_symlink("system"), None);
    }

    #[test]
    fn replaced_suffix_and_ignore_source_rules() {
        assert!(is_replaced_path_suffix("/mod/system/etc.replace"));
        assert!(!is_replaced_path_suffix("/mod/system/etc"));

        let ignore = vec!["/mod/system/etc".to_owned()];
        assert!(is_ignored_source("/mod/system/etc", &ignore));
        assert!(!is_ignored_source("/mod/system/bin", &ignore));
    }

    #[test]
    fn debug_tree_prints_hierarchy() {
        let mut root = Node::new_root("");

        {
            let child = root.insert_child(file_source("hosts", "/mod/system/etc/hosts"));
            assert!(child.module_path.is_some());
        }

        let text = format!("{root:?}");
        assert!(text.contains(" (Directory)"));
        assert!(text.contains("hosts (RegularFile) [/mod/system/etc/hosts]"));
    }
}
