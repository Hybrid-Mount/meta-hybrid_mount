// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::{
    collections::{HashSet, btree_map::Entry},
    fs::{self, DirEntry, Metadata, create_dir, create_dir_all, read_link},
    os::unix::fs::{MetadataExt, symlink},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use rustix::fs::{Gid, Mode, Uid, chmod, chown};
#[cfg(any(target_os = "linux", target_os = "android"))]
pub(super) use rustix::mount::mount_bind;

use crate::{
    core::inventory::{self, Module},
    domain::{ModuleRules, MountMode},
    mount::node::Node,
    sys::fs::{lgetfilecon, lsetfilecon},
};

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub(super) fn mount_bind<P, Q>(_from: P, _to: Q) -> Result<()>
where
    P: AsRef<Path>,
    Q: AsRef<Path>,
{
    bail!("bind mount is only supported on linux/android")
}

fn metadata_path<P>(path: P, node: &Node) -> Result<(Metadata, PathBuf)>
where
    P: AsRef<Path>,
{
    let path = path.as_ref();
    if path.exists() {
        Ok((path.metadata()?, path.to_path_buf()))
    } else if let Some(module_path) = &node.module_path {
        Ok((module_path.metadata()?, module_path.clone()))
    } else {
        bail!("cannot mount root dir {}!", path.display());
    }
}

fn copy_metadata(src: &Path, dst: &Path, metadata: &Metadata) -> Result<()> {
    chmod(dst, Mode::from_raw_mode(metadata.mode() as _))?;
    chown(
        dst,
        Some(Uid::from_raw(metadata.uid())),
        Some(Gid::from_raw(metadata.gid())),
    )?;
    lsetfilecon(dst, lgetfilecon(src)?.as_str())
}

pub fn tmpfs_skeleton<P>(path: P, work_dir_path: P, node: &Node) -> Result<()>
where
    P: AsRef<Path>,
{
    let (path, work_dir_path) = (path.as_ref(), work_dir_path.as_ref());
    crate::scoped_log!(
        debug,
        "magic:collect",
        "tmpfs skeleton: src={}, dst={}",
        path.display(),
        work_dir_path.display()
    );

    create_dir_all(work_dir_path)?;

    let (metadata, path) = metadata_path(path, node)?;
    copy_metadata(&path, work_dir_path, &metadata)?;
    Ok(())
}

pub fn mount_mirror<P>(path: P, work_dir_path: P, entry: &DirEntry) -> Result<()>
where
    P: AsRef<Path>,
{
    let path = path.as_ref().join(entry.file_name());
    let work_dir_path = work_dir_path.as_ref().join(entry.file_name());
    let file_type = entry.file_type()?;

    if file_type.is_file() {
        crate::scoped_log!(
            debug,
            "magic:collect",
            "mirror file: src={}, dst={}",
            path.display(),
            work_dir_path.display()
        );
        fs::File::create(&work_dir_path)?;
        mount_bind(&path, &work_dir_path)?;
    } else if file_type.is_dir() {
        crate::scoped_log!(
            debug,
            "magic:collect",
            "mirror dir: src={}, dst={}",
            path.display(),
            work_dir_path.display()
        );
        create_dir(&work_dir_path)?;
        copy_metadata(&path, &work_dir_path, &entry.metadata()?)?;
        for entry_result in path.read_dir()? {
            let entry = entry_result
                .with_context(|| format!("failed to enumerate mirror path {}", path.display()))?;
            mount_mirror(&path, &work_dir_path, &entry)?;
        }
    } else if file_type.is_symlink() {
        clone_symlink(&path, &work_dir_path)?;
    }

    Ok(())
}

fn collect_magic_subtree(
    target: &mut Node,
    module_dir: &Path,
    relative_path: &Path,
    rules: &ModuleRules,
    descendant_rule_prefixes: &HashSet<String>,
) -> Result<bool> {
    let mut has_file = false;

    for entry_result in module_dir.read_dir()? {
        let entry = entry_result?;

        let file_name = entry.file_name();
        let name = file_name
            .to_str()
            .with_context(|| format!("non-UTF-8 entry under {}", module_dir.display()))?
            .to_owned();
        let entry_path = entry.path();
        let next_relative = relative_path.join(&file_name);
        let next_relative_key = next_relative.to_string_lossy();
        let effective_mode = rules.effective_mode(&next_relative);

        match entry.file_type() {
            Ok(file_type) if file_type.is_dir() => {
                let has_descendant_rules =
                    descendant_rule_prefixes.contains(next_relative_key.as_ref());
                if matches!(effective_mode, MountMode::Magic) && !has_descendant_rules {
                    let mut node = Node::new_module(&name, &entry)?;
                    let subtree_has_file = node.collect_module_files(&entry_path)? || node.replace;
                    if subtree_has_file {
                        target.children.insert(name, node);
                        has_file = true;
                    }
                    continue;
                }

                if !has_descendant_rules {
                    continue;
                }

                let mut node = Node::new_module(&name, &entry)?;
                let subtree_has_file = collect_magic_subtree(
                    &mut node,
                    &entry_path,
                    &next_relative,
                    rules,
                    descendant_rule_prefixes,
                )? || node.replace;
                if subtree_has_file {
                    target.children.insert(name, node);
                    has_file = true;
                }
            }
            Ok(_) => {
                if matches!(effective_mode, MountMode::Magic) {
                    let node = Node::new_module(&name, &entry)?;
                    if target.children.get(&name).is_some_and(|existing| {
                        existing.file_type != crate::mount::node::NodeFileType::Symlink
                    }) {
                        continue;
                    }
                    target.children.insert(name, node);
                    has_file = true;
                }
            }
            Err(err) => return Err(err.into()),
        }
    }

    Ok(has_file)
}

pub fn collect_module_files(
    module_dir: &Path,
    managed_partitions: &[String],
    magic_modules: &[Module],
) -> Result<Option<Node>> {
    let mut root = Node::new_root("");
    let mut system = Node::new_root("system");
    let module_root = module_dir;
    let mut has_file = HashSet::new();
    let partitions: HashSet<String> = managed_partitions.iter().cloned().collect();

    crate::scoped_log!(
        debug,
        "magic:collect",
        "start: root={}",
        module_root.display()
    );

    module_root
        .read_dir()
        .with_context(|| format!("failed to read module root {}", module_root.display()))?;

    // The selected module slice is already ordered by mount priority. Walking
    // the directory here would make collision handling depend on readdir(3).
    for module in magic_modules {
        let id = &module.id;
        crate::scoped_log!(debug, "magic:collect", "module inspect: id={}", id);

        let module_path = module_root.join(id);
        let module_metadata = match fs::symlink_metadata(&module_path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                crate::scoped_log!(
                    warn,
                    "magic:collect",
                    "module skip: id={}, reason=missing_path",
                    id
                );
                continue;
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to inspect {}", module_path.display()));
            }
        };
        if !module_metadata.file_type().is_dir() {
            crate::scoped_log!(
                warn,
                "magic:collect",
                "module skip: id={}, reason=non_directory_entry",
                id
            );
            continue;
        }

        let prop = module_path.join("module.prop");
        inventory::validate_module_prop_id(&prop, id)?;

        if inventory::is_reserved_module_dir(id) || inventory::has_mount_block_marker(&module_path)?
        {
            crate::scoped_log!(
                debug,
                "magic:collect",
                "module skip: id={}, reason=blocked_or_reserved",
                id
            );
            continue;
        }

        let touched_partitions: Vec<String> = partitions
            .iter()
            .filter(|p| module_path.join(p).is_dir())
            .cloned()
            .collect();

        if touched_partitions.is_empty() {
            for p in &partitions {
                crate::scoped_log!(
                    debug,
                    "magic:collect",
                    "partition untouched: module={}, partition={}",
                    id,
                    p
                );
            }
            continue;
        }

        crate::scoped_log!(
            debug,
            "magic:collect",
            "module collect: path={}",
            module_path.display()
        );
        let rules = &module.rules;
        let descendant_rule_prefixes = rules.descendant_rule_prefixes();

        for p in touched_partitions {
            if p == "system" {
                has_file.insert(collect_magic_subtree(
                    &mut system,
                    &module_path.join(&p),
                    Path::new(&p),
                    rules,
                    &descendant_rule_prefixes,
                )?);
                continue;
            }

            let partition_node = match system.children.entry(p.clone()) {
                Entry::Occupied(mut occupied) => {
                    if occupied.get().file_type == crate::mount::node::NodeFileType::Symlink {
                        occupied.insert(Node::new_root(&p));
                    }
                    occupied.into_mut()
                }
                Entry::Vacant(vacant) => vacant.insert(Node::new_root(&p)),
            };

            has_file.insert(collect_magic_subtree(
                partition_node,
                &module_path.join(&p),
                Path::new(&p),
                rules,
                &descendant_rule_prefixes,
            )?);
        }
    }

    if has_file.contains(&true) {
        for partition in managed_partitions {
            if partition == "system" {
                continue;
            }

            let path_of_root = Path::new("/").join(partition);
            if path_of_root.is_dir() {
                let name = partition.clone();
                if let Some(node) = system.children.remove(&name) {
                    crate::scoped_log!(
                        debug,
                        "magic:collect",
                        "attach managed partition: name={}",
                        name
                    );
                    root.children.insert(name, node);
                }
            }
        }

        root.children.insert("system".to_string(), system);
        Ok(Some(root))
    } else {
        Ok(None)
    }
}

pub fn clone_symlink<S>(src: S, dst: S) -> Result<()>
where
    S: AsRef<Path>,
{
    let src_symlink = read_link(src.as_ref())?;
    symlink(&src_symlink, dst.as_ref())?;
    lsetfilecon(dst.as_ref(), lgetfilecon(src.as_ref())?.as_str()).with_context(|| {
        format!(
            "failed to clone symlink SELinux context to {}",
            dst.as_ref().display()
        )
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn magic_module(root: &Path, id: &str, value: &str) -> Module {
        let module_path = root.join(id);
        fs::create_dir_all(module_path.join("system")).unwrap();
        fs::write(module_path.join("module.prop"), format!("id={id}\n")).unwrap();
        fs::write(module_path.join("system/priority"), value).unwrap();
        Module {
            id: id.to_string(),
            source_path: module_path,
            rules: ModuleRules {
                default_mode: MountMode::Magic,
                ..Default::default()
            },
        }
    }

    #[test]
    fn collection_ignores_non_directory_entries_in_module_root() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("stray"), b"not a module").unwrap();
        let selected = magic_module(temp.path(), "selected", "selected");

        let root = collect_module_files(temp.path(), &["system".to_string()], &[selected]).unwrap();

        assert!(root.is_some());
    }

    #[test]
    fn collection_uses_selected_module_order_for_collisions() {
        let temp = tempfile::tempdir().unwrap();
        let first = magic_module(temp.path(), "first", "first");
        let second = magic_module(temp.path(), "second", "second");

        let root = collect_module_files(
            temp.path(),
            &["system".to_string()],
            &[second.clone(), first],
        )
        .unwrap()
        .unwrap();
        let selected_path = root.children["system"].children["priority"]
            .module_path
            .as_ref()
            .unwrap();

        assert!(selected_path.starts_with(&second.source_path));
    }

    #[test]
    fn collection_skips_a_selected_entry_that_is_not_a_directory() {
        let temp = tempfile::tempdir().unwrap();
        let source_path = temp.path().join("stray");
        fs::write(&source_path, b"not a module").unwrap();
        let selected = Module {
            id: "stray".to_string(),
            source_path,
            rules: ModuleRules {
                default_mode: MountMode::Magic,
                ..Default::default()
            },
        };

        let root = collect_module_files(temp.path(), &["system".to_string()], &[selected]).unwrap();

        assert!(root.is_none());
    }
}
