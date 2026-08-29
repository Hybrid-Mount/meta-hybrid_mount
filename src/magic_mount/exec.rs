// SPDX-License-Identifier: GPL-3.0-only

//! Magic Mount 挂载执行(仅 Linux/Android)。
//!
//! 语义对齐参考项目 `8b85c9e`:
//! - 文件直接 bind + 只读 remount;符号链接克隆到 staging;
//! - 目录在需要挂子项或 `replace` 时建立 tmpfs skeleton,
//!   mirror 原目录的其余条目,最后只读 remount 并 mount-move 到目标;
//! - whiteout 只记录不挂载;所有写入都发生在私有随机 tmpfs staging,
//!   模块源目录只读。

use std::collections::BTreeSet;
use std::fs::{self, DirEntry};
use std::os::unix::fs::{MetadataExt, symlink};
use std::path::{Path, PathBuf};

use rustix::fs::{Gid, Mode, Uid, chmod, chown};
use rustix::mount::{
    MountFlags, MountPropagationFlags, mount, mount_bind, mount_change, mount_move, mount_remount,
};

use crate::config::Mode as MountMode;
use crate::errors::{Error, Result};
use crate::mount_tree::{MountNode, MountTree, NodeFileType};
use crate::utils::{ensure_dir_exists, lgetfilecon, lsetfilecon};

pub struct MagicMount<'tree, 'stats, 'mount> {
    node: &'tree MountNode,
    path: PathBuf,
    work_dir_path: PathBuf,
    has_tmpfs: bool,
    umount: bool,
    stats: &'stats mut MagicMountStats,
    on_mount: &'mount mut dyn FnMut(&str),
}

impl<'tree, 'stats, 'mount> MagicMount<'tree, 'stats, 'mount> {
    pub fn new(
        node: &'tree MountNode,
        path: &Path,
        work_dir_path: &Path,
        has_tmpfs: bool,
        umount: bool,
        stats: &'stats mut MagicMountStats,
        on_mount: &'mount mut dyn FnMut(&str),
    ) -> Self {
        Self {
            node,
            path: path.join(&node.name),
            work_dir_path: work_dir_path.join(&node.name),
            has_tmpfs,
            umount,
            stats,
            on_mount,
        }
    }

    pub fn do_mount(&mut self) -> Result<()> {
        if crate::sys::faults::should_fail_next_magic_mount() {
            return Err(Error::msg(format!(
                "injected magic mount failure: target={}",
                self.path.display()
            )));
        }
        let file_type = self.node.file_type_for(MountMode::Magic).ok_or_else(|| {
            Error::msg(format!(
                "magic node has no selected source: {}",
                self.path.display()
            ))
        })?;
        match file_type {
            NodeFileType::Symlink => self.mount_symlink(),
            NodeFileType::RegularFile => self.mount_regular_file(),
            NodeFileType::Directory => self.mount_directory(),
            NodeFileType::Whiteout => {
                log::debug!("file {} is removed", self.path.display());
                Ok(())
            }
        }
    }
}

impl MagicMount<'_, '_, '_> {
    fn mount_symlink(&mut self) -> Result<()> {
        let Some(module_path) = self.node.module_path_for(MountMode::Magic) else {
            return Err(Error::MountRootSymlink {
                path: self.path.display().to_string(),
            });
        };

        log::debug!(
            "create module symlink {} -> {}",
            module_path.display(),
            self.work_dir_path.display()
        );
        clone_symlink(module_path, &self.work_dir_path).map_err(|err| {
            Error::msg(format!(
                "create module symlink {} -> {}: {err}",
                module_path.display(),
                self.work_dir_path.display()
            ))
        })?;

        self.stats.mounted_symlinks = self.stats.mounted_symlinks.saturating_add(1);
        Ok(())
    }

    fn mount_regular_file(&mut self) -> Result<()> {
        let Some(module_path) = self.node.module_path_for(MountMode::Magic) else {
            return Err(Error::MountRootFile {
                path: self.path.display().to_string(),
            });
        };

        let target = if self.has_tmpfs {
            fs::File::create(&self.work_dir_path)?;
            self.work_dir_path.as_path()
        } else {
            self.path.as_path()
        };

        log::debug!(
            "mount module file {} -> {}",
            module_path.display(),
            target.display()
        );
        mount_bind(module_path, target).map_err(|err| {
            Error::msg(format!(
                "mount module file {} -> {}: {err}",
                module_path.display(),
                target.display()
            ))
        })?;

        if self.umount && !self.work_dir_path.starts_with("/mnt") {
            crate::utils::ksu::send_unmountable(target);
        }

        // MS_REMOUNT | MS_BIND 组合用于把单文件改成只读;失败只告警不中断。
        if let Err(err) = mount_remount(target, MountFlags::RDONLY | MountFlags::BIND, "") {
            log::warn!("make file {} read-only: {err}", target.display());
        }

        self.stats.mounted_files = self.stats.mounted_files.saturating_add(1);
        let target = self.path.to_string_lossy().into_owned();
        self.stats.active_mounts.push(target.clone());
        (self.on_mount)(&target);
        Ok(())
    }

    fn mount_directory(&mut self) -> Result<()> {
        let replace = self.node.replace_for(MountMode::Magic);
        let module_path = self.node.module_path_for(MountMode::Magic);
        let mut tmpfs = !self.has_tmpfs && replace && module_path.is_some();
        let mut skipped_children = BTreeSet::new();

        if !self.has_tmpfs && !tmpfs {
            for (name, node) in self
                .node
                .children
                .iter()
                .filter(|(_, node)| node.has_backend(MountMode::Magic))
            {
                let real_path = self.path.join(name);
                let Some(node_type) = node.file_type_for(MountMode::Magic) else {
                    debug_assert!(false, "a filtered magic child must have a selected type");
                    log::error!("magic child has no selected type: {}", real_path.display());
                    skipped_children.insert(name.clone());
                    continue;
                };
                let need = match node_type {
                    NodeFileType::Symlink => true,
                    NodeFileType::Whiteout => match fs::symlink_metadata(&real_path) {
                        Ok(_) => true,
                        Err(err) if err.kind() == std::io::ErrorKind::NotFound => false,
                        Err(_) => true,
                    },
                    _ => {
                        if let Ok(metadata) = real_path.symlink_metadata() {
                            NodeFileType::from_file_type(metadata.file_type()).is_none_or(
                                |file_type| {
                                    file_type != node_type || file_type == NodeFileType::Symlink
                                },
                            )
                        } else {
                            // 实际路径不存在:必须用 tmpfs 承载新文件。
                            true
                        }
                    }
                };

                if need {
                    if module_path.is_none() {
                        log::error!(
                            "cannot create tmpfs on {}, ignored child: {name}",
                            self.path.display()
                        );
                        self.stats.ignored_files = self.stats.ignored_files.saturating_add(1);
                        skipped_children.insert(name.clone());
                        continue;
                    }
                    tmpfs = true;
                    break;
                }
            }
        }
        let has_tmpfs = tmpfs || self.has_tmpfs;

        if has_tmpfs {
            tmpfs_skeleton(&self.path, &self.work_dir_path, self.node)?;
        }

        if tmpfs {
            // 先自身 bind 一次,保证后续 mount-move 作用于这个挂载点。
            mount_bind(&self.work_dir_path, &self.work_dir_path).map_err(|err| {
                Error::msg(format!(
                    "creating tmpfs for {} at {}: {err}",
                    self.path.display(),
                    self.work_dir_path.display()
                ))
            })?;
        }

        let processed = if self.path.exists() && !replace {
            self.mount_path(has_tmpfs, &skipped_children)?
        } else {
            BTreeSet::new()
        };

        if replace {
            if module_path.is_none() {
                return Err(Error::DirDeclared {
                    path: self.path.display().to_string(),
                });
            }
            log::debug!("dir {} is replaced", self.path.display());
        }

        for (name, node) in self
            .node
            .children
            .iter()
            .filter(|(_, node)| node.has_backend(MountMode::Magic))
        {
            if processed.contains(name) || skipped_children.contains(name) {
                continue;
            }

            let result = MagicMount::new(
                node,
                &self.path,
                &self.work_dir_path,
                has_tmpfs,
                self.umount,
                &mut *self.stats,
                &mut *self.on_mount,
            )
            .do_mount();

            if let Err(err) = result {
                if has_tmpfs {
                    return Err(err);
                }
                log::error!(
                    "mount child {}/{} failed: {}",
                    self.path.display(),
                    name,
                    err
                );
            }
        }

        if tmpfs {
            log::debug!(
                "moving tmpfs {} -> {}",
                self.work_dir_path.display(),
                self.path.display()
            );

            if let Err(err) = mount_remount(
                &self.work_dir_path,
                MountFlags::RDONLY | MountFlags::BIND,
                "",
            ) {
                log::warn!("make dir {} read-only: {err}", self.path.display());
            }
            mount_move(&self.work_dir_path, &self.path).map_err(|err| {
                Error::msg(format!(
                    "moving tmpfs {} -> {}: {err}",
                    self.work_dir_path.display(),
                    self.path.display()
                ))
            })?;
            let target = self.path.to_string_lossy().into_owned();
            self.stats.active_mounts.push(target.clone());
            (self.on_mount)(&target);

            // 降为 private,减少 peer group 数量。
            if let Err(err) = mount_change(
                &self.path,
                MountPropagationFlags::PRIVATE | MountPropagationFlags::REC,
            ) {
                log::warn!("make dir {} private: {err}", self.path.display());
            }

            if self.umount {
                crate::utils::ksu::send_unmountable(&self.path);
            }
        }

        Ok(())
    }

    /// 处理实际目录中已有的条目:命中收集树的走 magic mount,
    /// 其余条目在 tmpfs 场景下 mirror 进 staging。
    fn mount_path(
        &mut self,
        has_tmpfs: bool,
        skipped_children: &BTreeSet<String>,
    ) -> Result<BTreeSet<String>> {
        let mut processed = BTreeSet::new();
        for entry in self.path.read_dir()? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();

            let result = if let Some(node) = self
                .node
                .children
                .get(&name)
                .filter(|node| node.has_backend(MountMode::Magic))
            {
                processed.insert(name.clone());
                if skipped_children.contains(&name) {
                    continue;
                }

                MagicMount::new(
                    node,
                    &self.path,
                    &self.work_dir_path,
                    has_tmpfs,
                    self.umount,
                    &mut *self.stats,
                    &mut *self.on_mount,
                )
                .do_mount()
            } else if has_tmpfs {
                mount_mirror(&self.path, &self.work_dir_path, &entry)
            } else {
                Ok(())
            };

            if let Err(err) = result {
                if has_tmpfs {
                    return Err(err);
                }
                log::error!(
                    "mount child {}/{} failed: {}",
                    self.path.display(),
                    name,
                    err
                );
            }
        }

        Ok(processed)
    }
}

/// magic mount 一次执行的统计(供 `run/state.json` 快照)。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MagicMountStats {
    pub mounted_files: u32,
    pub mounted_symlinks: u32,
    pub ignored_files: u32,
    /// Successful module-controlled bind and directory mount targets.
    pub active_mounts: Vec<String>,
}

/// 完整 magic mount 入口:消费共享树 → 建 staging tmpfs → 执行 → 汇总。
pub fn magic_mount(
    tree: &MountTree,
    mount_source: &str,
    work_dir: &Path,
    umount: bool,
    on_mount: &mut dyn FnMut(&str),
) -> Result<MagicMountStats> {
    if !tree.has_backend(MountMode::Magic) {
        log::info!("no modules selected for magic mount, skipping");
        return Ok(MagicMountStats::default());
    }

    log::debug!(
        "shared mount tree selected for magic execution: {:?}",
        tree.root
    );

    ensure_dir_exists(work_dir)?;

    mount(mount_source, work_dir, "tmpfs", MountFlags::empty(), None).map_err(|err| {
        Error::msg(format!(
            "mount tmpfs {mount_source} at {}: {err}",
            work_dir.display()
        ))
    })?;
    mount_change(
        work_dir,
        MountPropagationFlags::PRIVATE | MountPropagationFlags::REC,
    )
    .map_err(|err| Error::msg(format!("make {} private: {err}", work_dir.display())))?;

    let mut stats = MagicMountStats::default();
    MagicMount::new(
        &tree.root,
        Path::new("/"),
        work_dir,
        false,
        umount,
        &mut stats,
        on_mount,
    )
    .do_mount()?;

    stats.active_mounts.sort();
    stats.active_mounts.dedup();
    log::info!(
        "mounted files: {}, mounted symlinks: {}, active targets: {}",
        stats.mounted_files,
        stats.mounted_symlinks,
        stats.active_mounts.len()
    );

    Ok(stats)
}

/// 按真实路径(存在时)或模块源路径复制 mode/uid/gid/SELinux 到 staging。
fn tmpfs_skeleton(path: &Path, work_dir_path: &Path, node: &MountNode) -> Result<()> {
    log::debug!(
        "creating tmpfs skeleton for {} at {}",
        path.display(),
        work_dir_path.display()
    );

    fs::create_dir_all(work_dir_path)?;

    let (metadata, reference) = match path.metadata() {
        Ok(metadata) => (metadata, path.to_path_buf()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            let Some(module_path) = node.module_path_for(MountMode::Magic) else {
                return Err(Error::MountRootFile {
                    path: path.display().to_string(),
                });
            };
            (module_path.metadata()?, module_path.to_path_buf())
        }
        Err(err) => return Err(err.into()),
    };

    chmod(work_dir_path, Mode::from_raw_mode(metadata.mode()))?;
    chown(
        work_dir_path,
        Some(Uid::from_raw(metadata.uid())),
        Some(Gid::from_raw(metadata.gid())),
    )?;
    lsetfilecon(work_dir_path, &lgetfilecon(&reference)?)?;
    Ok(())
}

/// 把真实目录中未被收集树覆盖的条目递归 mirror 到 tmpfs staging。
fn mount_mirror(path: &Path, work_dir_path: &Path, entry: &DirEntry) -> Result<()> {
    let path = path.join(entry.file_name());
    let work_dir_path = work_dir_path.join(entry.file_name());
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
        fs::create_dir(&work_dir_path)?;
        let metadata = entry.metadata()?;
        chmod(&work_dir_path, Mode::from_raw_mode(metadata.mode()))?;
        chown(
            &work_dir_path,
            Some(Uid::from_raw(metadata.uid())),
            Some(Gid::from_raw(metadata.gid())),
        )?;
        lsetfilecon(&work_dir_path, &lgetfilecon(&path)?)?;

        for child in path.read_dir()? {
            mount_mirror(&path, &work_dir_path, &child?)?;
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

fn clone_symlink(source: &Path, target: &Path) -> Result<()> {
    let link = fs::read_link(source)?;
    symlink(&link, target)?;
    lsetfilecon(target, &lgetfilecon(source)?)?;
    log::debug!(
        "clone symlink {} -> {}({})",
        source.display(),
        target.display(),
        link.display()
    );
    Ok(())
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use crate::module_id::ModuleId;
    use crate::mount_tree::{MountSource, MountTree};

    #[test]
    fn injected_magic_mount_failure_fires_before_side_effects() {
        let root =
            std::env::temp_dir().join(format!("hybrid-mount-magic-fault-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let source = root.join("source");
        fs::write(&source, "data").unwrap();

        let mut tree = MountTree::default();
        tree.insert(
            "hosts",
            MountSource {
                module_id: ModuleId::try_from("m").unwrap(),
                relative: "hosts".to_owned(),
                source_path: source,
                file_type: NodeFileType::RegularFile,
                replace: false,
                backend: MountMode::Magic,
            },
        );
        let node = tree.root.children.get("hosts").unwrap();
        let mut stats = MagicMountStats::default();
        let mut calls = 0;
        let mut on_mount = |_: &str| calls += 1;

        crate::sys::faults::enable_next_magic_mount_failure();
        let mut mount =
            MagicMount::new(node, &root, &root, false, false, &mut stats, &mut on_mount);
        let err = mount.do_mount().unwrap_err();
        crate::sys::faults::reset();

        assert!(err.to_string().contains("injected magic mount"), "{err}");
        assert_eq!(calls, 0);
        assert_eq!(stats.mounted_files, 0);
        fs::remove_dir_all(&root).ok();
    }
}
