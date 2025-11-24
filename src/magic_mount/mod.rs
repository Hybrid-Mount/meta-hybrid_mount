mod node;
mod try_umount;

// Only keep XATTR const
pub(super) const REPLACE_DIR_FILE_NAME: &str = ".replace";
pub(super) const REPLACE_DIR_XATTR: &str = "trusted.overlay.opaque";

use std::{
    fs::{self, DirEntry, create_dir, create_dir_all, read_dir, read_link},
    os::unix::fs::{MetadataExt, symlink},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use rustix::{
    fs::{Gid, Mode, Uid, chmod, chown},
    mount::{
        MountFlags, MountPropagationFlags, UnmountFlags, mount, mount_bind, mount_change,
        mount_move, mount_remount, unmount,
    },
};

use crate::{
    magic_mount::{
        node::{Node, NodeFileType},
        try_umount::send_unmountable,
    },
    utils::{ensure_dir_exists, lgetfilecon, lsetfilecon},
};

// Modified to accept a list of specific content paths
fn collect_module_files(content_paths: &[PathBuf], extra_partitions: &[String]) -> Result<Option<Node>> {
    let mut root = Node::new_root("");
    let mut system = Node::new_root("system");
    let mut has_file = false;

    for module_path in content_paths {
        let module_system = module_path.join("system");
        if !module_system.is_dir() {
            continue;
        }

        log::debug!("collecting {}", module_path.display());
        has_file |= system.collect_module_files(&module_system)?;
    }

    if has_file {
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
            // Simple assumption for extra partitions
            let require_symlink = false;

            if path_of_root.is_dir() && (!require_symlink || path_of_system.is_symlink()) {
                let name = partition.to_string();
                if let Some(node) = system.children.remove(&name) {
                    log::debug!("attach extra partition '{}' to root", name);
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

// ... clone_symlink, mount_mirror, do_magic_mount ...
fn clone_symlink<Src: AsRef<Path>, Dst: AsRef<Path>>(src: Src, dst: Dst) -> Result<()> {
    let src_symlink = read_link(src.as_ref())?;
    symlink(&src_symlink, dst.as_ref())?;
    lsetfilecon(dst.as_ref(), lgetfilecon(src.as_ref())?.as_str())?;
    Ok(())
}

fn mount_mirror<P: AsRef<Path>, WP: AsRef<Path>>(path: P, work_dir_path: WP, entry: &DirEntry) -> Result<()> {
    let path = path.as_ref().join(entry.file_name());
    let work_dir_path = work_dir_path.as_ref().join(entry.file_name());
    if entry.file_type()?.is_file() {
        fs::File::create(&work_dir_path)?;
        mount_bind(&path, &work_dir_path)?;
    } else if entry.file_type()?.is_dir() {
        create_dir(&work_dir_path)?;
        let metadata = entry.metadata()?;
        chmod(&work_dir_path, Mode::from_raw_mode(metadata.mode()))?;
        unsafe {
            chown(&work_dir_path, Some(Uid::from_raw(metadata.uid())), Some(Gid::from_raw(metadata.gid())))?;
        }
        lsetfilecon(&work_dir_path, lgetfilecon(&path)?.as_str())?;
        for entry in read_dir(&path)?.flatten() {
            mount_mirror(&path, &work_dir_path, &entry)?;
        }
    } else if entry.file_type()?.is_symlink() {
        clone_symlink(&path, &work_dir_path)?;
    }
    Ok(())
}

fn do_magic_mount<P: AsRef<Path>, WP: AsRef<Path>>(
    path: P,
    work_dir_path: WP,
    current: Node,
    has_tmpfs: bool,
) -> Result<()> {
    let mut current = current;
    let path = path.as_ref().join(&current.name);
    let work_dir_path = work_dir_path.as_ref().join(&current.name);
    
    match current.file_type {
        NodeFileType::RegularFile => {
            let target_path = if has_tmpfs {
                fs::File::create(&work_dir_path)?;
                &work_dir_path
            } else {
                &path
            };
            if let Some(module_path) = &current.module_path {
                mount_bind(module_path, target_path)?;
                let _ = send_unmountable(target_path);
                let _ = mount_remount(target_path, MountFlags::RDONLY | MountFlags::BIND, "");
            }
        }
        NodeFileType::Symlink => {
            if let Some(module_path) = &current.module_path {
                clone_symlink(module_path, &work_dir_path)?;
            }
        }
        NodeFileType::Directory => {
            let mut create_tmpfs = !has_tmpfs && current.replace && current.module_path.is_some();
            if !has_tmpfs && !create_tmpfs {
                for (name, node) in &current.children {
                    let real_path = path.join(name);
                    let need = match node.file_type {
                        NodeFileType::Symlink => true,
                        NodeFileType::Whiteout => real_path.exists(),
                        _ => {
                            if let Ok(meta) = real_path.symlink_metadata() {
                                let ft = NodeFileType::from_file_type(meta.file_type()).unwrap_or(NodeFileType::Whiteout);
                                ft != node.file_type || ft == NodeFileType::Symlink
                            } else { true }
                        }
                    };
                    if need {
                        create_tmpfs = true;
                        break;
                    }
                }
            }

            let has_tmpfs = has_tmpfs || create_tmpfs;

            if has_tmpfs {
                create_dir_all(&work_dir_path)?;
                let (metadata, src_path) = if path.exists() { (path.metadata()?, &path) } 
                                           else { (current.module_path.as_ref().unwrap().metadata()?, current.module_path.as_ref().unwrap()) };
                chmod(&work_dir_path, Mode::from_raw_mode(metadata.mode()))?;
                // FIX: Added unsafe block
                unsafe {
                    chown(&work_dir_path, Some(Uid::from_raw(metadata.uid())), Some(Gid::from_raw(metadata.gid())))?;
                }
                lsetfilecon(&work_dir_path, lgetfilecon(src_path)?.as_str())?;
            }

            if create_tmpfs {
                mount_bind(&work_dir_path, &work_dir_path)?;
            }

            if path.exists() && !current.replace {
                for entry in path.read_dir()?.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if let Some(node) = current.children.remove(&name) {
                        if !node.skip {
                            do_magic_mount(&path, &work_dir_path, node, has_tmpfs)?;
                        }
                    } else if has_tmpfs {
                        mount_mirror(&path, &work_dir_path, &entry)?;
                    }
                }
            }

            for (_, node) in current.children {
                if !node.skip {
                    do_magic_mount(&path, &work_dir_path, node, has_tmpfs)?;
                }
            }

            if create_tmpfs {
                let _ = mount_remount(&work_dir_path, MountFlags::RDONLY | MountFlags::BIND, "");
                mount_move(&work_dir_path, &path)?;
                let _ = mount_change(&path, MountPropagationFlags::PRIVATE);
                let _ = send_unmountable(&path);
            }
        }
        NodeFileType::Whiteout => {}
    }
    Ok(())
}

// Public Entry Point
pub fn mount_partitions(
    tmp_path: &Path,
    module_paths: &[PathBuf],
    mount_source: &str,
    extra_partitions: &[String],
) -> Result<()> {
    if let Some(root) = collect_module_files(module_paths, extra_partitions)? {
        log::debug!("Magic Mount Root: {}", root);

        let tmp_dir = tmp_path.join("workdir");
        ensure_dir_exists(&tmp_dir)?;

        // FIX: Changed None to "" for data argument to satisfy rustix::path::Arg trait
        mount(mount_source, &tmp_dir, "tmpfs", MountFlags::empty(), "").context("mount tmp")?;
        mount_change(&tmp_dir, MountPropagationFlags::PRIVATE).context("make tmp private")?;

        let result = do_magic_mount("/", &tmp_dir, root, false);

        if let Err(e) = unmount(&tmp_dir, UnmountFlags::DETACH) {
            log::error!("failed to unmount tmp {}", e);
        }
        fs::remove_dir(tmp_dir).ok();

        result
    } else {
        log::info!("No files to magic mount");
        Ok(())
    }
}
