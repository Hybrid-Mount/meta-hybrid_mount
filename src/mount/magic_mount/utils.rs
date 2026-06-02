// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::{
    collections::{HashMap, HashSet, btree_map::Entry},
    fs::{self, DirEntry, Metadata, create_dir, create_dir_all, read_link},
    io::{BufRead, BufReader},
    os::unix::fs::{MetadataExt, symlink},
    path::{Path, PathBuf},
};

use anyhow::{Result, bail};
use rustix::fs::{Gid, Mode, Uid, chmod, chown};
#[cfg(any(target_os = "linux", target_os = "android"))]
pub(super) use rustix::mount::mount_bind;

use crate::{
    core::inventory::{self, Module},
    domain::{ModuleRules, MountMode},
    mount::node::Node,
    sys::fs::{lgetfilecon, lsetfilecon},
    utils::validate_module_id,
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
            let entry = match entry_result {
                Ok(entry) => entry,
                Err(err) => {
                    crate::scoped_log!(
                        warn,
                        "magic:collect",
                        "enumerate mirror failed: path={}, error={}",
                        path.display(),
                        err
                    );
                    continue;
                }
            };
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
    use_kasumi: bool,
) -> Result<bool> {
    let mut has_file = false;

    for entry_result in module_dir.read_dir()? {
        let entry = match entry_result {
            Ok(entry) => entry,
            Err(err) => {
                crate::scoped_log!(
                    warn,
                    "magic:collect",
                    "enumerate subtree failed: path={}, error={}",
                    module_dir.display(),
                    err
                );
                continue;
            }
        };

        let file_name = entry.file_name();
        let name = file_name.to_string_lossy().into_owned();
        let entry_path = entry.path();
        let next_relative = relative_path.join(&file_name);
        let next_relative_key = next_relative.to_string_lossy();
        let effective_mode = rules.effective_mode(&next_relative, use_kasumi);

        match entry.file_type() {
            Ok(file_type) if file_type.is_dir() => {
                let has_descendant_rules =
                    descendant_rule_prefixes.contains(next_relative_key.as_ref());
                if matches!(effective_mode, MountMode::Magic) && !has_descendant_rules {
                    if let Some(mut node) = Node::new_module(&name, &entry) {
                        let subtree_has_file =
                            node.collect_module_files(&entry_path)? || node.replace;
                        if subtree_has_file {
                            target.children.insert(name, node);
                            has_file = true;
                        }
                    }
                    continue;
                }

                if !has_descendant_rules {
                    continue;
                }

                let Some(mut node) = Node::new_module(&name, &entry) else {
                    continue;
                };
                let subtree_has_file = collect_magic_subtree(
                    &mut node,
                    &entry_path,
                    &next_relative,
                    rules,
                    descendant_rule_prefixes,
                    use_kasumi,
                )? || node.replace;
                if subtree_has_file {
                    target.children.insert(name, node);
                    has_file = true;
                }
            }
            Ok(_) => {
                if matches!(effective_mode, MountMode::Magic)
                    && let Some(node) = Node::new_module(&name, &entry)
                {
                    if target.children.get(&name).is_some_and(|existing| {
                        existing.file_type != crate::mount::node::NodeFileType::Symlink
                    }) {
                        continue;
                    }
                    target.children.insert(name, node);
                    has_file = true;
                }
            }
            Err(err) => {
                crate::scoped_log!(
                    warn,
                    "magic:collect",
                    "file type failed: path={}, error={}",
                    entry_path.display(),
                    err
                );
            }
        }
    }

    Ok(has_file)
}

pub fn collect_module_files(
    module_dir: &Path,
    managed_partitions: &[String],
    magic_modules: &[Module],
    use_kasumi: bool,
) -> Result<Option<Node>> {
    let mut root = Node::new_root("");
    let mut system = Node::new_root("system");
    let module_root = module_dir;
    let mut has_file = HashSet::new();
    let partitions: HashSet<String> = managed_partitions.iter().cloned().collect();
    let selected_rules: HashMap<&str, &ModuleRules> = magic_modules
        .iter()
        .map(|module| (module.id.as_str(), &module.rules))
        .collect();

    crate::scoped_log!(
        debug,
        "magic:collect",
        "start: root={}",
        module_root.display()
    );

    for entry_result in module_root.read_dir()? {
        let entry = match entry_result {
            Ok(entry) => entry,
            Err(err) => {
                crate::scoped_log!(
                    warn,
                    "magic:collect",
                    "enumerate root failed: path={}, error={}",
                    module_root.display(),
                    err
                );
                continue;
            }
        };
        if !entry.file_type()?.is_dir() {
            continue;
        }

        let file_name = entry.file_name();
        let Some(id) = file_name.to_str().map(str::to_owned) else {
            crate::scoped_log!(
                warn,
                "magic:collect",
                "skip: reason=non_utf8_module_dir, name={:?}",
                file_name
            );
            continue;
        };
        crate::scoped_log!(debug, "magic:collect", "module inspect: id={}", id);

        let Some(rules) = selected_rules.get(id.as_str()).copied() else {
            crate::scoped_log!(
                debug,
                "magic:collect",
                "module skip: id={}, reason=not_selected",
                id
            );
            continue;
        };

        let module_path = entry.path();
        let prop = module_path.join("module.prop");
        if !prop.is_file() {
            crate::scoped_log!(
                debug,
                "magic:collect",
                "module skip: id={}, reason=missing_module_prop",
                id
            );
            continue;
        }
        if !is_valid_module_prop_id(&prop)? {
            crate::scoped_log!(
                debug,
                "magic:collect",
                "module skip: id={}, reason=invalid_module_id",
                id
            );
            continue;
        }

        if inventory::is_reserved_module_dir(&id) || inventory::has_mount_block_marker(&module_path)
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
        let descendant_rule_prefixes = rules.descendant_rule_prefixes();

        for p in touched_partitions {
            if p == "system" {
                has_file.insert(collect_magic_subtree(
                    &mut system,
                    &module_path.join(&p),
                    Path::new(&p),
                    rules,
                    &descendant_rule_prefixes,
                    use_kasumi,
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
                use_kasumi,
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

fn is_valid_module_prop_id(prop: &Path) -> Result<bool> {
    let file = fs::File::open(prop)?;
    for line_result in BufReader::new(file).lines() {
        let line = match line_result {
            Ok(line) => line,
            Err(e) => {
                crate::scoped_log!(
                    warn,
                    "magic:collect",
                    "read module.prop failed: path={}, error={}",
                    prop.display(),
                    e
                );
                return Ok(false);
            }
        };
        if line.starts_with("id")
            && let Some((_, value)) = line.split_once('=')
        {
            return Ok(validate_module_id(value).is_ok());
        }
    }
    Ok(true)
}

pub fn clone_symlink<S>(src: S, dst: S) -> Result<()>
where
    S: AsRef<Path>,
{
    let src_symlink = read_link(src.as_ref())?;
    symlink(&src_symlink, dst.as_ref())?;
    if let Err(e) = lsetfilecon(dst.as_ref(), lgetfilecon(src.as_ref())?.as_str()) {
        crate::scoped_log!(
            debug,
            "magic_mount",
            "clone symlink selinux context failed: dst={}, error={:#}",
            dst.as_ref().display(),
            e
        );
    }
    Ok(())
}
