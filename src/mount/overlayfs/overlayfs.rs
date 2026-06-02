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

#[cfg(not(any(target_os = "linux", target_os = "android")))]
use std::ops::BitOr;
use std::{
    ffi::CString,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};
#[cfg(any(target_os = "linux", target_os = "android"))]
use procfs::process::Process;
#[cfg(any(target_os = "linux", target_os = "android"))]
use rustix::{
    fd::AsFd,
    fs::CWD,
    mount::{MountFlags, MoveMountFlags, UnmountFlags, mount, move_mount, unmount as umount2},
};

#[cfg(not(any(target_os = "linux", target_os = "android")))]
const CWD: i32 = libc::AT_FDCWD;

#[cfg(not(any(target_os = "linux", target_os = "android")))]
#[derive(Clone, Copy)]
struct MountFlags;

#[cfg(not(any(target_os = "linux", target_os = "android")))]
impl MountFlags {
    const BIND: Self = Self;
    const REC: Self = Self;

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
fn mount<P, Q>(
    _source: P,
    _target: Q,
    _fstype: &str,
    _flags: MountFlags,
    _data: Option<&std::ffi::CStr>,
) -> Result<()>
where
    P: AsRef<Path>,
    Q: AsRef<Path>,
{
    bail!("mount is only supported on linux/android")
}

use crate::{
    defs,
    mount::{
        overlayfs::utils::{fs, umount_dir},
        umount_mgr::send_umountable,
    },
    sys::fs::ensure_dir_exists,
};

const MAX_LAYERS: usize = 64;

#[cfg(any(target_os = "linux", target_os = "android"))]
fn collect_child_mount_points(root_path: &Path) -> Result<Vec<String>> {
    let mounts = Process::myself()?
        .mountinfo()
        .with_context(|| "get mountinfo")?;

    let mut mount_seq: Vec<String> = mounts
        .0
        .iter()
        .filter(|m| {
            let mp = Path::new(&m.mount_point);
            mp.starts_with(root_path) && mp != root_path
        })
        .filter_map(|m| m.mount_point.to_str().map(|p| p.to_string()))
        .collect();
    mount_seq.sort();
    mount_seq.dedup();
    Ok(mount_seq)
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn collect_child_mount_points(_root_path: &Path) -> Result<Vec<String>> {
    Ok(Vec::new())
}

fn mount_overlay_core(
    lower_dirs: &[String],
    upperdir: Option<&Path>,
    workdir: Option<&Path>,
    dest: &Path,
    mount_source: &str,
) -> Result<()> {
    let lowerdir_config = lower_dirs.join(":");

    crate::scoped_log!(
        debug,
        "overlayfs",
        "core mount: dest={}, layers={}, source={}",
        dest.display(),
        lower_dirs.len(),
        mount_source
    );

    let upperdir_s = upperdir
        .filter(|up| up.exists())
        .map(|e| e.display().to_string());
    let workdir_s = workdir
        .filter(|wd| wd.exists())
        .map(|e| e.display().to_string());

    if let Err(e) = fs(
        upperdir_s.clone(),
        workdir_s.clone(),
        lowerdir_config.clone(),
        mount_source,
        dest,
    ) {
        crate::scoped_log!(warn, "overlayfs", "fsopen failed, fallback=mount: {:#}", e);
        let safe_lower = lowerdir_config.replace(',', "\\,");
        let mut data = format!("lowerdir={safe_lower}");

        if let (Some(upperdir), Some(workdir)) = (upperdir_s, workdir_s) {
            data = format!(
                "{data},upperdir={},workdir={}",
                upperdir.replace(',', "\\,"),
                workdir.replace(',', "\\,")
            );
        }
        mount(
            mount_source,
            dest,
            "overlay",
            MountFlags::empty(),
            Some(CString::new(data)?.as_c_str()),
        )?;
    }
    Ok(())
}

pub fn mount_overlayfs(
    lower_dirs: &[String],
    lowest: &str,
    upperdir: Option<PathBuf>,
    workdir: Option<PathBuf>,
    dest: impl AsRef<Path>,
    mount_source: &str,
) -> Result<()> {
    let mut current_layers: Vec<String> = lower_dirs.to_vec();
    current_layers.push(lowest.to_string());
    let mut staging_dirs: Vec<PathBuf> = Vec::new();

    while current_layers.len() > MAX_LAYERS {
        let split_idx = current_layers.len().saturating_sub(MAX_LAYERS - 1);
        let bottom_chunk: Vec<String> = current_layers.drain(split_idx..).collect();

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let staging_dir = Path::new(defs::RUN_DIR).join(format!(
            "staging_{}_{}",
            timestamp,
            current_layers.len()
        ));

        ensure_dir_exists(&staging_dir)?;

        mount_overlay_core(&bottom_chunk, None, None, &staging_dir, mount_source)?;

        // Staging dirs are temporary and self-cleaned below — do NOT
        // register them with KSU's global umount list.
        staging_dirs.push(staging_dir.clone());
        current_layers.push(staging_dir.to_string_lossy().into_owned());
    }

    let result = mount_overlay_core(
        &current_layers,
        upperdir.as_deref(),
        workdir.as_deref(),
        dest.as_ref(),
        mount_source,
    );

    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        // Clean up staging overlay mounts. Use MNT_DETACH so the final overlay
        // can keep its references to the merged lower layers alive until unmounted.
        for staging_dir in staging_dirs.iter().rev() {
            if let Err(e) = umount2(staging_dir.as_path(), UnmountFlags::DETACH) {
                crate::scoped_log!(
                    warn,
                    "overlayfs",
                    "failed to detach staging overlay {}: {:#}",
                    staging_dir.display(),
                    e
                );
            }
            if let Err(e) = std::fs::remove_dir(staging_dir) {
                crate::scoped_log!(
                    debug,
                    "overlayfs",
                    "failed to remove staging dir {}: {:#}",
                    staging_dir.display(),
                    e
                );
            }
        }
    }

    result
}

pub fn bind_mount(from: impl AsRef<Path>, to: impl AsRef<Path>) -> Result<()> {
    crate::scoped_log!(
        info,
        "overlayfs",
        "bind mount: src={}, dst={}",
        from.as_ref().display(),
        to.as_ref().display()
    );
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        use rustix::mount::{OpenTreeFlags, open_tree};
        match open_tree(
            CWD,
            from.as_ref(),
            OpenTreeFlags::OPEN_TREE_CLOEXEC
                | OpenTreeFlags::OPEN_TREE_CLONE
                | OpenTreeFlags::AT_RECURSIVE,
        ) {
            Result::Ok(tree) => {
                if move_mount(
                    tree.as_fd(),
                    "",
                    CWD,
                    to.as_ref(),
                    MoveMountFlags::MOVE_MOUNT_F_EMPTY_PATH,
                )
                .is_err()
                {
                    mount(
                        from.as_ref(),
                        to.as_ref(),
                        "",
                        MountFlags::BIND | MountFlags::REC,
                        None,
                    )?;
                }
            }
            _ => {
                mount(
                    from.as_ref(),
                    to.as_ref(),
                    "",
                    MountFlags::BIND | MountFlags::REC,
                    None,
                )?;
            }
        }
    }

    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        let _ = CWD;
        mount(
            from.as_ref(),
            to.as_ref(),
            "",
            MountFlags::BIND | MountFlags::REC,
            None,
        )?;
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        // handled above
    }

    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        Ok(())
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        Ok(())
    }
}

fn mount_overlay_child(
    mount_point: &str,
    relative: &String,
    module_roots: &Vec<String>,
    stock_root: &String,
    mount_source: &str,
) -> Result<()> {
    if !module_roots
        .iter()
        .any(|lower| Path::new(&format!("{lower}{relative}")).exists())
    {
        return bind_mount(stock_root, mount_point);
    }
    if !Path::new(&stock_root).is_dir() {
        return Ok(());
    }
    let mut lower_dirs: Vec<String> = vec![];
    for lower in module_roots {
        let lower_dir = format!("{lower}{relative}");
        let path = Path::new(&lower_dir);
        if path.is_dir() {
            lower_dirs.push(lower_dir);
        } else if path.exists() {
            return Ok(());
        }
    }
    if lower_dirs.is_empty() {
        return Ok(());
    }
    if let Err(e) = mount_overlayfs(
        &lower_dirs,
        stock_root,
        None,
        None,
        mount_point,
        mount_source,
    ) {
        crate::scoped_log!(
            warn,
            "overlayfs",
            "child overlay failed: mount_point={}, error={:#}",
            mount_point,
            e
        );
        return Err(e);
    }
    if let Err(e) = send_umountable(mount_point) {
        crate::scoped_log!(
            warn,
            "overlayfs",
            "failed to register umountable at {}: {:#}",
            mount_point,
            e
        );
    }
    Ok(())
}

pub fn mount_overlay(
    root: &String,
    module_roots: &Vec<String>,
    workdir: Option<PathBuf>,
    upperdir: Option<PathBuf>,
    mount_source: &str,
) -> Result<()> {
    crate::scoped_log!(info, "overlayfs", "mount root: target={}", root);
    // Restore original CWD on exit — chdir is a process-global side effect.
    let old_cwd = std::env::current_dir().ok();
    std::env::set_current_dir(root).with_context(|| format!("failed to chdir to {root}"))?;
    let stock_root = ".";

    let root_path = Path::new(root);
    let mount_seq = collect_child_mount_points(root_path)?;

    let root_result = mount_overlayfs(module_roots, root, upperdir, workdir, root, mount_source)
        .with_context(|| "mount overlayfs for root failed");
    if let Err(e) = root_result {
        if let Some(cwd) = old_cwd {
            std::env::set_current_dir(&cwd).ok();
        }
        return Err(e);
    }

    for mount_point in &mount_seq {
        let relative = mount_point.replacen(root, "", 1);
        let stock_root: String = format!("{stock_root}{relative}");
        if !Path::new(&stock_root).exists() {
            continue;
        }
        if let Err(e) = mount_overlay_child(
            mount_point,
            &relative,
            module_roots,
            &stock_root,
            mount_source,
        ) {
            crate::scoped_log!(
                warn,
                "overlayfs",
                "child mount failed, revert root: mount_point={}, error={:#}",
                mount_point,
                e
            );
            umount_dir(root).with_context(|| format!("failed to revert {root}"))?;
            if let Some(cwd) = old_cwd {
                std::env::set_current_dir(&cwd).ok();
            }
            bail!(e);
        }
    }
    if let Some(cwd) = old_cwd {
        std::env::set_current_dir(&cwd).ok();
    }
    Ok(())
}
