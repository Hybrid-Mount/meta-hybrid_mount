// SPDX-License-Identifier: GPL-3.0-only

//! 模块清单的只读扫描(供 planner 与 CLI 使用)。
//!
//! 只读取并记录,绝不写回模块目录。
//! 行为参考上游 `scanner.rs`(module.prop 必填字段、disable/remove/skip_mount、
//! system/额外分区存在性)。

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::{FileTypeExt, MetadataExt};

use crate::defs;
use crate::mount_tree::NodeFileType;
use crate::utils::validate_module_id;

/// 模块内一个可挂载条目:`relative` 相对模块根(如 `system/etc/hosts`),
/// 统一使用 `/` 分隔,便于跨平台比较。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleEntry {
    pub relative: String,
    pub file_type: NodeFileType,
    pub replace: bool,
}

/// 只读扫描出的模块记录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleRecord {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub disabled: bool,
    pub skip_mount: bool,
    pub has_mount_files: bool,
    pub source_path: PathBuf,
    pub entries: Vec<ModuleEntry>,
}

impl ModuleRecord {
    /// 是否参与挂载(供 planner 与 WebUI 模块列表使用)。
    pub fn mountable(&self) -> bool {
        self.has_mount_files && !self.disabled && !self.skip_mount
    }
}

/// 扫描模块目录,按 id 排序返回。
pub fn list_modules(module_dir: &Path, extra_partitions: &[String]) -> Vec<ModuleRecord> {
    let mut modules = Vec::new();

    let Ok(entries) = module_dir.read_dir() else {
        return modules;
    };

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                log::warn!(
                    "failed to read module directory entry in {}: {err}",
                    module_dir.display()
                );
                continue;
            }
        };
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            log::warn!(
                "failed to inspect module directory entry: {}",
                path.display()
            );
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }

        let prop_path = path.join(defs::MODULE_PROP_FILE_NAME);
        let Ok(prop_metadata) = fs::symlink_metadata(&prop_path) else {
            continue;
        };
        if !prop_metadata.file_type().is_file() {
            log::warn!("{} is not a regular module.prop file", prop_path.display());
            continue;
        }
        let Ok(prop_text) = fs::read_to_string(&prop_path) else {
            continue;
        };
        let prop = parse_prop(&prop_text);

        let Some(id) = prop.get("id") else {
            log::warn!("{} missing module id", path.display());
            continue;
        };
        let Some(name) = prop.get("name") else {
            log::warn!("{} missing module name", path.display());
            continue;
        };
        let Some(version) = prop.get("version") else {
            log::warn!("{} missing module version", path.display());
            continue;
        };
        let Some(author) = prop.get("author") else {
            log::warn!("{} missing module author", path.display());
            continue;
        };
        let Some(description) = prop.get("description") else {
            log::warn!("{} missing module description", path.display());
            continue;
        };

        if validate_module_id(id).is_err() {
            log::warn!("{} invalid module id: {id}", path.display());
            continue;
        }

        let disabled = path.join(defs::DISABLE_FILE_NAME).exists()
            || path.join(defs::REMOVE_FILE_NAME).exists();
        let skip_mount = path.join(defs::SKIP_MOUNT_FILE_NAME).exists();

        let mut has_mount_files = false;
        let mut entries = Vec::new();
        let mut partitions = BTreeSet::from(["system".to_owned()]);
        partitions.extend(
            extra_partitions
                .iter()
                .filter(|partition| partition.as_str() != "system")
                .cloned(),
        );
        for partition in partitions {
            let partition_dir = path.join(&partition);
            // `Path::is_dir` follows symlinks.  Magisk-style modules commonly
            // expose aliases such as `product -> ./system/product`; following
            // that alias here scans the same subtree once as `product/*` and
            // again as `system/product/*`.  Besides producing duplicate plan
            // sources, this can make the prepared OverlayFS tree much larger
            // than the ext4 size scan (which deliberately does not follow
            // symlinks).  Only real partition directories are scan roots;
            // promoted content under `system/<partition>` is already found by
            // the `system` walk and mapped to the correct target later.
            let Ok(metadata) = fs::symlink_metadata(&partition_dir) else {
                continue;
            };
            if !metadata.file_type().is_dir() {
                continue;
            }
            has_mount_files = true;
            entries.extend(collect_partition_entries(&partition_dir, &partition));
        }
        entries.sort_by(|left, right| left.relative.cmp(&right.relative));

        modules.push(ModuleRecord {
            id: id.clone(),
            name: name.clone(),
            version: version.clone(),
            author: author.clone(),
            description: description.clone(),
            disabled,
            skip_mount,
            has_mount_files,
            source_path: path,
            entries,
        });
    }

    modules.sort_by(|left, right| left.id.cmp(&right.id));
    modules
}

/// 轻量 `key=value` 解析:空行与 `#` 注释跳过,键值去首尾空白。
fn parse_prop(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let (key, value) = line.split_once('=')?;
            Some((key.trim().to_owned(), value.trim().to_owned()))
        })
        .collect()
}

fn collect_partition_entries(partition_dir: &Path, partition: &str) -> Vec<ModuleEntry> {
    fn walk(dir: &Path, root: &Path, partition: &str, out: &mut Vec<ModuleEntry>) {
        let Ok(entries) = dir.read_dir() else {
            return;
        };

        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(err) => {
                    log::warn!("failed to read module entry in {}: {err}", dir.display());
                    continue;
                }
            };
            let path = entry.path();
            if entry.file_name() == defs::REPLACE_DIR_FILE_NAME {
                continue;
            }
            let Ok(relative) = path.strip_prefix(root) else {
                continue;
            };
            let relative = relative
                .components()
                .filter_map(|component| component.as_os_str().to_str())
                .collect::<Vec<_>>()
                .join("/");
            if relative.is_empty() {
                continue;
            }

            let Ok(metadata) = fs::symlink_metadata(&path) else {
                continue;
            };
            let Some(file_type) = classify_file_type(&metadata) else {
                log::warn!("unsupported module entry type: {}", path.display());
                continue;
            };
            let replace = file_type == NodeFileType::Directory && is_replace_dir(&path);

            out.push(ModuleEntry {
                relative: format!("{partition}/{relative}"),
                file_type,
                replace,
            });

            if file_type == NodeFileType::Directory {
                walk(&path, root, partition, out);
            }
        }
    }

    let mut out = Vec::new();
    walk(partition_dir, partition_dir, partition, &mut out);
    out.sort_by(|left, right| left.relative.cmp(&right.relative));
    out
}

fn classify_file_type(metadata: &fs::Metadata) -> Option<NodeFileType> {
    #[cfg(unix)]
    if metadata.file_type().is_char_device() && metadata.rdev() == 0 {
        return Some(NodeFileType::Whiteout);
    }

    NodeFileType::from_file_type(metadata.file_type())
}

fn is_replace_dir(path: &Path) -> bool {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    if extattr::lgetxattr(path, defs::REPLACE_DIR_XATTR)
        .is_ok_and(|value| String::from_utf8_lossy(&value) == "y")
    {
        return true;
    }

    path.join(defs::REPLACE_DIR_FILE_NAME).exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn module_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("hybrid-mount-scanner-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_module(root: &Path, id: &str) -> PathBuf {
        let path = root.join(id);
        fs::create_dir_all(path.join("system/etc")).unwrap();
        fs::write(
            path.join("module.prop"),
            format!("id={id}\nname=N\nversion=1\nauthor=A\ndescription=D\n"),
        )
        .unwrap();
        fs::write(path.join("system/etc/hosts"), "127.0.0.1 localhost").unwrap();
        path
    }

    #[test]
    fn lists_modules_sorted_with_prop_and_entries() {
        let root = module_dir("list");
        write_module(&root, "b_mod");
        write_module(&root, "a_mod");

        let modules = list_modules(&root, &[]);

        assert_eq!(modules.len(), 2);
        assert_eq!(modules[0].id, "a_mod");
        assert_eq!(modules[1].id, "b_mod");
        assert!(modules[0].mountable());

        let hosts = modules[0]
            .entries
            .iter()
            .find(|entry| entry.relative == "system/etc/hosts")
            .unwrap();
        assert_eq!(hosts.file_type, NodeFileType::RegularFile);
        assert!(
            modules[0]
                .entries
                .iter()
                .any(|entry| entry.relative == "system/etc"
                    && entry.file_type == NodeFileType::Directory)
        );

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn disable_remove_and_skip_mount_mark_module() {
        let root = module_dir("state");
        let path = write_module(&root, "off");
        fs::write(path.join("disable"), "").unwrap();
        let modules = list_modules(&root, &[]);
        assert!(modules[0].disabled);
        assert!(!modules[0].mountable());

        fs::remove_file(path.join("disable")).unwrap();
        fs::write(path.join("skip_mount"), "").unwrap();
        let modules = list_modules(&root, &[]);
        assert!(modules[0].skip_mount);
        assert!(!modules[0].mountable());

        fs::remove_file(path.join("skip_mount")).unwrap();
        fs::write(path.join("remove"), "").unwrap();
        let modules = list_modules(&root, &[]);
        assert!(modules[0].disabled);

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn module_without_system_or_extra_partition_has_no_mount_files() {
        let root = module_dir("empty");
        let path = root.join("plain");
        fs::create_dir_all(&path).unwrap();
        fs::write(
            path.join("module.prop"),
            "id=plain\nname=N\nversion=1\nauthor=A\ndescription=D\n",
        )
        .unwrap();

        let modules = list_modules(&root, &[]);
        assert!(!modules[0].has_mount_files);

        fs::create_dir_all(path.join("product")).unwrap();
        let modules = list_modules(&root, &["product".to_owned()]);
        assert!(modules[0].has_mount_files);

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn missing_required_prop_field_skips_module() {
        let root = module_dir("prop");
        let path = root.join("bad");
        fs::create_dir_all(path.join("system")).unwrap();
        fs::write(path.join("module.prop"), "id=bad\n").unwrap();

        assert!(list_modules(&root, &[]).is_empty());
        fs::remove_dir_all(&root).ok();
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_module_directory_and_prop_are_not_scanned() {
        use std::os::unix::fs::symlink;

        let root = module_dir("symlink-module");
        let outside = module_dir("symlink-module-outside");
        let real_module = write_module(&outside, "real");
        symlink(&real_module, root.join("linked_module")).unwrap();

        let linked_prop = root.join("linked_prop");
        fs::create_dir_all(linked_prop.join("system")).unwrap();
        symlink(
            real_module.join("module.prop"),
            linked_prop.join("module.prop"),
        )
        .unwrap();

        assert!(list_modules(&root, &[]).is_empty());

        fs::remove_dir_all(&root).ok();
        fs::remove_dir_all(&outside).ok();
    }

    #[test]
    fn extra_partitions_are_included_with_their_root_name() {
        let root = module_dir("extra");
        let path = write_module(&root, "x");
        fs::create_dir_all(path.join("product/app")).unwrap();
        fs::write(path.join("product/app/x.apk"), "x").unwrap();

        let modules = list_modules(&root, &["product".to_owned()]);
        assert!(modules[0].has_mount_files);
        assert!(
            modules[0]
                .entries
                .iter()
                .any(|entry| entry.relative == "product/app/x.apk")
        );

        fs::remove_dir_all(&root).ok();
    }

    #[cfg(unix)]
    #[test]
    fn partition_root_symlink_does_not_duplicate_promoted_system_subtree() {
        use std::os::unix::fs::symlink;

        let root = module_dir("partition-alias");
        let path = write_module(&root, "alias");
        fs::create_dir_all(path.join("system/product/media")).unwrap();
        fs::write(
            path.join("system/product/media/bootanimation.zip"),
            "payload",
        )
        .unwrap();
        symlink("./system/product", path.join("product")).unwrap();

        let modules = list_modules(&root, &["product".to_owned()]);
        let entries = &modules[0].entries;

        assert!(
            entries
                .iter()
                .any(|entry| entry.relative == "system/product/media/bootanimation.zip")
        );
        assert!(
            entries
                .iter()
                .all(|entry| !entry.relative.starts_with("product/"))
        );
        assert_eq!(
            entries
                .iter()
                .filter(|entry| entry.file_type == NodeFileType::RegularFile)
                .count(),
            2
        );

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn replace_marker_is_directory_metadata_not_a_mount_node() {
        let root = module_dir("replace");
        let path = write_module(&root, "replace_mod");
        fs::write(path.join("system/etc/.replace"), "").unwrap();

        let modules = list_modules(&root, &[]);
        let etc = modules[0]
            .entries
            .iter()
            .find(|entry| entry.relative == "system/etc")
            .unwrap();

        assert!(etc.replace);
        assert!(
            modules[0]
                .entries
                .iter()
                .all(|entry| entry.relative != "system/etc/.replace")
        );

        fs::remove_dir_all(&root).ok();
    }
}
