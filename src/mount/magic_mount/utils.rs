// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    collections::HashSet,
    fs::{self, DirEntry, Metadata, create_dir, create_dir_all, read_link},
    io::{BufRead, BufReader},
    os::unix::fs::{MetadataExt, symlink},
    path::{Path, PathBuf},
};

use anyhow::{Result, bail};
use rustix::{
    fs::{Gid, Mode, Uid, chmod, chown},
    mount::mount_bind,
};

use crate::{
    core::inventory,
    mount::node::Node,
    sys::fs::{lgetfilecon, lsetfilecon},
    utils::validate_module_id,
};

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

pub fn tmpfs_skeleton<P>(path: P, work_dir_path: P, node: &Node) -> Result<()>
where
    P: AsRef<Path>,
{
    let (path, work_dir_path) = (path.as_ref(), work_dir_path.as_ref());
    log::debug!(
        "creating tmpfs skeleton for {} at {}",
        path.display(),
        work_dir_path.display()
    );

    create_dir_all(work_dir_path)?;

    let (metadata, path) = metadata_path(path, node)?;

    chmod(work_dir_path, Mode::from_raw_mode(metadata.mode()))?;
    chown(
        work_dir_path,
        Some(Uid::from_raw(metadata.uid())),
        Some(Gid::from_raw(metadata.gid())),
    )?;
    lsetfilecon(work_dir_path, lgetfilecon(path)?.as_str())?;

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
        log::debug!(
            "mount mirror file {} -> {}",
            path.display(),
            work_dir_path.display()
        );
        fs::File::create(&work_dir_path)?;
        mount_bind(&path, &work_dir_path)?;
    } else if file_type.is_dir() {
        log::debug!(
            "mount mirror dir {} -> {}",
            path.display(),
            work_dir_path.display()
        );
        create_dir(&work_dir_path)?;
        let metadata = entry.metadata()?;
        chmod(&work_dir_path, Mode::from_raw_mode(metadata.mode()))?;
        chown(
            &work_dir_path,
            Some(Uid::from_raw(metadata.uid())),
            Some(Gid::from_raw(metadata.gid())),
        )?;
        lsetfilecon(&work_dir_path, lgetfilecon(&path)?.as_str())?;
        for entry in path.read_dir()?.flatten() {
            mount_mirror(&path, &work_dir_path, &entry)?;
        }
    } else if file_type.is_symlink() {
        log::debug!(
            "create mirror symlink {} -> {}",
            path.display(),
            work_dir_path.display()
        );
        clone_symlink(&path, &work_dir_path)?;
    }

    Ok(())
}

pub fn collect_module_files(
    module_dir: &Path,
    extra_partitions: &[String],
    need_id: HashSet<String>,
) -> Result<Option<Node>> {
    let mut root = Node::new_root("");
    let mut system = Node::new_root("system");
    let module_root = module_dir;
    let mut has_file = HashSet::new();
    let mut partitions = HashSet::new();
    partitions.insert("system".to_string());
    partitions.extend(extra_partitions.iter().cloned());

    log::debug!("begin collect module files: {}", module_root.display());

    for entry in module_root.read_dir()?.flatten() {
        if !entry.file_type()?.is_dir() {
            continue;
        }

        let file_name = entry.file_name();
        let Some(id) = file_name.to_str().map(str::to_owned) else {
            log::warn!("skipped non-utf8 module directory name: {:?}", file_name);
            continue;
        };
        log::debug!("processing new module: {id}");

        if !need_id.contains(&id) {
            log::debug!("module {id} was blocked.");
            continue;
        }

        let module_path = entry.path();
        let prop = module_path.join("module.prop");
        if !prop.is_file() {
            log::debug!("skipped module {id}, because not found module.prop");
            continue;
        }
        if !is_valid_module_prop_id(&prop)? {
            log::debug!("skipped module {id}, invalid ID format");
            continue;
        }

        if inventory::is_reserved_module_dir(&id) || inventory::has_mount_block_marker(&module_path)
        {
            log::debug!("skipped module {id}, due to disable/remove/skip_mount");
            continue;
        }

        let touched_partitions: Vec<String> = partitions
            .iter()
            .filter(|p| module_path.join(p).is_dir())
            .cloned()
            .collect();

        if touched_partitions.is_empty() {
            for p in &partitions {
                log::debug!("{id} due not modify {p}");
            }
            continue;
        }

        log::debug!("collecting {}", module_path.display());

        for p in touched_partitions {
            has_file.insert(system.collect_module_files(module_path.join(p))?);
        }
    }

    if has_file.contains(&true) {
        const BUILTIN_PARTITIONS: [(&str, bool); 4] = [
            ("vendor", true),
            ("system_ext", true),
            ("product", true),
            ("odm", false),
        ];

        for (partition, require_symlink) in BUILTIN_PARTITIONS {
            let path_of_root = Path::new("/").join(partition);
            let path_of_system = Path::new("/system").join(partition);
            if path_of_root.is_dir() && (!require_symlink || path_of_system.is_symlink()) {
                let name = partition.to_string();
                if let Some(node) = system.children.remove(&name) {
                    root.children.insert(name, node);
                }
            }
        }

        for partition in extra_partitions {
            if BUILTIN_PARTITIONS.iter().any(|(p, _)| p == partition) {
                continue;
            }
            if partition == "system" {
                continue;
            }

            let path_of_root = Path::new("/").join(partition);
            let path_of_system = Path::new("/system").join(partition);
            let require_symlink = false;

            if path_of_root.is_dir() && (!require_symlink || path_of_system.is_symlink()) {
                let name = partition.clone();
                if let Some(node) = system.children.remove(&name) {
                    log::debug!("attach extra partition '{name}' to root");
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
                log::warn!("failed to read module.prop {}: {}", prop.display(), e);
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
    lsetfilecon(dst.as_ref(), lgetfilecon(src.as_ref())?.as_str())?;
    log::debug!(
        "clone symlink {} -> {}({})",
        dst.as_ref().display(),
        dst.as_ref().display(),
        src_symlink.display()
    );
    Ok(())
}
