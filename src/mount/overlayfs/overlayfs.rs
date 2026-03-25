// Copyright 2026 https://github.com/KernelSU-Modules-Repo/meta-overlayfs

use std::{
    ffi::CString,
    os::fd::AsFd,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};
use procfs::process::Process;
use rustix::{
    fs::CWD,
    mount::{MountFlags, MoveMountFlags, mount, move_mount},
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

fn mount_overlay_core(
    lower_dirs: &[String],
    upperdir: Option<&Path>,
    workdir: Option<&Path>,
    dest: &Path,
    mount_source: &str,
) -> Result<()> {
    let lowerdir_config = lower_dirs.join(":");

    log::debug!(
        "core mount overlayfs on {:?}, layers={}, source={}",
        dest,
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
        log::warn!("fsopen mount failed: {:#}, fallback to mount", e);
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

    while current_layers.len() > MAX_LAYERS {
        let split_idx = current_layers.len().saturating_sub(MAX_LAYERS - 1);
        let bottom_chunk: Vec<String> = current_layers.drain(split_idx..).collect();

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let staging_dir = Path::new(defs::RUN_DIR).join(format!("staging_{}", timestamp));

        ensure_dir_exists(&staging_dir)?;

        mount_overlay_core(&bottom_chunk, None, None, &staging_dir, mount_source)?;

        let _ = send_umountable(&staging_dir);

        current_layers.push(staging_dir.to_string_lossy().to_string());
    }

    mount_overlay_core(
        &current_layers,
        upperdir.as_deref(),
        workdir.as_deref(),
        dest.as_ref(),
        mount_source,
    )
}

pub fn bind_mount(from: impl AsRef<Path>, to: impl AsRef<Path>) -> Result<()> {
    log::info!(
        "bind mount {} -> {}",
        from.as_ref().display(),
        to.as_ref().display()
    );
    use rustix::mount::{OpenTreeFlags, open_tree};
    match open_tree(
        CWD,
        from.as_ref(),
        OpenTreeFlags::OPEN_TREE_CLOEXEC
            | OpenTreeFlags::OPEN_TREE_CLONE
            | OpenTreeFlags::AT_RECURSIVE,
    ) {
        Result::Ok(tree) => {
            move_mount(
                tree.as_fd(),
                "",
                CWD,
                to.as_ref(),
                MoveMountFlags::MOVE_MOUNT_F_EMPTY_PATH,
            )?;
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
    Ok(())
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
        log::warn!(
            "failed to mount overlayfs for child {}: {:#}",
            mount_point,
            e
        );
        return Err(e);
    }
    let _ = send_umountable(mount_point);
    Ok(())
}

pub fn mount_overlay(
    root: &String,
    module_roots: &Vec<String>,
    workdir: Option<PathBuf>,
    upperdir: Option<PathBuf>,
    mount_source: &str,
) -> Result<()> {
    log::info!("mount overlay for {}", root);
    std::env::set_current_dir(root).with_context(|| format!("failed to chdir to {root}"))?;
    let stock_root = ".";

    let mounts = Process::myself()?
        .mountinfo()
        .with_context(|| "get mountinfo")?;

    let root_path = Path::new(root);
    let mut mount_seq = mounts
        .0
        .iter()
        .filter(|m| {
            let mp = Path::new(&m.mount_point);
            mp.starts_with(root_path) && mp != root_path
        })
        .map(|m| m.mount_point.to_str())
        .collect::<Vec<_>>();

    mount_seq.sort();
    mount_seq.dedup();

    mount_overlayfs(module_roots, root, upperdir, workdir, root, mount_source)
        .with_context(|| "mount overlayfs for root failed")?;
    for mount_point in mount_seq.iter() {
        let Some(mount_point) = mount_point else {
            continue;
        };
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
            log::warn!(
                "failed to mount overlay for child {}: {:#}, revert",
                mount_point,
                e
            );
            umount_dir(root).with_context(|| format!("failed to revert {root}"))?;
            bail!(e);
        }
    }
    Ok(())
}
