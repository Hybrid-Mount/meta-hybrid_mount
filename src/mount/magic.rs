// Copyright 2025 Meta-Hybrid Mount Authors
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    collections::{HashMap, HashSet},
    fs::{self, DirEntry, create_dir, read_dir, read_link},
    os::unix::fs::{MetadataExt, symlink},
    path::{Path, PathBuf},
    sync::atomic::AtomicU32,
};

use anyhow::{Context, Result, bail};
use rustix::{
    fs::{Gid, Mode, Uid, chmod, chown},
    mount::{
        MountFlags, MountPropagationFlags, UnmountFlags, mount, mount_bind, mount_change,
        mount_move, mount_remount, unmount,
    },
};

#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::try_umount::send_unmountable;
use crate::{
    defs::{DISABLE_FILE_NAME, REMOVE_FILE_NAME, SKIP_MOUNT_FILE_NAME},
    mount::node::{Node, NodeFileType},
    utils::{ensure_dir_exists, lgetfilecon, lsetfilecon, validate_module_id},
};

const ROOT_PARTITIONS: [&str; 4] = ["vendor", "system_ext", "product", "odm"];

static MOUNTED_FILES: AtomicU32 = AtomicU32::new(0);
static MOUNTED_SYMBOLS_FILES: AtomicU32 = AtomicU32::new(0);

fn clone_symlink<S>(src: S, dst: S) -> Result<()>
where
    S: AsRef<Path>,
{
    let src_symlink = read_link(src.as_ref())?;
    symlink(&src_symlink, dst.as_ref())?;
    lsetfilecon(dst.as_ref(), lgetfilecon(src.as_ref())?.as_str())?;
    Ok(())
}

fn mount_mirror<P>(path: P, work_dir_path: P, entry: &DirEntry) -> Result<()>
where
    P: AsRef<Path>,
{
    let path = path.as_ref().join(entry.file_name());
    let work_dir_path = work_dir_path.as_ref().join(entry.file_name());
    let file_type = entry.file_type()?;

    if file_type.is_file() {
        fs::File::create(&work_dir_path)?;
        mount_bind(&path, &work_dir_path)?;
    } else if file_type.is_dir() {
        create_dir(&work_dir_path)?;
        let metadata = entry.metadata()?;
        chmod(&work_dir_path, Mode::from_raw_mode(metadata.mode()))?;
        chown(
            &work_dir_path,
            Some(Uid::from_raw(metadata.uid())),
            Some(Gid::from_raw(metadata.gid())),
        )?;
        lsetfilecon(&work_dir_path, lgetfilecon(&path)?.as_str())?;
        for entry in read_dir(&path)?.flatten() {
            mount_mirror(&path, &work_dir_path, &entry)?;
        }
    } else if file_type.is_symlink() {
        clone_symlink(&path, &work_dir_path)?;
    }
    Ok(())
}

fn process_module(
    path: &Path,
    extra_partitions: &[String],
    exclusion_list: Option<&HashSet<String>>,
) -> Result<(Node, Node)> {
    let mut root = Node::new_root("");
    let mut system = Node::new_root("system");

    if path.join(DISABLE_FILE_NAME).exists()
        || path.join(REMOVE_FILE_NAME).exists()
        || path.join(SKIP_MOUNT_FILE_NAME).exists()
    {
        return Ok((root, system));
    }

    if let Some(name) = path.file_name().and_then(|n| n.to_str())
        && let Err(e) = validate_module_id(name)
    {
        log::warn!("Skipping invalid module {}: {}", name, e);
        return Ok((root, system));
    }

    let is_excluded = |part: &str| -> bool {
        if let Some(list) = exclusion_list {
            list.contains(part)
        } else {
            false
        }
    };

    if !is_excluded("system") {
        let mod_system = path.join("system");
        if mod_system.is_dir() {
            system.collect_module_files(&mod_system)?;
        }
    }

    for partition in ROOT_PARTITIONS {
        if is_excluded(partition) {
            continue;
        }
        let mod_part = path.join(partition);
        if mod_part.is_dir() {
            let node = system
                .children
                .entry(partition.to_string())
                .or_insert_with(|| Node::new_root(partition));
            if node.file_type == NodeFileType::Symlink {
                node.file_type = NodeFileType::Directory;
                node.module_path = None;
            }
            node.collect_module_files(&mod_part)?;
        }
    }

    for partition in extra_partitions {
        if ROOT_PARTITIONS.contains(&partition.as_str()) || partition == "system" {
            continue;
        }
        if is_excluded(partition) {
            continue;
        }

        let path_of_root = Path::new("/").join(partition);
        let path_of_system = Path::new("/system").join(partition);

        if path_of_root.is_dir() {
            let name = partition.clone();
            let mod_part = path.join(partition);
            // Fix: Collapsed if statement to satisfy clippy
            if mod_part.is_dir() && (!path_of_system.exists() || path_of_system.is_symlink()) {
                let node = root
                    .children
                    .entry(name)
                    .or_insert_with(|| Node::new_root(partition));
                node.collect_module_files(&mod_part)?;
            }
        }
    }
    Ok((root, system))
}

fn merge_nodes(high: &mut Node, low: Node) {
    if high.module_path.is_none() {
        high.module_path = low.module_path;
        high.file_type = low.file_type;
        high.replace = low.replace;
    }
    for (name, low_child) in low.children {
        match high.children.entry(name) {
            std::collections::hash_map::Entry::Vacant(v) => {
                v.insert(low_child);
            }
            std::collections::hash_map::Entry::Occupied(mut o) => {
                merge_nodes(o.get_mut(), low_child);
            }
        }
    }
}

fn collect_module_files(
    module_paths: &[PathBuf],
    extra_partitions: &[String],
    exclusions: &HashMap<PathBuf, HashSet<String>>,
) -> Result<Option<Node>> {
    let (mut final_root, mut final_system) = module_paths
        .iter()
        .map(|path| {
            let exclusion = exclusions.get(path);
            process_module(path, extra_partitions, exclusion)
        })
        .reduce(|a, b| {
            let (mut r_a, mut s_a) = a?;
            let (r_b, s_b) = b?;
            merge_nodes(&mut r_a, r_b);
            merge_nodes(&mut s_a, s_b);
            Ok((r_a, s_a))
        })
        .unwrap_or(Ok((Node::new_root(""), Node::new_root("system"))))?;

    let has_content = !final_root.children.is_empty() || !final_system.children.is_empty();

    if has_content {
        const BUILTIN_CHECKS: [(&str, bool); 4] = [
            ("vendor", true),
            ("system_ext", true),
            ("product", true),
            ("odm", false),
        ];

        for (partition, require_symlink) in BUILTIN_CHECKS {
            let path_of_root = Path::new("/").join(partition);
            let path_of_system = Path::new("/system").join(partition);

            if path_of_root.is_dir() && (!require_symlink || path_of_system.is_symlink()) {
                let name = partition.to_string();
                if let Some(node) = final_system.children.remove(&name) {
                    final_root.children.insert(name, node);
                }
            }
        }
        final_root
            .children
            .insert("system".to_string(), final_system);
        Ok(Some(final_root))
    } else {
        Ok(None)
    }
}

struct MagicMount {
    node: Node,
    path: PathBuf,
    work_dir_path: PathBuf,
    has_tmpfs: bool,
    #[cfg(any(target_os = "linux", target_os = "android"))]
    umount: bool,
}

impl MagicMount {
    fn new<P>(
        node: &Node,
        path: P,
        work_dir_path: P,
        has_tmpfs: bool,
        #[cfg(any(target_os = "linux", target_os = "android"))] umount: bool,
    ) -> Self
    where
        P: AsRef<Path>,
    {
        Self {
            node: node.clone(),
            path: path.as_ref().join(node.name.clone()),
            work_dir_path: work_dir_path.as_ref().join(node.name.clone()),
            has_tmpfs,
            #[cfg(any(target_os = "linux", target_os = "android"))]
            umount,
        }
    }

    fn do_magic_mount(&mut self) -> Result<()> {
        match self.node.file_type {
            NodeFileType::RegularFile => self.handle_regular_file(),
            NodeFileType::Symlink => self.handle_symlink(),
            NodeFileType::Directory => self.handle_directory(),
            NodeFileType::Whiteout => {
                log::debug!("file {} is removed", self.path.display());
                Ok(())
            }
        }
    }

    fn handle_regular_file(&self) -> Result<()> {
        let target_path = if self.has_tmpfs {
            fs::File::create(&self.work_dir_path)?;
            &self.work_dir_path
        } else {
            &self.path
        };

        if let Some(module_path) = &self.node.module_path {
            mount_bind(module_path, target_path).with_context(|| {
                #[cfg(any(target_os = "linux", target_os = "android"))]
                if self.umount {
                    let _ = send_unmountable(target_path);
                }
                format!(
                    "mount module file {} -> {}",
                    module_path.display(),
                    self.work_dir_path.display()
                )
            })?;

            if let Err(e) = mount_remount(target_path, MountFlags::RDONLY | MountFlags::BIND, "") {
                log::warn!("make file {} ro: {e:#?}", target_path.display());
            }

            let mounted = MOUNTED_FILES.load(std::sync::atomic::Ordering::Relaxed) + 1;
            MOUNTED_FILES.store(mounted, std::sync::atomic::Ordering::Relaxed);
            Ok(())
        } else {
            bail!("cannot mount root file {}!", self.path.display());
        }
    }

    fn handle_symlink(&self) -> Result<()> {
        if let Some(module_path) = &self.node.module_path {
            clone_symlink(module_path, &self.work_dir_path)?;

            let mounted = MOUNTED_SYMBOLS_FILES.load(std::sync::atomic::Ordering::Relaxed) + 1;
            MOUNTED_SYMBOLS_FILES.store(mounted, std::sync::atomic::Ordering::Relaxed);
            Ok(())
        } else {
            bail!("cannot mount root symlink {}!", self.path.display());
        }
    }

    fn handle_directory(&mut self) -> Result<()> {
        let mut create_tmpfs =
            !self.has_tmpfs && self.node.replace && self.node.module_path.is_some();
        if !self.has_tmpfs && !create_tmpfs {
            for node in self.node.children.values() {
                let real_path = self.path.join(&node.name);
                let need = match node.file_type {
                    NodeFileType::Symlink => true,
                    NodeFileType::Whiteout => real_path.exists(),
                    _ => {
                        if let Ok(metadata) = real_path.symlink_metadata() {
                            let file_type = NodeFileType::from(metadata.file_type());
                            file_type != node.file_type || file_type == NodeFileType::Symlink
                        } else {
                            true
                        }
                    }
                };
                if need {
                    if node.module_path.is_none() {
                        continue;
                    }
                    create_tmpfs = true;
                    break;
                }
            }
        }

        let has_tmpfs = self.has_tmpfs || create_tmpfs;

        if has_tmpfs {
            ensure_dir_exists(&self.work_dir_path)?;
            if let Ok((metadata, path)) = if self.path.exists() {
                Ok((self.path.metadata()?, &self.path))
            } else if let Some(mp) = &self.node.module_path {
                Ok((mp.metadata()?, mp))
            } else {
                Err(anyhow::anyhow!("No source for dir"))
            } {
                chmod(&self.work_dir_path, Mode::from_raw_mode(metadata.mode()))?;
                chown(
                    &self.work_dir_path,
                    Some(Uid::from_raw(metadata.uid())),
                    Some(Gid::from_raw(metadata.gid())),
                )?;
                lsetfilecon(&self.work_dir_path, lgetfilecon(path)?.as_str())?;
            }
        }

        if create_tmpfs {
            mount_bind(&self.work_dir_path, &self.work_dir_path)?;
        }

        if self.path.exists() && !self.node.replace {
            for entry in self.path.read_dir()?.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if let Some(node) = self.node.children.remove(&name) {
                    if !node.skip {
                        Self::new(
                            &node,
                            &self.path,
                            &self.work_dir_path,
                            has_tmpfs,
                            #[cfg(any(target_os = "linux", target_os = "android"))]
                            self.umount,
                        )
                        .do_magic_mount()?;
                    }
                } else if has_tmpfs {
                    mount_mirror(&self.path, &self.work_dir_path, &entry)?;
                }
            }
        }

        for node in self.node.children.values() {
            if !node.skip {
                Self::new(
                    node,
                    &self.path,
                    &self.work_dir_path,
                    has_tmpfs,
                    #[cfg(any(target_os = "linux", target_os = "android"))]
                    self.umount,
                )
                .do_magic_mount()?;
            }
        }

        if create_tmpfs {
            mount_remount(
                &self.work_dir_path,
                MountFlags::RDONLY | MountFlags::BIND,
                "",
            )
            .ok();
            mount_move(&self.work_dir_path, &self.path)?;
            mount_change(&self.path, MountPropagationFlags::PRIVATE)?;

            #[cfg(any(target_os = "linux", target_os = "android"))]
            if self.umount {
                let _ = send_unmountable(&self.path);
            }
        }
        Ok(())
    }
}

pub fn mount_partitions(
    tmp_path: &Path,
    module_paths: &[PathBuf],
    mount_source: &str,
    extra_partitions: &[String],
    exclusions: HashMap<PathBuf, HashSet<String>>,
    #[cfg(any(target_os = "linux", target_os = "android"))] disable_umount: bool,
    #[cfg(not(any(target_os = "linux", target_os = "android")))] _disable_umount: bool,
) -> Result<()> {
    if let Some(root) = collect_module_files(module_paths, extra_partitions, &exclusions)? {
        let tmp_dir = tmp_path.join("workdir");
        ensure_dir_exists(&tmp_dir)?;

        mount(
            mount_source,
            &tmp_dir,
            "tmpfs",
            MountFlags::empty(),
            None::<&std::ffi::CStr>,
        )
        .context("mount tmp")?;

        mount_change(&tmp_dir, MountPropagationFlags::PRIVATE)?;

        let result = MagicMount::new(
            &root,
            Path::new("/"),
            tmp_dir.as_path(),
            false,
            #[cfg(any(target_os = "linux", target_os = "android"))]
            !disable_umount,
        )
        .do_magic_mount();

        let _ = unmount(&tmp_dir, UnmountFlags::DETACH);

        #[cfg(any(target_os = "linux", target_os = "android"))]
        if !disable_umount {
            let _ = crate::try_umount::commit();
        }

        fs::remove_dir(tmp_dir).ok();

        let files = MOUNTED_FILES.load(std::sync::atomic::Ordering::Relaxed);
        let symlinks = MOUNTED_SYMBOLS_FILES.load(std::sync::atomic::Ordering::Relaxed);
        log::info!(
            "Magic Mount: {} files, {} symlinks processed.",
            files,
            symlinks
        );

        result
    } else {
        Ok(())
    }
}
