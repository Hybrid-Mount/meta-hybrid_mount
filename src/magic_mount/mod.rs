mod node;
mod try_umount;

use std::{
    fs::{self, DirEntry, create_dir, create_dir_all, read_dir, read_link},
    os::unix::fs::{MetadataExt, symlink},
    path::Path,
    sync::atomic::AtomicBool,
};

use anyhow::{Context, Result, bail};
use rustix::{
    fs::{Gid, Mode, Uid, chmod, chown},
    mount::{
        MountFlags, MountPropagationFlags, UnmountFlags, mount, mount_bind, mount_change,
        mount_move, mount_remount, unmount,
    },
    path::Arg,
};

use crate::{
    defs::{DISABLE_FILE_NAME, REMOVE_FILE_NAME, SKIP_MOUNT_FILE_NAME},
    magic_mount::{
        node::{Node, NodeFileType},
        try_umount::send_unmountable,
    },
    utils::{ensure_dir_exists, lgetfilecon, lsetfilecon},
};

pub static UMOUNT: AtomicBool = AtomicBool::new(false);

fn collect_module_files(module_dir: &Path, extra_partitions: &[String]) -> Result<Option<Node>> {
    let mut root = Node::new_root("");
    let mut system = Node::new_root("system");
    let module_root = module_dir;
    let mut has_file = false;

    for entry in module_root.read_dir()?.flatten() {
        if !entry.file_type()?.is_dir() {
            continue;
        }

        if entry.path().join(DISABLE_FILE_NAME).exists()
            || entry.path().join(REMOVE_FILE_NAME).exists()
            || entry.path().join(SKIP_MOUNT_FILE_NAME).exists()
        {
            continue;
        }

        let mod_system = entry.path().join("system");
        if !mod_system.is_dir() {
            continue;
        }

        log::debug!("collecting {}", entry.path().display());

        has_file |= system.collect_module_files(&mod_system)?;
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

fn clone_symlink<S>(src: S, dst: S) -> Result<()>
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

fn mount_mirror<P>(path: P, work_dir_path: P, entry: &DirEntry) -> Result<()>
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
        for entry in read_dir(&path)?.flatten() {
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

#[allow(clippy::too_many_lines)]
fn do_magic_mount<P>(path: P, work_dir_path: P, current: Node, has_tmpfs: bool) -> Result<()>
where
    P: AsRef<Path>,
{
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
                log::debug!(
                    "mount module file {} -> {}",
                    module_path.display(),
                    work_dir_path.display()
                );
                mount_bind(module_path, target_path).with_context(|| {
                    if UMOUNT.load(std::sync::atomic::Ordering::Relaxed) {
                        // tell ksu about this mount
                        let _ = send_unmountable(target_path);
                    }
                    format!(
                        "mount module file {} -> {}",
                        module_path.display(),
                        work_dir_path.display(),
                    )
                })?;
                // we should use MS_REMOUNT | MS_BIND | MS_xxx to change mount flags
                if let Err(e) =
                    mount_remount(target_path, MountFlags::RDONLY | MountFlags::BIND, "")
                {
                    log::warn!("make file {} ro: {e:#?}", target_path.display());
                }
            } else {
                bail!("cannot mount root file {}!", path.display());
            }
        }
        NodeFileType::Symlink => {
            if let Some(module_path) = &current.module_path {
                log::debug!(
                    "create module symlink {} -> {}",
                    module_path.display(),
                    work_dir_path.display()
                );
                clone_symlink(module_path, &work_dir_path).with_context(|| {
                    format!(
                        "create module symlink {} -> {}",
                        module_path.display(),
                        work_dir_path.display(),
                    )
                })?;
            } else {
                bail!("cannot mount root symlink {}!", path.display());
            }
        }
        NodeFileType::Directory => {
            let mut create_tmpfs = !has_tmpfs && current.replace && current.module_path.is_some();
            if !has_tmpfs && !create_tmpfs {
                for it in &mut current.children {
                    let (name, node) = it;
                    let real_path = path.join(name);
                    let need = match node.file_type {
                        NodeFileType::Symlink => true,
                        NodeFileType::Whiteout => real_path.exists(),
                        _ => {
                            if let Ok(metadata) = real_path.symlink_metadata() {
                                let file_type = NodeFileType::from(metadata.file_type());
                                file_type != node.file_type || file_type == NodeFileType::Symlink
                            } else {
                                // real path not exists
                                true
                            }
                        }
                    };
                    if need {
                        if current.module_path.is_none() {
                            log::error!(
                                "cannot create tmpfs on {}, ignore: {name}",
                                path.display()
                            );
                            node.skip = true;
                            continue;
                        }
                        create_tmpfs = true;
                        break;
                    }
                }
            }

            let has_tmpfs = has_tmpfs || create_tmpfs;

            if has_tmpfs {
                log::debug!(
                    "creating tmpfs skeleton for {} at {}",
                    path.display(),
                    work_dir_path.display()
                );
                create_dir_all(&work_dir_path)?;
                let (metadata, path) = if path.exists() {
                    (path.metadata()?, &path)
                } else if let Some(module_path) = &current.module_path {
                    (module_path.metadata()?, module_path)
                } else {
                    bail!("cannot mount root dir {}!", path.display());
                };
                chmod(&work_dir_path, Mode::from_raw_mode(metadata.mode()))?;
                chown(
                    &work_dir_path,
                    Some(Uid::from_raw(metadata.uid())),
                    Some(Gid::from_raw(metadata.gid())),
                )?;
                lsetfilecon(&work_dir_path, lgetfilecon(path)?.as_str())?;
            }

            if create_tmpfs {
                log::debug!(
                    "creating tmpfs for {} at {}",
                    path.display(),
                    work_dir_path.display()
                );
                mount_bind(&work_dir_path, &work_dir_path)
                    .context("bind self")
                    .with_context(|| {
                        format!(
                            "creating tmpfs for {} at {}",
                            path.display(),
                            work_dir_path.display(),
                        )
                    })?;
            }

            if path.exists() && !current.replace {
                for entry in path.read_dir()?.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    let result = if let Some(node) = current.children.remove(&name) {
                        if node.skip {
                            continue;
                        }
                        do_magic_mount(&path, &work_dir_path, node, has_tmpfs)
                            .with_context(|| format!("magic mount {}/{name}", path.display()))
                    } else if has_tmpfs {
                        mount_mirror(&path, &work_dir_path, &entry)
                            .with_context(|| format!("mount mirror {}/{name}", path.display()))
                    } else {
                        Ok(())
                    };

                    if let Err(e) = result {
                        if has_tmpfs {
                            return Err(e);
                        }
                        log::error!("mount child {}/{name} failed: {e:#?}", path.display());
                    }
                }
            }

            if current.replace {
                if current.module_path.is_none() {
                    bail!(
                        "dir {} is declared as replaced but it is root!",
                        path.display()
                    );
                }
                log::debug!("dir {} is replaced", path.display());
            }

            for (name, node) in current.children {
                if node.skip {
                    continue;
                }
                if let Err(e) = do_magic_mount(&path, &work_dir_path, node, has_tmpfs)
                    .with_context(|| format!("magic mount {}/{name}", path.display()))
                {
                    if has_tmpfs {
                        return Err(e);
                    }
                    log::error!("mount child {}/{name} failed: {e:#?}", path.display());
                }
            }

            if create_tmpfs {
                log::debug!(
                    "moving tmpfs {} -> {}",
                    work_dir_path.display(),
                    path.display()
                );
                if let Err(e) =
                    mount_remount(&work_dir_path, MountFlags::RDONLY | MountFlags::BIND, "")
                {
                    log::warn!("make dir {} ro: {e:#?}", path.display());
                }
                mount_move(&work_dir_path, &path)
                    .context("move self")
                    .with_context(|| {
                        format!(
                            "moving tmpfs {} -> {}",
                            work_dir_path.display(),
                            path.display()
                        )
                    })?;
                // make private to reduce peer group count
                if let Err(e) = mount_change(&path, MountPropagationFlags::PRIVATE) {
                    log::warn!("make dir {} private: {e:#?}", path.display());
                }
                if UMOUNT.load(std::sync::atomic::Ordering::Relaxed) {
                    // tell ksu about this one too
                    let _ = send_unmountable(path);
                }
            }
        }
        NodeFileType::Whiteout => {
            log::debug!("file {} is removed", path.display());
        }
    }

    Ok(())
}

pub fn magic_mount<P>(
    tmp_path: P,
    module_dir: &Path,
    mount_source: &str,
    extra_partitions: &[String],
) -> Result<()>
where
    P: AsRef<Path>,
{
    if let Some(root) = collect_module_files(module_dir, extra_partitions)? {
        log::debug!("collected: {root}");

        let tmp_root = tmp_path.as_ref();
        let tmp_dir = tmp_root.join("workdir");
        ensure_dir_exists(&tmp_dir)?;

        mount(mount_source, &tmp_dir, "tmpfs", MountFlags::empty(), None).context("mount tmp")?;
        mount_change(&tmp_dir, MountPropagationFlags::PRIVATE).context("make tmp private")?;

        let result = do_magic_mount(Path::new("/"), tmp_dir.as_path(), root, false);

        if let Err(e) = unmount(&tmp_dir, UnmountFlags::DETACH) {
            log::error!("failed to unmount tmp {e}");
        }
        fs::remove_dir(tmp_dir).ok();

        result
    } else {
        log::info!("no modules to mount, skipping!");
        Ok(())
    }
}
