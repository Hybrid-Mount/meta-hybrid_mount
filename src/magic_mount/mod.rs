mod node;
#[cfg(any(target_os = "linux", target_os = "android"))]
mod try_umount;
mod utils;

use std::{
    fs::{self, DirEntry, create_dir, read_dir, read_link},
    os::unix::fs::{MetadataExt, symlink},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use rustix::{
    fs::{Gid, Mode, Uid, chmod, chown},
    mount::{
        MountFlags, MountPropagationFlags, UnmountFlags, mount, mount_bind, mount_change,
        mount_remount, unmount,
    },
    path::Arg,
};

use crate::{
    defs::{DISABLE_FILE_NAME, REMOVE_FILE_NAME, SKIP_MOUNT_FILE_NAME},
    magic_mount::node::{Node, NodeFileType},
    utils::{ensure_dir_exists, lgetfilecon, lsetfilecon},
};

#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::magic_mount::try_umount::send_unmountable;

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
            log::debug!(
                "mount module file {} -> {}",
                module_path.display(),
                self.work_dir_path.display()
            );
            mount_bind(module_path, target_path).with_context(|| {
                #[cfg(any(target_os = "linux", target_os = "android"))]
                if self.umount {
                    // tell ksu about this mount
                    let _ = send_unmountable(target_path);
                }
                format!(
                    "mount module file {} -> {}",
                    module_path.display(),
                    self.work_dir_path.display(),
                )
            })?;
            // we should use MS_REMOUNT | MS_BIND | MS_xxx to change mount flags
            if let Err(e) = mount_remount(target_path, MountFlags::RDONLY | MountFlags::BIND, "") {
                log::warn!("make file {} ro: {e:#?}", target_path.display());
            }
            Ok(())
        } else {
            bail!("cannot mount root file {}!", self.path.display());
        }
    }

    fn handle_directory(&mut self) -> Result<()> {
        let mut create_tmpfs =
            !self.has_tmpfs && self.node.replace && self.node.module_path.is_some();

        if !self.has_tmpfs && !create_tmpfs {
            self.check_tmpfs();
            create_tmpfs = self.has_tmpfs;
        }

        let has_tmpfs = self.has_tmpfs || create_tmpfs;

        if has_tmpfs {
            self.creatimg_tmpfs_skeleton()?;
        }

        if create_tmpfs {
            log::debug!(
                "creating tmpfs for {} at {}",
                self.path.display(),
                self.work_dir_path.display()
            );

            mount_bind(&self.work_dir_path, &self.work_dir_path)
                .context("bind self")
                .with_context(|| {
                    format!(
                        "creating tmpfs for {} at {}",
                        self.path.display(),
                        self.work_dir_path.display(),
                    )
                })?;
        }

        if self.path.exists() && !self.node.replace {
            for entry in self.path.read_dir()?.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                let result = {
                    if let Some(node) = self.node.children.remove(&name) {
                        if node.skip {
                            continue;
                        }

                        Self::new(
                            &node,
                            &self.path,
                            &self.work_dir_path,
                            has_tmpfs,
                            #[cfg(any(target_os = "linux", target_os = "android"))]
                            self.umount,
                        )
                        .do_magic_mount()
                        .with_context(|| format!("magic mount {}/{name}", self.path.display()))
                    } else if has_tmpfs {
                        mount_mirror(&self.path, &self.work_dir_path, &entry)
                            .with_context(|| format!("mount mirror {}/{name}", self.path.display()))
                    } else {
                        Ok(())
                    }
                };

                if let Err(e) = result {
                    if has_tmpfs {
                        return Err(e);
                    }
                    log::error!("mount child {}/{name} failed: {e:#?}", self.path.display());
                }
            }
        }

        if self.node.replace {
            if self.node.module_path.is_none() {
                bail!(
                    "dir {} is declared as replaced but it is root!",
                    self.path.display()
                );
            }

            log::debug!("dir {} is replaced", self.path.display());
        }

        for (name, node) in &self.node.children {
            if node.skip {
                continue;
            }

            if let Err(e) = Self::new(
                node,
                &self.path,
                &self.work_dir_path,
                has_tmpfs,
                #[cfg(any(target_os = "linux", target_os = "android"))]
                self.umount,
            )
            .do_magic_mount()
            .with_context(|| format!("magic mount {}/{name}", self.path.display()))
            {
                if has_tmpfs {
                    return Err(e);
                }

                log::error!("mount child {}/{name} failed: {e:#?}", self.path.display());
            }
        }

        if create_tmpfs {
            self.moving_tmpfs()?;
        }
        Ok(())
    }
    fn handle_symlink(&self) -> Result<()> {
        if let Some(module_path) = &self.node.module_path {
            log::debug!(
                "create module symlink {} -> {}",
                module_path.display(),
                self.work_dir_path.display()
            );
            clone_symlink(module_path, &self.work_dir_path).with_context(|| {
                format!(
                    "create module symlink {} -> {}",
                    module_path.display(),
                    self.work_dir_path.display(),
                )
            })?;
            Ok(())
        } else {
            bail!("cannot mount root symlink {}!", self.path.display());
        }
    }
}

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

pub fn magic_mount<P>(
    tmp_path: P,
    module_dir: &Path,
    mount_source: &str,
    extra_partitions: &[String],
    #[cfg(any(target_os = "linux", target_os = "android"))] umount: bool,
    #[cfg(not(any(target_os = "linux", target_os = "android")))] _umount: bool,
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

        let result = {
            MagicMount::new(
                &root,
                Path::new("/"),
                tmp_dir.as_path(),
                false,
                #[cfg(any(target_os = "linux", target_os = "android"))]
                umount,
            )
            .do_magic_mount()
        };

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
