// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    collections::HashSet,
    fs,
    os::unix::fs::{FileTypeExt, MetadataExt},
    path::{Path, PathBuf},
};
#[cfg(test)]
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    path::Component,
};

use anyhow::{Context, Result, bail};
use walkdir::WalkDir;

use super::common::build_managed_partitions;
use crate::{
    conf::config,
    core::{
        inventory::{Module, MountMode},
        ops::planner::{HymofsAddRule, HymofsMergeRule, MountPlan},
    },
    defs,
};

#[derive(Debug, Default)]
pub(super) struct CompiledRules {
    pub(super) add_rules: Vec<HymofsAddRule>,
    pub(super) merge_rules: Vec<HymofsMergeRule>,
    pub(super) hide_rules: Vec<String>,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HymofsTreeFileType {
    Root,
    Directory,
    RegularFile,
    Symlink,
    BlockDevice,
    CharDevice,
    Fifo,
    Socket,
    Hidden,
    Unknown,
}

#[cfg(test)]
impl HymofsTreeFileType {
    fn as_str(self) -> &'static str {
        match self {
            Self::Root => "Root",
            Self::Directory => "Directory",
            Self::RegularFile => "RegularFile",
            Self::Symlink => "Symlink",
            Self::BlockDevice => "BlockDevice",
            Self::CharDevice => "CharDevice",
            Self::Fifo => "Fifo",
            Self::Socket => "Socket",
            Self::Hidden => "Hidden",
            Self::Unknown => "Unknown",
        }
    }
}

#[cfg(test)]
#[derive(Clone)]
struct HymofsTreeNode {
    name: String,
    file_type: HymofsTreeFileType,
    children: BTreeMap<String, Self>,
    actions: BTreeSet<&'static str>,
    modules: BTreeSet<String>,
    sources: Vec<PathBuf>,
}

#[cfg(test)]
impl fmt::Debug for HymofsTreeNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.debug_tree(f, 0)
    }
}

#[cfg(test)]
impl HymofsTreeNode {
    fn new_root() -> Self {
        Self {
            name: "/".to_string(),
            file_type: HymofsTreeFileType::Root,
            children: BTreeMap::default(),
            actions: BTreeSet::default(),
            modules: BTreeSet::default(),
            sources: Vec::new(),
        }
    }

    fn new_directory<S>(name: S) -> Self
    where
        S: Into<String>,
    {
        Self {
            name: name.into(),
            file_type: HymofsTreeFileType::Directory,
            children: BTreeMap::default(),
            actions: BTreeSet::default(),
            modules: BTreeSet::default(),
            sources: Vec::new(),
        }
    }

    fn debug_tree(&self, f: &mut fmt::Formatter<'_>, indent: usize) -> fmt::Result {
        let indent_str = "  ".repeat(indent);

        write!(
            f,
            "{}{} ({})",
            indent_str,
            self.name,
            self.file_type.as_str()
        )?;

        for action in &self.actions {
            write!(f, " [{}]", action)?;
        }

        if !self.modules.is_empty() {
            write!(
                f,
                " [modules={}]",
                self.modules.iter().cloned().collect::<Vec<_>>().join(",")
            )?;
        }

        if !self.sources.is_empty() {
            write!(
                f,
                " [sources={}]",
                self.sources
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            )?;
        }

        writeln!(f)?;

        for child in self.children.values() {
            child.debug_tree(f, indent + 1)?;
        }

        Ok(())
    }

    fn insert_rule(
        &mut self,
        target: &Path,
        leaf_file_type: HymofsTreeFileType,
        action: &'static str,
        source: Option<&Path>,
        module_id: Option<String>,
    ) {
        let components: Vec<String> = target
            .components()
            .filter_map(|component| match component {
                Component::Normal(value) => Some(value.to_string_lossy().into_owned()),
                _ => None,
            })
            .collect();

        if components.is_empty() {
            self.actions.insert(action);
            if let Some(module_id) = module_id {
                self.modules.insert(module_id);
            }
            if let Some(source) = source
                && !self.sources.iter().any(|existing| existing == source)
            {
                self.sources.push(source.to_path_buf());
            }
            return;
        }

        let mut current = self;
        for (index, component) in components.iter().enumerate() {
            let is_leaf = index == components.len() - 1;
            current = current
                .children
                .entry(component.clone())
                .or_insert_with(|| Self::new_directory(component.clone()));

            if !is_leaf {
                current.file_type = HymofsTreeFileType::Directory;
                continue;
            }

            if current.children.is_empty() {
                current.file_type = leaf_file_type;
            } else {
                current.file_type = HymofsTreeFileType::Directory;
            }
            current.actions.insert(action);
            if let Some(module_id) = module_id.as_ref() {
                current.modules.insert(module_id.clone());
            }
            if let Some(source) = source
                && !current.sources.iter().any(|existing| existing == source)
            {
                current.sources.push(source.to_path_buf());
            }
        }
    }
}

fn resolve_path_for_hymofs_with_root(system_root: &Path, path: &Path) -> PathBuf {
    let virtual_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        Path::new("/").join(path)
    };

    let translated_path = if system_root == Path::new("/") {
        virtual_path.clone()
    } else {
        let relative = virtual_path.strip_prefix("/").unwrap_or(&virtual_path);
        system_root.join(relative)
    };

    let Some(parent) = translated_path.parent() else {
        return virtual_path;
    };

    let Some(filename) = translated_path.file_name() else {
        return virtual_path;
    };

    let mut current = parent.to_path_buf();
    let mut suffix = Vec::new();

    while current != system_root && !current.exists() {
        if let Some(name) = current.file_name() {
            suffix.push(name.to_os_string());
        }
        if !current.pop() {
            break;
        }
    }

    let mut resolved = if current.exists() {
        current
    } else {
        parent.to_path_buf()
    };

    for item in suffix.iter().rev() {
        resolved.push(item);
    }
    resolved.push(filename);

    if system_root == Path::new("/") {
        return resolved;
    }

    if let Ok(relative) = resolved.strip_prefix(system_root) {
        return Path::new("/").join(relative);
    }

    virtual_path
}

fn mirror_module_root(config: &config::Config, module: &Module) -> Result<PathBuf> {
    let module_root = config.hymofs.mirror_path.join(&module.id);
    if module_root.exists() {
        Ok(module_root)
    } else {
        bail!(
            "missing HymoFS mirror content for module {} at {}",
            module.id,
            module_root.display()
        )
    }
}

fn build_dtype(path: &Path) -> Result<(i32, bool)> {
    let metadata = fs::symlink_metadata(path).with_context(|| {
        format!(
            "failed to read metadata for hymofs source {}",
            path.display()
        )
    })?;
    let file_type = metadata.file_type();

    if file_type.is_char_device() && metadata.rdev() == 0 {
        return Ok((libc::DT_UNKNOWN as i32, true));
    }

    let d_type = if file_type.is_file() {
        libc::DT_REG as i32
    } else if file_type.is_symlink() {
        libc::DT_LNK as i32
    } else if file_type.is_dir() {
        libc::DT_DIR as i32
    } else if file_type.is_block_device() {
        libc::DT_BLK as i32
    } else if file_type.is_char_device() {
        libc::DT_CHR as i32
    } else if file_type.is_fifo() {
        libc::DT_FIFO as i32
    } else if file_type.is_socket() {
        libc::DT_SOCK as i32
    } else {
        libc::DT_UNKNOWN as i32
    };

    Ok((d_type, false))
}

#[cfg(test)]
fn tree_file_type_from_dtype(d_type: i32) -> HymofsTreeFileType {
    match d_type {
        x if x == libc::DT_DIR as i32 => HymofsTreeFileType::Directory,
        x if x == libc::DT_REG as i32 => HymofsTreeFileType::RegularFile,
        x if x == libc::DT_LNK as i32 => HymofsTreeFileType::Symlink,
        x if x == libc::DT_BLK as i32 => HymofsTreeFileType::BlockDevice,
        x if x == libc::DT_CHR as i32 => HymofsTreeFileType::CharDevice,
        x if x == libc::DT_FIFO as i32 => HymofsTreeFileType::Fifo,
        x if x == libc::DT_SOCK as i32 => HymofsTreeFileType::Socket,
        _ => HymofsTreeFileType::Unknown,
    }
}

#[cfg(test)]
fn extract_module_id_from_source(source: &Path, mirror_path: &Path) -> Option<String> {
    source
        .strip_prefix(Path::new(defs::MODULES_DIR))
        .ok()
        .or_else(|| source.strip_prefix(mirror_path).ok())
        .and_then(|relative| {
            relative.components().find_map(|component| match component {
                Component::Normal(value) => Some(value.to_string_lossy().into_owned()),
                _ => None,
            })
        })
}

pub(super) fn log_compiled_rule_summary(compiled: &CompiledRules, user_hide_paths: &[PathBuf]) {
    crate::scoped_log!(
        debug,
        "mount:hymofs",
        "compiled rules: add_rules={}, merge_rules={}, hide_rules={}, user_hide_rules={}",
        compiled.add_rules.len(),
        compiled.merge_rules.len(),
        compiled.hide_rules.len(),
        user_hide_paths.len()
    );
}

fn relative_mode(module: &Module, relative: &Path) -> MountMode {
    let relative_str = relative.to_string_lossy();
    module.rules.get_mode(relative_str.as_ref())
}

pub(super) fn compile_rules(
    modules: &[Module],
    plan: &MountPlan,
    config: &config::Config,
) -> Result<CompiledRules> {
    compile_rules_with_root(modules, plan, config, Path::new("/"))
}

pub(super) fn compile_rules_with_root(
    modules: &[Module],
    plan: &MountPlan,
    config: &config::Config,
    system_root: &Path,
) -> Result<CompiledRules> {
    let managed_partitions = build_managed_partitions(config);
    let active_ids: HashSet<&str> = plan.hymofs_module_ids.iter().map(String::as_str).collect();
    let mut compiled = CompiledRules::default();
    let mut managed_partition_list: Vec<String> = managed_partitions.into_iter().collect();
    managed_partition_list.sort();

    for module in modules.iter().rev() {
        if !active_ids.contains(module.id.as_str()) {
            continue;
        }

        let module_root = mirror_module_root(config, module)?;
        let mut scanned_partition_roots: HashSet<PathBuf> = HashSet::new();

        for partition_name in &managed_partition_list {
            let partition_root = module_root.join(partition_name);
            if !partition_root.is_dir() {
                continue;
            }
            let normalized_partition_root =
                fs::canonicalize(&partition_root).unwrap_or_else(|_| partition_root.clone());
            if !scanned_partition_roots.insert(normalized_partition_root) {
                crate::scoped_log!(
                    debug,
                    "mount:hymofs",
                    "partition root dedupe: module={}, partition={}, root={}",
                    module.id,
                    partition_name,
                    partition_root.display()
                );
                continue;
            }

            let mut iterator = WalkDir::new(&partition_root)
                .follow_links(false)
                .into_iter();

            while let Some(entry_result) = iterator.next() {
                let entry = match entry_result {
                    Ok(entry) => entry,
                    Err(err) => {
                        crate::scoped_log!(
                            warn,
                            "mount:hymofs",
                            "walk failed: module={}, partition={}, error={}",
                            module.id,
                            partition_name,
                            err
                        );
                        continue;
                    }
                };

                if entry.depth() == 0 {
                    continue;
                }

                let path = entry.path();
                let relative = match path.strip_prefix(&module_root) {
                    Ok(relative) => relative,
                    Err(err) => {
                        crate::scoped_log!(
                            warn,
                            "mount:hymofs",
                            "relative path failed: module={}, path={}, error={}",
                            module.id,
                            path.display(),
                            err
                        );
                        continue;
                    }
                };

                if !matches!(relative_mode(module, relative), MountMode::Hymofs) {
                    continue;
                }

                if path
                    .file_name()
                    .is_some_and(|name| name == defs::REPLACE_DIR_FILE_NAME)
                {
                    continue;
                }

                let resolved_virtual_path =
                    resolve_path_for_hymofs_with_root(system_root, &Path::new("/").join(relative));
                let target_key = resolved_virtual_path.display().to_string();

                if entry.file_type().is_dir() {
                    if resolved_virtual_path.is_dir() {
                        compiled.merge_rules.push(HymofsMergeRule {
                            target: target_key,
                            source: path.to_path_buf(),
                        });
                        iterator.skip_current_dir();
                    }
                    continue;
                }

                if entry.file_type().is_symlink()
                    && resolved_virtual_path.exists()
                    && resolved_virtual_path.is_dir()
                {
                    crate::scoped_log!(
                        warn,
                        "mount:hymofs",
                        "symlink skip: module={}, path={}, reason=directory_target",
                        module.id,
                        resolved_virtual_path.display()
                    );
                    continue;
                }

                let (file_type, hide_only) = build_dtype(path)?;
                if hide_only {
                    compiled.hide_rules.push(target_key);
                    continue;
                }

                compiled.add_rules.push(HymofsAddRule {
                    target: target_key,
                    source: path.to_path_buf(),
                    file_type,
                });
            }
        }
    }

    Ok(compiled)
}

#[cfg(test)]
pub(super) fn render_compiled_tree(
    compiled: &CompiledRules,
    mirror_path: &Path,
    user_hide_paths: &[PathBuf],
) -> Option<String> {
    if compiled.add_rules.is_empty()
        && compiled.merge_rules.is_empty()
        && compiled.hide_rules.is_empty()
        && user_hide_paths.is_empty()
    {
        return None;
    }

    let mut root = HymofsTreeNode::new_root();

    for rule in &compiled.merge_rules {
        root.insert_rule(
            Path::new(&rule.target),
            HymofsTreeFileType::Directory,
            "MERGE",
            Some(&rule.source),
            extract_module_id_from_source(&rule.source, mirror_path),
        );
    }

    for rule in &compiled.add_rules {
        root.insert_rule(
            Path::new(&rule.target),
            tree_file_type_from_dtype(rule.file_type),
            "ADD",
            Some(&rule.source),
            extract_module_id_from_source(&rule.source, mirror_path),
        );
    }

    for path in &compiled.hide_rules {
        root.insert_rule(
            Path::new(path),
            HymofsTreeFileType::Hidden,
            "HIDE",
            None,
            None,
        );
    }

    for path in user_hide_paths {
        root.insert_rule(path, HymofsTreeFileType::Hidden, "USER_HIDE", None, None);
    }

    Some(format!("{root:?}"))
}
