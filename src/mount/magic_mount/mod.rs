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

mod utils;

use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::{Path, PathBuf},
};
#[cfg(not(any(target_os = "linux", target_os = "android")))]
use std::{ffi::CStr, ops::BitOr};

use anyhow::{Context, Result, bail};
#[cfg(any(target_os = "linux", target_os = "android"))]
use rustix::mount::{
    MountFlags, MountPropagationFlags, UnmountFlags, mount, mount_change, mount_move,
    mount_remount, unmount,
};

use self::utils::mount_bind;

#[cfg(not(any(target_os = "linux", target_os = "android")))]
#[derive(Clone, Copy)]
struct MountFlags;

#[cfg(not(any(target_os = "linux", target_os = "android")))]
impl MountFlags {
    const RDONLY: Self = Self;
    const BIND: Self = Self;

    fn empty() -> Self {
        Self
    }
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
impl BitOr for MountFlags {
    type Output = Self;

    fn bitor(self, _rhs: Self) -> Self::Output {
        Self
    }
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
#[derive(Clone, Copy)]
struct MountPropagationFlags;

#[cfg(not(any(target_os = "linux", target_os = "android")))]
impl MountPropagationFlags {
    const PRIVATE: Self = Self;
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
#[derive(Clone, Copy)]
struct UnmountFlags;

#[cfg(not(any(target_os = "linux", target_os = "android")))]
impl UnmountFlags {
    const DETACH: Self = Self;
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn mount<P, Q>(
    _source: P,
    _target: Q,
    _fstype: &str,
    _flags: MountFlags,
    _data: Option<&CStr>,
) -> Result<()>
where
    P: AsRef<Path>,
    Q: AsRef<Path>,
{
    bail!("mount is only supported on linux/android")
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn mount_change<P>(_target: P, _flags: MountPropagationFlags) -> Result<()>
where
    P: AsRef<Path>,
{
    bail!("mount propagation changes are only supported on linux/android")
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn mount_move<P, Q>(_source: P, _target: Q) -> Result<()>
where
    P: AsRef<Path>,
    Q: AsRef<Path>,
{
    bail!("mount move is only supported on linux/android")
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn mount_remount<P>(_target: P, _flags: MountFlags, _data: &str) -> Result<()>
where
    P: AsRef<Path>,
{
    bail!("mount remount is only supported on linux/android")
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn unmount<P>(_target: P, _flags: UnmountFlags) -> Result<()>
where
    P: AsRef<Path>,
{
    bail!("unmount is only supported on linux/android")
}

#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::mount::umount_mgr::send_umountable;
use crate::{
    core::{inventory::Module, recovery::ModuleStageFailure, runtime_state::MountStatistics},
    mount::{
        magic_mount::utils::{clone_symlink, collect_module_files, mount_mirror},
        node::{Node, NodeFileType},
    },
    sys::fs::ensure_dir_exists,
};

fn try_remount_readonly(mount_target: &Path, log_path: &Path) {
    if let Err(e) = mount_remount(mount_target, MountFlags::RDONLY | MountFlags::BIND, "") {
        crate::scoped_log!(
            warn,
            "magic",
            "remount readonly failed: path={}, error={:#?}",
            log_path.display(),
            e
        );
    }
}

fn collect_module_ids(node: &Node, ids: &mut HashSet<String>) {
    if let Some(module_path) = &node.module_path
        && let Some(module_id) = crate::utils::extract_module_id(module_path)
    {
        ids.insert(module_id);
    }

    for child in node.children.values() {
        collect_module_ids(child, ids);
    }
}

fn infer_module_ids(node: &Node) -> Vec<String> {
    let mut ids = HashSet::new();
    collect_module_ids(node, &mut ids);
    let mut module_ids: Vec<String> = ids.into_iter().collect();
    module_ids.sort();
    module_ids
}

fn wrap_with_module_context(err: anyhow::Error, node: &Node) -> anyhow::Error {
    let module_ids = infer_module_ids(node);
    if module_ids.is_empty() {
        err
    } else {
        ModuleStageFailure::execute(module_ids, err).into()
    }
}

#[derive(Debug, Default)]
struct MountContext {
    stats: MountStatistics,
    failed_module_ids: HashSet<String>,
    symlinks_by_module: BTreeMap<String, usize>,
}

impl MountContext {
    fn record_failed_node(&mut self, node: &Node) {
        self.stats.record_failed();
        self.failed_module_ids.extend(infer_module_ids(node));
    }

    fn record_symlink(&mut self, module_path: &Path) {
        self.stats.record_symlink();
        let module_id =
            crate::utils::extract_module_id(module_path).unwrap_or_else(|| "<unknown>".to_string());
        *self.symlinks_by_module.entry(module_id).or_default() += 1;
    }
}

pub struct MagicMountOptions<'a> {
    pub mount_source: &'a str,
    pub managed_partitions: &'a [String],
    pub use_kasumi: bool,
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
            path: path.as_ref().join(&node.name),
            work_dir_path: work_dir_path.as_ref().join(&node.name),
            node: node.clone(),
            has_tmpfs,
            #[cfg(any(target_os = "linux", target_os = "android"))]
            umount,
        }
    }

    fn do_mount(&mut self, context: &mut MountContext) -> Result<()> {
        match self.node.file_type {
            NodeFileType::Symlink => self.symlink(context),
            NodeFileType::RegularFile => self.regular_file(context),
            NodeFileType::Directory => self.directory(context),
            NodeFileType::Whiteout => {
                crate::scoped_log!(debug, "magic", "whiteout: path={}", self.path.display());
                Ok(())
            }
        }
    }
}

impl MagicMount {
    fn symlink(&self, context: &mut MountContext) -> Result<()> {
        if let Some(module_path) = &self.node.module_path {
            clone_symlink(module_path, &self.work_dir_path).with_context(|| {
                format!(
                    "create module symlink {} -> {}",
                    module_path.display(),
                    self.work_dir_path.display(),
                )
            })?;
            context.record_symlink(module_path);
            Ok(())
        } else {
            bail!("cannot mount root symlink {}!", self.path.display());
        }
    }

    fn regular_file(&self, context: &mut MountContext) -> Result<()> {
        let target = if self.has_tmpfs {
            fs::File::create(&self.work_dir_path)?;
            &self.work_dir_path
        } else {
            &self.path
        };

        let Some(module_path) = self.node.module_path.as_ref() else {
            bail!("cannot mount root file {}!", self.path.display());
        };

        crate::scoped_log!(
            debug,
            "magic",
            "mount file: src={}, dst={}",
            module_path.display(),
            self.work_dir_path.display()
        );

        mount_bind(module_path, target).with_context(|| {
            format!(
                "mount module file {} -> {}",
                module_path.display(),
                self.work_dir_path.display(),
            )
        })?;

        #[cfg(any(target_os = "linux", target_os = "android"))]
        if self.umount
            && let Err(e) = send_umountable(target)
        {
            crate::scoped_log!(
                warn,
                "magic",
                "failed to register umountable at {}: {:#}",
                target.display(),
                e
            );
        }

        try_remount_readonly(target, target);
        context.stats.record_file();
        Ok(())
    }

    fn directory(&mut self, context: &mut MountContext) -> Result<()> {
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
                            file_type != self.node.file_type || file_type == NodeFileType::Symlink
                        } else {
                            true
                        }
                    }
                };
                if need {
                    if self.node.module_path.is_none() {
                        crate::scoped_log!(
                            error,
                            "magic",
                            "tmpfs create skipped: path={}, child={}, reason=root_without_module_path",
                            self.path.display(),
                            name
                        );
                        context.stats.record_ignored();
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
            context.stats.record_tmpfs();
        }

        if self.path.exists() && !self.node.replace {
            self.mount_path(has_tmpfs, context)?;
        }

        if self.node.replace {
            if self.node.module_path.is_none() {
                bail!(
                    "dir {} is declared as replaced but it is root!",
                    self.path.display()
                );
            }
            crate::scoped_log!(debug, "magic", "replace dir: path={}", self.path.display());
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
                .do_mount(context)
            }
            .with_context(|| format!("magic mount {}/{name}", self.path.display()))
            {
                if has_tmpfs {
                    return Err(wrap_with_module_context(e, node));
                }
                crate::scoped_log!(
                    error,
                    "magic",
                    "mount child failed: path={}/{}, error={:#?}",
                    self.path.display(),
                    name,
                    e
                );
                context.record_failed_node(node);
            }
        }

        if tmpfs {
            crate::scoped_log!(
                debug,
                "magic",
                "move tmpfs: src={}, dst={}",
                self.work_dir_path.display(),
                self.path.display()
            );

            try_remount_readonly(&self.work_dir_path, &self.path);
            mount_move(&self.work_dir_path, &self.path).with_context(|| {
                format!(
                    "moving tmpfs {} -> {}",
                    self.work_dir_path.display(),
                    self.path.display()
                )
            })?;
            if let Err(e) = mount_change(&self.path, MountPropagationFlags::PRIVATE) {
                crate::scoped_log!(
                    warn,
                    "magic",
                    "make private failed: path={}, error={:#?}",
                    self.path.display(),
                    e
                );
            }

            #[cfg(any(target_os = "linux", target_os = "android"))]
            if self.umount
                && let Err(e) = send_umountable(&self.path)
            {
                crate::scoped_log!(
                    warn,
                    "magic",
                    "failed to register umountable at {}: {:#}",
                    self.path.display(),
                    e
                );
            }
            context.stats.record_dir();
        }
        Ok(())
    }
}

impl MagicMount {
    fn mount_path(&mut self, has_tmpfs: bool, context: &mut MountContext) -> Result<()> {
        for entry in self.path.read_dir()?.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let mut failed_module_ids: Option<Vec<String>> = None;
            let result = {
                if let Some(node) = self.node.children.remove(&name) {
                    if node.skip {
                        continue;
                    }
                    // pre-compute module ids before the node is consumed
                    failed_module_ids = Some(infer_module_ids(&node));

                    Self::new(
                        &node,
                        &self.path,
                        &self.work_dir_path,
                        has_tmpfs,
                        #[cfg(any(target_os = "linux", target_os = "android"))]
                        self.umount,
                    )
                    .do_mount(context)
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
                    if let Some(ids) = failed_module_ids
                        && !ids.is_empty()
                    {
                        return Err(ModuleStageFailure::execute(ids, e).into());
                    }
                    return Err(e);
                }
                crate::scoped_log!(
                    error,
                    "magic",
                    "mount child failed: path={}/{}, error={:#?}",
                    self.path.display(),
                    name,
                    e
                );
                if let Some(ids) = failed_module_ids {
                    context.stats.record_failed();
                    context.failed_module_ids.extend(ids);
                } else {
                    context.stats.record_failed();
                }
            }
        }

        Ok(())
    }
}

pub fn magic_mount<P>(
    tmp_path: P,
    module_dir: &Path,
    options: MagicMountOptions<'_>,
    magic_modules: &[Module],
    #[cfg(any(target_os = "linux", target_os = "android"))] umount: bool,
    #[cfg(not(any(target_os = "linux", target_os = "android")))] _umount: bool,
) -> Result<(Vec<String>, MountStatistics)>
where
    P: AsRef<Path>,
{
    let mut context = MountContext::default();

    if let Some(root) = collect_module_files(
        module_dir,
        options.managed_partitions,
        magic_modules,
        options.use_kasumi,
    )? {
        crate::scoped_log!(debug, "magic", "collected tree: {:?}", root);
        let tmp_root = tmp_path.as_ref();
        let tmp_dir = tmp_root.join("workdir");
        ensure_dir_exists(&tmp_dir)?;

        mount(
            options.mount_source,
            &tmp_dir,
            "tmpfs",
            MountFlags::empty(),
            None,
        )
        .context("mount tmp")?;
        mount_change(&tmp_dir, MountPropagationFlags::PRIVATE).context("make tmp private")?;

        let ret = MagicMount::new(
            &root,
            Path::new("/"),
            tmp_dir.as_path(),
            false,
            #[cfg(any(target_os = "linux", target_os = "android"))]
            umount,
        )
        .do_mount(&mut context)
        .map_err(|e| wrap_with_module_context(e, &root));

        let mut mounted_module_ids = infer_module_ids(&root);
        mounted_module_ids.retain(|id| !context.failed_module_ids.contains(id));

        if let Err(e) = unmount(&tmp_dir, UnmountFlags::DETACH) {
            crate::scoped_log!(
                error,
                "magic",
                "unmount temp failed: path={}, error={}",
                tmp_dir.display(),
                e
            );
        }
        fs::remove_dir(tmp_dir).ok();

        for (module_id, count) in &context.symlinks_by_module {
            crate::scoped_log!(
                debug,
                "magic",
                "symlink summary: module={}, mounted_symlinks={}",
                module_id,
                count
            );
        }

        crate::scoped_log!(
            info,
            "magic",
            "complete: mounted_modules={}, failed_modules={}, mounted_files={}, mounted_symlinks={}, ignored_files={}",
            mounted_module_ids.len(),
            context.failed_module_ids.len(),
            context.stats.files_mounted,
            context.stats.symlinks_created,
            context.stats.ignored_entries
        );

        ret.map(|()| (mounted_module_ids, context.stats))
    } else {
        crate::scoped_log!(info, "magic", "skip: reason=no_modules_to_mount");
        Ok((Vec::new(), context.stats))
    }
}
