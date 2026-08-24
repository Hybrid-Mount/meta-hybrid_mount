// SPDX-License-Identifier: GPL-3.0-only

//! Magic Mount 挂载执行(仅 Linux/Android)。
//!
//! 语义对齐参考项目 `8b85c9e`:
//! - 文件直接 bind + 只读 remount;符号链接克隆到 staging;
//! - 目录在需要挂子项或 `replace` 时建立 tmpfs skeleton,
//!   mirror 原目录的其余条目,最后只读 remount 并 mount-move 到目标;
//! - whiteout 只记录不挂载;所有写入都发生在私有随机 tmpfs staging,
//!   模块源目录只读。

use std::fs::{self, DirEntry};
use std::os::unix::fs::{MetadataExt, symlink};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use rustix::fs::{Gid, Mode, Uid, chmod, chown};
use rustix::mount::{
    MountFlags, MountPropagationFlags, mount, mount_bind, mount_change, mount_move, mount_remount,
};

use crate::errors::{Error, Result};
use crate::magic_mount::node::{Node, NodeFileType};
use crate::magic_mount::scan::{ScanOptions, collect_module_files};
use crate::utils::{ensure_dir_exists, lgetfilecon, lsetfilecon};

static MOUNTED_FILES: AtomicU32 = AtomicU32::new(0);
static IGNORED_FILES: AtomicU32 = AtomicU32::new(0);
static MOUNTED_SYMLINKS: AtomicU32 = AtomicU32::new(0);

pub struct MagicMount {
    node: Node,
    path: PathBuf,
    work_dir_path: PathBuf,
    has_tmpfs: bool,
    umount: bool,
}

impl MagicMount {
    pub fn new(
        node: &Node,
        path: &Path,
        work_dir_path: &Path,
        has_tmpfs: bool,
        umount: bool,
    ) -> Self {
        Self {
            node: node.clone(),
            path: path.join(&node.name),
            work_dir_path: work_dir_path.join(&node.name),
            has_tmpfs,
            umount,
        }
    }

    pub fn do_mount(&mut self) -> Result<()> {
        match self.node.file_type {
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

impl MagicMount {
    fn mount_symlink(&self) -> Result<()> {
        let Some(module_path) = &self.node.module_path else {
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

        MOUNTED_SYMLINKS.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    fn mount_regular_file(&self) -> Result<()> {
        let Some(module_path) = &self.node.module_path else {
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

        MOUNTED_FILES.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    fn mount_directory(&mut self) -> Result<()> {
        let mut tmpfs = !self.has_tmpfs && self.node.replace && self.node.module_path.is_some();

        if !self.has_tmpfs && !tmpfs {
            for (name, node) in &mut self.node.children {
                let real_path = self.path.join(name);
                let need = match node.file_type {
                    NodeFileType::Symlink => true,
                    NodeFileType::Whiteout => real_path.exists(),
                    _ => {
                        if let Ok(metadata) = real_path.symlink_metadata() {
                            let file_type = NodeFileType::from(metadata.file_type());
                            file_type != node.file_type || file_type == NodeFileType::Symlink
                        } else {
                            // 实际路径不存在:必须用 tmpfs 承载新文件。
                            true
                        }
                    }
                };

                if need {
                    if self.node.module_path.is_none() {
                        log::error!(
                            "cannot create tmpfs on {}, ignored child: {name}",
                            self.path.display()
                        );
                        IGNORED_FILES.fetch_add(1, Ordering::Relaxed);
                        node.skip = true;
                        continue;
                    }
                    tmpfs = true;
                    break;
                }
            }
        }
        let has_tmpfs = tmpfs || self.has_tmpfs;

        if has_tmpfs {
            tmpfs_skeleton(&self.path, &self.work_dir_path, &self.node)?;
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

        if self.path.exists() && !self.node.replace {
            self.mount_path(has_tmpfs)?;
        }

        if self.node.replace {
            if self.node.module_path.is_none() {
                return Err(Error::DirDeclared {
                    path: self.path.display().to_string(),
                });
            }
            log::debug!("dir {} is replaced", self.path.display());
        }

        for (name, node) in &self.node.children {
            if node.skip {
                continue;
            }

            let result = MagicMount::new(
                node,
                &self.path,
                &self.work_dir_path,
                has_tmpfs,
                self.umount,
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
    fn mount_path(&mut self, has_tmpfs: bool) -> Result<()> {
        for entry in self.path.read_dir()?.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();

            let result = if let Some(node) = self.node.children.remove(&name) {
                if node.skip {
                    continue;
                }

                MagicMount::new(
                    &node,
                    &self.path,
                    &self.work_dir_path,
                    has_tmpfs,
                    self.umount,
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

        Ok(())
    }
}

/// magic mount 一次执行的统计(供 `run/state.json` 快照)。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MagicMountStats {
    pub mounted_files: u32,
    pub mounted_symlinks: u32,
    pub ignored_files: u32,
}

/// 完整 magic mount 入口:扫描 → 建 staging tmpfs → 执行 → 汇总。
pub fn magic_mount(
    module_dir: &Path,
    mount_source: &str,
    work_dir: &Path,
    options: &ScanOptions<'_>,
    umount: bool,
) -> Result<MagicMountStats> {
    let Some(root) = collect_module_files(module_dir, options)? else {
        log::info!("no modules selected for magic mount, skipping");
        return Ok(MagicMountStats::default());
    };

    log::debug!("collected: {root:?}");

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

    MagicMount::new(&root, Path::new("/"), work_dir, false, umount).do_mount()?;

    let files = MOUNTED_FILES.load(Ordering::Relaxed);
    let symlinks = MOUNTED_SYMLINKS.load(Ordering::Relaxed);
    log::info!("mounted files: {files}, mounted symlinks: {symlinks}");

    Ok(MagicMountStats {
        mounted_files: files,
        mounted_symlinks: symlinks,
        ignored_files: IGNORED_FILES.load(Ordering::Relaxed),
    })
}

/// 按真实路径(存在时)或模块源路径复制 mode/uid/gid/SELinux 到 staging。
fn tmpfs_skeleton(path: &Path, work_dir_path: &Path, node: &Node) -> Result<()> {
    log::debug!(
        "creating tmpfs skeleton for {} at {}",
        path.display(),
        work_dir_path.display()
    );

    fs::create_dir_all(work_dir_path)?;

    let (metadata, reference) = if path.exists() {
        (path.metadata()?, path.to_path_buf())
    } else if let Some(module_path) = &node.module_path {
        (module_path.metadata()?, module_path.clone())
    } else {
        return Err(Error::MountRootFile {
            path: path.display().to_string(),
        });
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

        for child in path.read_dir()?.flatten() {
            mount_mirror(&path, &work_dir_path, &child)?;
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
        target.display(),
        target.display(),
        link.display()
    );
    Ok(())
}
