// Copyright 2026 https://github.com/Tools-cx-app/meta-magic_mount

use std::{
    collections::HashSet,
    fs::{self, DirEntry, Metadata, create_dir, create_dir_all, read_link},
    os::unix::fs::{MetadataExt, symlink},
    path::{Path, PathBuf},
};

use anyhow::{Result, bail};
use rustix::{
    fs::{Gid, Mode, Uid, chmod, chown},
    mount::mount_bind,
};

use crate::{
    defs::{DISABLE_FILE_NAME, REMOVE_FILE_NAME, SKIP_MOUNT_FILE_NAME},
    mount::node::Node,
    utils::{lgetfilecon, lsetfilecon, validate_module_id},
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

    log::debug!("begin collect module files: {}", module_root.display());

    for entry in module_root.read_dir()?.flatten() {
        if !entry.file_type()?.is_dir() {
            continue;
        }

        let id = entry.file_name().to_str().unwrap().to_string();
        log::debug!("processing new module: {id}");

        if !need_id.contains(&id) {
            log::debug!("module {id} was blocked.");
            continue;
        }

        let prop = entry.path().join("module.prop");
        if !prop.exists() {
            log::debug!("skipped module {id}, because not found module.prop");
            continue;
        }
        let string = fs::read_to_string(prop)?;
        for line in string.lines() {
            if line.starts_with("id")
                && let Some((_, value)) = line.split_once('=')
            {
                validate_module_id(value)?;
            }
        }

        if entry.path().join(DISABLE_FILE_NAME).exists()
            || entry.path().join(REMOVE_FILE_NAME).exists()
            || entry.path().join(SKIP_MOUNT_FILE_NAME).exists()
        {
            log::debug!("skipped module {id}, due to disable/remove/skip_mount");
            continue;
        }

        let mut modified = false;
        let mut partitions = HashSet::new();
        partitions.insert("system".to_string());
        partitions.extend(extra_partitions.iter().cloned());

        for p in &partitions {
            if entry.path().join(p).is_dir() {
                modified = true;
                break;
            }
            log::debug!("{id} due not modify {p}");
        }

        if !modified {
            continue;
        }

        log::debug!("collecting {}", entry.path().display());

        for p in partitions {
            if !entry.path().join(&p).exists() {
                continue;
            }

            has_file.insert(system.collect_module_files(entry.path().join(&p))?);
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
