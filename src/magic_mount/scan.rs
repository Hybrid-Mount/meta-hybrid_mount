// SPDX-License-Identifier: GPL-3.0-only

//! 模块只读扫描:module.prop 判定、状态标记、system 目录收集与分区提升。
//!
//! 这里只读取并建树,绝不移动、合并、删除或写回模块源目录。
//! 行为对齐参考项目 `8b85c9e` 的 `collect_module_files`。

use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::Path;

use crate::defs;
use crate::errors::Result;
use crate::magic_mount::node::{
    BUILTIN_PARTITIONS, Node, NodeFileType, NodeSource, is_builtin_partition, is_ignored_source,
    is_replaced_path_suffix, promote_partition,
};
use crate::utils::validate_module_id;

/// 路径级过滤回调:`(module_id, 源路径) -> 是否收集`。
pub type PathFilter<'a> = dyn Fn(&str, &Path) -> bool + 'a;

/// 混合策略扩展点:只收集 planner 判定为 magic 的模块/路径。
///
/// - `modules: None` 表示不过滤模块(参考项目行为);
/// - `path_filter` 在每个源路径上调用,返回 `false` 的路径不进入树。
#[derive(Default)]
pub struct Selection<'a> {
    pub modules: Option<&'a BTreeSet<String>>,
    pub path_filter: Option<&'a PathFilter<'a>>,
}

/// 一次扫描的全部输入。
#[derive(Default)]
pub struct ScanOptions<'a> {
    /// 额外分区(参考项目 extra partitions,内建分区之外的提升目标)。
    pub extra_partitions: &'a [String],
    /// 自定义 ignore 列表(parser 产物;命中路径不挂载)。
    pub ignore_sources: &'a [String],
    /// 混合 planner 注入的选择器。
    pub selection: Selection<'a>,
}

/// 收集模块目录下的 magic mount 树;没有任何文件时返回 `None`。
pub fn collect_module_files(module_dir: &Path, options: &ScanOptions<'_>) -> Result<Option<Node>> {
    let mut root = Node::new_root("");
    let mut system = Node::new_root("system");
    let mut has_file = false;
    let direct_partitions: BTreeSet<String> = BUILTIN_PARTITIONS
        .iter()
        .map(|(partition, _)| (*partition).to_owned())
        .chain(
            options
                .extra_partitions
                .iter()
                .filter(|partition| partition.as_str() != "system")
                .cloned(),
        )
        .collect();

    log::debug!("begin collecting module files: {}", module_dir.display());

    for entry in module_dir.read_dir()?.flatten() {
        if !entry.file_type()?.is_dir() {
            continue;
        }

        let id = entry.file_name().to_string_lossy().to_string();
        log::debug!("processing module: {id}");

        let prop = entry.path().join(defs::MODULE_PROP_FILE_NAME);
        if !prop.exists() {
            log::debug!("skipped module {id}: missing module.prop");
            continue;
        }

        let prop_text = fs::read_to_string(prop)?;
        for line in prop_text.lines() {
            if line.starts_with("id")
                && let Some((_, value)) = line.split_once('=')
            {
                validate_module_id(value)?;
            }
        }

        let module_path = entry.path();
        if module_path.join(defs::DISABLE_FILE_NAME).exists()
            || module_path.join(defs::REMOVE_FILE_NAME).exists()
            || module_path.join(defs::SKIP_MOUNT_FILE_NAME).exists()
        {
            log::debug!("skipped module {id}: disabled / remove / skip_mount");
            continue;
        }

        if let Some(selected) = options.selection.modules
            && !selected.contains(&id)
        {
            log::debug!("skipped module {id}: not selected by planner");
            continue;
        }

        let module_system = module_path.join("system");
        let mut found_mount_root = false;
        if module_system.is_dir() {
            log::debug!("collecting {}", module_system.display());
            has_file |= collect_dir(&mut system, &module_system, &id, options)?;
            found_mount_root = true;
        }

        for partition in &direct_partitions {
            let partition_path = module_path.join(partition);
            if !partition_path.is_dir() {
                continue;
            }
            log::debug!(
                "collecting direct partition: module={id}, partition={partition}, path={}",
                partition_path.display()
            );
            let replace = is_replace_dir(&partition_path);
            let child = root.insert_child(NodeSource {
                name: partition.clone(),
                file_type: NodeFileType::Directory,
                module_path: Some(partition_path.clone()),
                replace,
                skip: false,
            });
            has_file |= collect_dir(child, &partition_path, &id, options)? || child.replace;
            found_mount_root = true;
        }

        if !found_mount_root {
            log::debug!("skipped module {id}: no managed partition directory");
        }
    }

    if !has_file {
        return Ok(None);
    }

    for (partition, require_symlink) in BUILTIN_PARTITIONS {
        if root.children.contains_key(partition) {
            system.children.remove(partition);
            continue;
        }
        let root_partition = Path::new("/").join(partition);
        let system_partition = Path::new("/system").join(partition);
        if root_partition.is_dir() && (!require_symlink || system_partition.is_symlink()) {
            let _ = promote_partition(&mut root, &mut system, partition);
        }
    }

    for partition in options.extra_partitions {
        if is_builtin_partition(partition) || partition == "system" {
            continue;
        }
        if root.children.contains_key(partition) {
            system.children.remove(partition);
            continue;
        }
        if Path::new("/").join(partition).is_dir()
            && promote_partition(&mut root, &mut system, partition)
        {
            log::debug!("attached extra partition '{partition}' to root");
        }
    }

    root.children.insert("system".to_owned(), system);
    Ok(Some(root))
}

/// 递归收集一个模块源目录;返回该目录是否贡献了挂载内容。
fn collect_dir(
    parent: &mut Node,
    dir: &Path,
    module_id: &str,
    options: &ScanOptions<'_>,
) -> Result<bool> {
    let mut has_file = false;

    for entry in dir.read_dir()?.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();

        if let Some(path_filter) = options.selection.path_filter
            && !path_filter(module_id, &entry.path())
        {
            log::debug!("path filtered out by planner: {}", entry.path().display());
            continue;
        }

        let Ok(metadata) = entry.metadata() else {
            continue;
        };

        let path = entry.path();
        let file_type = if metadata.file_type().is_char_device() && metadata.rdev() == 0 {
            NodeFileType::Whiteout
        } else {
            NodeFileType::from(metadata.file_type())
        };
        let replace = file_type == NodeFileType::Directory && is_replace_dir(&path);
        let skip = is_skip_path(&path, options.ignore_sources);

        if replace {
            log::debug!("{} needs replace", path.display());
        }
        if skip {
            log::debug!("{} was skipped", path.display());
        }

        let child = parent.insert_child(NodeSource {
            name: name.clone(),
            file_type,
            module_path: Some(path.clone()),
            replace,
            skip,
        });

        let contributes = if child.file_type == NodeFileType::Directory {
            collect_dir(child, &path, module_id, options)? || child.replace
        } else {
            true
        };
        has_file |= contributes;
    }

    Ok(has_file)
}

/// 目录替换标记:xattr 为 `y`,或目录内含 `.replace` 标记文件。
fn is_replace_dir(path: &Path) -> bool {
    let xattr_says_replace = extattr::lgetxattr(path, defs::REPLACE_DIR_XATTR)
        .is_ok_and(|value| String::from_utf8_lossy(&value) == "y");

    xattr_says_replace || path.join(defs::REPLACE_DIR_FILE_NAME).exists()
}

/// 跳过规则:命中自定义 ignore 列表,或路径本身以 `.replace` 结尾。
fn is_skip_path(path: &Path, ignore_sources: &[String]) -> bool {
    let source = path.to_string_lossy();
    is_ignored_source(&source, ignore_sources) || is_replaced_path_suffix(&source)
}
