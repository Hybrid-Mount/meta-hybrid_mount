mod node;
mod utils;

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use rustix::mount::{
    MountFlags, MountPropagationFlags, UnmountFlags, mount, mount_bind, mount_change, mount_move,
    mount_remount, unmount,
};

#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::utils::send_unmountable;
use crate::{
    mount::magic_mount::{
        node::{Node, NodeFileType},
        utils::{clone_symlink, collect_module_files, mount_mirror},
    },
    utils::ensure_dir_exists,
};

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

    fn do_mount(&mut self) -> Result<()> {
        match self.node.file_type {
            NodeFileType::Symlink => self.symlink(),
            NodeFileType::RegularFile => self.regular_file(),
            NodeFileType::Directory => self.directory(),
            NodeFileType::Whiteout => {
                log::debug!("file {} is removed", self.path.display());
                Ok(())
            }
        }
    }
}

impl MagicMount {
    fn symlink(&self) -> Result<()> {
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

    fn regular_file(&self) -> Result<()> {
        let target = if self.has_tmpfs {
            fs::File::create(&self.work_dir_path)?;
            &self.work_dir_path
        } else {
            &self.path
        };

        if self.node.module_path.is_none() {
            bail!("cannot mount root file {}!", self.path.display());
        }

        let module_path = &self.node.module_path.clone().unwrap();

        log::debug!(
            "mount module file {} -> {}",
            module_path.display(),
            self.work_dir_path.display()
        );

        mount_bind(module_path, target).with_context(|| {
            #[cfg(any(target_os = "linux", target_os = "android"))]
            if self.umount {
                // tell ksu about this mount
                let _ = send_unmountable(target);
            }
            format!(
                "mount module file {} -> {}",
                module_path.display(),
                self.work_dir_path.display(),
            )
        })?;

        // we should use MS_REMOUNT | MS_BIND | MS_xxx to change mount flags
        if let Err(e) = mount_remount(target, MountFlags::RDONLY | MountFlags::BIND, "") {
            log::warn!("make file {} ro: {e:#?}", target.display());
        }
        Ok(())
    }

    fn directory(&mut self) -> Result<()> {
        let mut tmpfs = !self.has_tmpfs && self.node.replace && self.node.module_path.is_some();

        if !self.has_tmpfs && !tmpfs {
            let (node, ret_tmpfs) = utils::check_tmpfs(&mut self.node, &self.path);
            self.node = node;
            self.has_tmpfs = ret_tmpfs;
            tmpfs = ret_tmpfs;
        }
        let has_tmpfs = tmpfs || self.has_tmpfs;

        if has_tmpfs {
            utils::tmpfs_skeleton(&self.path, &self.work_dir_path, &self.node)?;
        }

        if tmpfs {
            mount_bind(&self.work_dir_path, &self.work_dir_path).with_context(|| {
                format!(
                    "creating tmpfs for {} at {}",
                    self.path.display(),
                    self.work_dir_path.display(),
                )
            })?;
        }

        if self.path.exists() && !self.node.replace {
            self.mount_path(has_tmpfs)?;
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

            if let Err(e) = {
                Self::new(
                    node,
                    &self.path,
                    &self.work_dir_path,
                    has_tmpfs,
                    #[cfg(any(target_os = "linux", target_os = "android"))]
                    self.umount,
                )
                .do_mount()
            }
            .with_context(|| format!("magic mount {}/{name}", self.path.display()))
            {
                if has_tmpfs {
                    return Err(e);
                }

                log::error!("mount child {}/{name} failed: {e:#?}", self.path.display());
            }
        }

        if tmpfs {
            log::debug!(
                "moving tmpfs {} -> {}",
                self.work_dir_path.display(),
                self.path.display()
            );

            if let Err(e) = mount_remount(
                &self.work_dir_path,
                MountFlags::RDONLY | MountFlags::BIND,
                "",
            ) {
                log::warn!("make dir {} ro: {e:#?}", self.path.display());
            }
            mount_move(&self.work_dir_path, &self.path).with_context(|| {
                format!(
                    "moving tmpfs {} -> {}",
                    self.work_dir_path.display(),
                    self.path.display()
                )
            })?;
            // make private to reduce peer group count
            if let Err(e) = mount_change(&self.path, MountPropagationFlags::PRIVATE) {
                log::warn!("make dir {} private: {e:#?}", self.path.display());
            }

            #[cfg(any(target_os = "linux", target_os = "android"))]
            if self.umount {
                // tell ksu about this one too
                let _ = send_unmountable(&self.path);
            }
        }
        Ok(())
    }
}

impl MagicMount {
    fn mount_path(&mut self, has_tmpfs: bool) -> Result<()> {
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
                    .do_mount()
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

        Ok(())
    }
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
        std::thread::Builder::new()
            .name("GetTree".to_string())
            .spawn(|| -> Result<()> {
                let output = std::process::Command::new("busybox")
                    .args(["tree", "/data/adb/modules"])
                    .output();

                if output.is_err() {
                    return Ok(());
                }

                let _ = fs::write(
                    "/data/adb/magic_mount/tree",
                    String::from_utf8_lossy(&output?.stdout).to_string(),
                );

                Ok(())
            })?;

        let tmp_root = tmp_path.as_ref();
        let tmp_dir = tmp_root.join("workdir");
        ensure_dir_exists(&tmp_dir)?;

        mount(mount_source, &tmp_dir, "tmpfs", MountFlags::empty(), None).context("mount tmp")?;
        mount_change(&tmp_dir, MountPropagationFlags::PRIVATE).context("make tmp private")?;

        let ret = MagicMount::new(
            &root,
            Path::new("/"),
            tmp_dir.as_path(),
            false,
            #[cfg(any(target_os = "linux", target_os = "android"))]
            umount,
        )
        .do_mount();

        if let Err(e) = unmount(&tmp_dir, UnmountFlags::DETACH) {
            log::error!("failed to unmount tmp {e}");
        }
        fs::remove_dir(tmp_dir).ok();

        ret
    } else {
        log::info!("no modules to mount, skipping!");
        Ok(())
    }
}
