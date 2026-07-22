// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::{
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
    mount::{MoveMountFlags, move_mount},
};

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

    fs(upperdir_s, workdir_s, lowerdir_config, mount_source, dest)?;
    crate::scoped_log!(info, "overlayfs", "mount success: {}", dest.display());
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

    while current_layers.len() > MAX_LAYERS {
        let split_idx = current_layers.len().saturating_sub(MAX_LAYERS - 1);
        let bottom_chunk: Vec<String> = current_layers.drain(split_idx..).collect();

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("system clock is before the Unix epoch")?
            .as_nanos();
        let staging_dir = Path::new(defs::RUN_DIR).join(format!(
            "staging_{}_{}",
            timestamp,
            current_layers.len()
        ));

        ensure_dir_exists(&staging_dir)?;

        mount_overlay_core(&bottom_chunk, None, None, &staging_dir, mount_source)?;
        crate::scoped_log!(
            debug,
            "overlayfs",
            "staging layer created: path={}, input_layers={}",
            staging_dir.display(),
            bottom_chunk.len()
        );

        send_umountable(&staging_dir)?;

        current_layers.push(staging_dir.to_string_lossy().into_owned());
    }

    mount_overlay_core(
        &current_layers,
        upperdir.as_deref(),
        workdir.as_deref(),
        dest.as_ref(),
        mount_source,
    )
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn bind_mount(from: impl AsRef<Path>, to: impl AsRef<Path>) -> Result<()> {
    crate::scoped_log!(
        info,
        "overlayfs",
        "bind mount: src={}, dst={}",
        from.as_ref().display(),
        to.as_ref().display()
    );
    use rustix::mount::{OpenTreeFlags, open_tree};
    let tree = open_tree(
        CWD,
        from.as_ref(),
        OpenTreeFlags::OPEN_TREE_CLOEXEC
            | OpenTreeFlags::OPEN_TREE_CLONE
            | OpenTreeFlags::AT_RECURSIVE,
    )?;
    move_mount(
        tree.as_fd(),
        "",
        CWD,
        to.as_ref(),
        MoveMountFlags::MOVE_MOUNT_F_EMPTY_PATH,
    )?;
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn bind_mount(_from: impl AsRef<Path>, _to: impl AsRef<Path>) -> Result<()> {
    bail!("bind mounts are only supported on linux/android")
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
        bail!("overlay child stock path is not a directory: {stock_root}");
    }
    let mut lower_dirs: Vec<String> = vec![];
    for lower in module_roots {
        let lower_dir = format!("{lower}{relative}");
        let path = Path::new(&lower_dir);
        if path.is_dir() {
            lower_dirs.push(lower_dir);
        } else if path.exists() {
            bail!("overlay child module path is not a directory: {lower_dir}");
        }
    }
    if lower_dirs.is_empty() {
        bail!("overlay child has no directory layers: {mount_point}");
    }
    mount_overlayfs(
        &lower_dirs,
        stock_root,
        None,
        None,
        mount_point,
        mount_source,
    )?;
    send_umountable(mount_point)?;
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
    let old_cwd = std::env::current_dir().context("failed to read current directory")?;
    std::env::set_current_dir(root).with_context(|| format!("failed to chdir to {root}"))?;
    let result = (|| -> Result<()> {
        let stock_root = ".";
        let root_path = Path::new(root);
        let mount_seq = collect_child_mount_points(root_path)?;

        mount_overlayfs(module_roots, root, upperdir, workdir, root, mount_source)
            .context("mount overlayfs for root failed")?;

        for mount_point in &mount_seq {
            let relative = mount_point.replacen(root, "", 1);
            let stock_root: String = format!("{stock_root}{relative}");
            if !Path::new(&stock_root).exists() {
                continue;
            }
            if let Err(error) = mount_overlay_child(
                mount_point,
                &relative,
                module_roots,
                &stock_root,
                mount_source,
            ) {
                umount_dir(root).with_context(|| format!("failed to revert {root}"))?;
                return Err(error)
                    .with_context(|| format!("failed to mount overlay child {mount_point}"));
            }
        }
        Ok(())
    })();

    let restore_result = std::env::set_current_dir(&old_cwd)
        .with_context(|| format!("failed to restore cwd to {}", old_cwd.display()));
    match (result, restore_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(error)) => Err(error),
        (Err(operation_error), Err(restore_error)) => Err(operation_error.context(format!(
            "additionally failed to restore cwd: {restore_error:#}"
        ))),
    }
}
