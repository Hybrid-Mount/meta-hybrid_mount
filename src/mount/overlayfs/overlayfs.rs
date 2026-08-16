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
    mount::{overlayfs::utils::fs, umount_mgr::send_umountable},
    sys::{fs::ensure_dir_exists, mount::MountRollback},
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

fn record_mounted_target<F>(
    rollback: &mut MountRollback,
    target: impl Into<PathBuf>,
    mount: F,
) -> Result<()>
where
    F: FnOnce() -> Result<()>,
{
    let target = target.into();
    mount()?;
    // Record immediately after the kernel mount succeeds. Any later failure,
    // including umount registration or cwd restoration, can now unwind it.
    rollback.record(target);
    Ok(())
}

fn take_staging_chunk(current_layers: &mut Vec<String>) -> Option<Vec<String>> {
    if current_layers.len() <= MAX_LAYERS {
        return None;
    }

    // Reserve one slot for the staging mount that replaces these bottom
    // layers. Repeating this reduction keeps every overlay at <= MAX_LAYERS.
    let split_idx = current_layers.len() - (MAX_LAYERS - 1);
    Some(current_layers.drain(split_idx..).collect())
}

struct OverlayMountSpec<'a> {
    lower_dirs: &'a [String],
    lowest: &'a str,
    upperdir: Option<&'a Path>,
    workdir: Option<&'a Path>,
    dest: &'a Path,
    mount_source: &'a str,
}

fn mount_overlayfs_into(
    spec: OverlayMountSpec<'_>,
    register_umountable: bool,
    rollback: &mut MountRollback,
) -> Result<()> {
    let mut current_layers: Vec<String> = spec.lower_dirs.to_vec();
    current_layers.push(spec.lowest.to_string());

    while let Some(bottom_chunk) = take_staging_chunk(&mut current_layers) {
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

        record_mounted_target(rollback, staging_dir.clone(), || {
            mount_overlay_core(&bottom_chunk, None, None, &staging_dir, spec.mount_source)
        })?;
        crate::scoped_log!(
            debug,
            "overlayfs",
            "staging layer created: path={}, input_layers={}",
            staging_dir.display(),
            bottom_chunk.len()
        );

        if register_umountable {
            send_umountable(&staging_dir).with_context(|| {
                format!(
                    "failed to register overlay staging mount as umountable: {}",
                    staging_dir.display()
                )
            })?;
        }

        current_layers.push(staging_dir.to_string_lossy().into_owned());
    }

    record_mounted_target(rollback, spec.dest.to_path_buf(), || {
        mount_overlay_core(
            &current_layers,
            spec.upperdir,
            spec.workdir,
            spec.dest,
            spec.mount_source,
        )
    })
}

fn combine_operation_and_restore(operation: Result<()>, restore: Result<()>) -> Result<()> {
    match (operation, restore) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(error)) => Err(error),
        (Err(operation_error), Err(restore_error)) => Err(operation_error.context(format!(
            "additionally failed to restore cwd: {restore_error:#}"
        ))),
    }
}

fn finalize_overlay_transaction_with<F>(
    mut rollback: MountRollback,
    operation: Result<()>,
    restore: Result<()>,
    detach: F,
) -> Result<Vec<PathBuf>>
where
    F: FnMut(&Path) -> Result<()>,
{
    match combine_operation_and_restore(operation, restore) {
        Ok(()) => Ok(rollback.into_targets()),
        Err(error) => match rollback.rollback_with(detach) {
            Ok(()) => Err(error),
            Err(rollback_error) => Err(error.context(format!(
                "additionally failed to roll back mounted targets: {rollback_error:#}"
            ))),
        },
    }
}

fn finalize_overlay_transaction(
    rollback: MountRollback,
    operation: Result<()>,
    restore: Result<()>,
) -> Result<Vec<PathBuf>> {
    finalize_overlay_transaction_with(
        rollback,
        operation,
        restore,
        crate::sys::mount::detach_mount,
    )
}

fn register_root_before_mounting_children<R, C>(
    register_umountable: bool,
    root: &str,
    register_root: R,
    mount_children: C,
) -> Result<()>
where
    R: FnOnce(&str) -> Result<()>,
    C: FnOnce() -> Result<()>,
{
    if register_umountable {
        register_root(root)?;
    }
    mount_children()
}

pub fn mount_overlayfs(
    lower_dirs: &[String],
    lowest: &str,
    upperdir: Option<PathBuf>,
    workdir: Option<PathBuf>,
    dest: impl AsRef<Path>,
    mount_source: &str,
) -> Result<Vec<PathBuf>> {
    let mut rollback = MountRollback::default();
    let result = mount_overlayfs_into(
        OverlayMountSpec {
            lower_dirs,
            lowest,
            upperdir: upperdir.as_deref(),
            workdir: workdir.as_deref(),
            dest: dest.as_ref(),
            mount_source,
        },
        true,
        &mut rollback,
    );
    finalize_overlay_transaction(rollback, result, Ok(()))
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
    relative: &str,
    module_roots: &[String],
    stock_root: &str,
    mount_source: &str,
    register_umountable: bool,
    rollback: &mut MountRollback,
) -> Result<()> {
    if !module_roots
        .iter()
        .any(|lower| Path::new(&format!("{lower}{relative}")).exists())
    {
        record_mounted_target(rollback, PathBuf::from(mount_point), || {
            bind_mount(stock_root, mount_point)
        })?;
        if register_umountable {
            send_umountable(mount_point).with_context(|| {
                format!("failed to register overlay child bind as umountable: {mount_point}")
            })?;
        }
        return Ok(());
    }
    if !Path::new(stock_root).is_dir() {
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
    mount_overlayfs_into(
        OverlayMountSpec {
            lower_dirs: &lower_dirs,
            lowest: stock_root,
            upperdir: None,
            workdir: None,
            dest: Path::new(mount_point),
            mount_source,
        },
        register_umountable,
        rollback,
    )?;
    if register_umountable {
        send_umountable(mount_point).with_context(|| {
            format!("failed to register overlay child as umountable: {mount_point}")
        })?;
    }
    Ok(())
}

pub fn mount_overlay(
    root: &str,
    module_roots: &[String],
    workdir: Option<PathBuf>,
    upperdir: Option<PathBuf>,
    mount_source: &str,
    register_umountable: bool,
) -> Result<Vec<PathBuf>> {
    crate::scoped_log!(info, "overlayfs", "mount root: target={}", root);
    let old_cwd = std::env::current_dir().context("failed to read current directory")?;
    std::env::set_current_dir(root).with_context(|| format!("failed to chdir to {root}"))?;
    let mut rollback = MountRollback::default();
    let result = (|| -> Result<()> {
        let stock_root = ".";
        let root_path = Path::new(root);
        let mount_seq = collect_child_mount_points(root_path)?;

        mount_overlayfs_into(
            OverlayMountSpec {
                lower_dirs: module_roots,
                lowest: root,
                upperdir: upperdir.as_deref(),
                workdir: workdir.as_deref(),
                dest: root_path,
                mount_source,
            },
            register_umountable,
            &mut rollback,
        )
        .context("mount overlayfs for root failed")?;

        // KernelSU inserts each registration at the list head, then walks the
        // list forwards at app-fork time. Registering the parent first makes
        // the effective detach order children-before-parent and prevents a
        // child path from resolving to an underlying real mount.
        register_root_before_mounting_children(
            register_umountable,
            root,
            |target| {
                send_umountable(target).with_context(|| {
                    format!("failed to register overlay root as umountable: {target}")
                })
            },
            || {
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
                        register_umountable,
                        &mut rollback,
                    ) {
                        return Err(error).with_context(|| {
                            format!("failed to mount overlay child {mount_point}")
                        });
                    }
                }
                Ok(())
            },
        )
    })();

    let restore_result = std::env::set_current_dir(&old_cwd)
        .with_context(|| format!("failed to restore cwd to {}", old_cwd.display()));
    finalize_overlay_transaction(rollback, result, restore_result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staging_reduction_keeps_each_overlay_within_kernel_layer_limit() {
        let mut layers = (0..131).map(|index| format!("layer-{index}")).collect();
        let mut chunks = Vec::new();

        while let Some(chunk) = take_staging_chunk(&mut layers) {
            assert!(chunk.len() <= MAX_LAYERS);
            chunks.push(chunk);
            layers.push(format!("staging-{}", chunks.len()));
        }

        assert_eq!(chunks.len(), 2);
        assert!(chunks.iter().all(|chunk| chunk.len() == MAX_LAYERS - 1));
        assert_eq!(layers.len(), 7);
        assert!(layers.len() <= MAX_LAYERS);
    }

    #[test]
    fn root_registration_precedes_all_child_work() {
        use std::cell::RefCell;

        let events = RefCell::new(Vec::new());
        register_root_before_mounting_children(
            true,
            "/system",
            |target| {
                events.borrow_mut().push(format!("register:{target}"));
                Ok(())
            },
            || {
                events
                    .borrow_mut()
                    .push("register:/system/vendor".to_string());
                events
                    .borrow_mut()
                    .push("register:/system/vendor/etc".to_string());
                Ok(())
            },
        )
        .unwrap();

        assert_eq!(
            events.into_inner(),
            vec![
                "register:/system",
                "register:/system/vendor",
                "register:/system/vendor/etc",
            ]
        );
    }

    #[test]
    fn failed_mount_is_not_recorded_but_success_is_recorded_immediately() {
        let mut rollback = MountRollback::default();
        let error =
            record_mounted_target(&mut rollback, "/failed", || anyhow::bail!("mount failed"))
                .unwrap_err();
        assert!(error.to_string().contains("mount failed"));

        record_mounted_target(&mut rollback, "/mounted", || Ok(())).unwrap();
        assert_eq!(rollback.into_targets(), vec![PathBuf::from("/mounted")]);
    }

    #[test]
    fn send_failure_rolls_back_staging_root_and_children_in_reverse_order() {
        let mut rollback = MountRollback::default();
        for target in [
            "/run/staging-a",
            "/system",
            "/system/vendor",
            "/system/vendor/etc/hosts",
        ] {
            rollback.record(target);
        }
        let mut detached = Vec::new();

        let error = finalize_overlay_transaction_with(
            rollback,
            Err(anyhow::anyhow!("send_umountable failed")),
            Ok(()),
            |target| {
                detached.push(target.to_path_buf());
                Ok(())
            },
        )
        .unwrap_err();

        assert!(error.to_string().contains("send_umountable failed"));
        assert_eq!(
            detached,
            vec![
                PathBuf::from("/system/vendor/etc/hosts"),
                PathBuf::from("/system/vendor"),
                PathBuf::from("/system"),
                PathBuf::from("/run/staging-a"),
            ]
        );
    }

    #[test]
    fn cwd_restore_failure_rolls_back_all_mounts_and_preserves_both_errors() {
        let mut rollback = MountRollback::default();
        rollback.record("/run/staging-a");
        rollback.record("/system");
        let mut detached = Vec::new();

        let error = finalize_overlay_transaction_with(
            rollback,
            Ok(()),
            Err(anyhow::anyhow!("cwd disappeared")),
            |target| {
                detached.push(target.to_path_buf());
                if target == Path::new("/system") {
                    anyhow::bail!("busy")
                }
                Ok(())
            },
        )
        .unwrap_err();
        let rendered = format!("{error:#}");

        assert!(rendered.contains("cwd disappeared"));
        assert!(rendered.contains("failed to roll back"));
        assert!(rendered.contains("/system: busy"));
        assert_eq!(
            detached,
            vec![PathBuf::from("/system"), PathBuf::from("/run/staging-a")]
        );
    }

    #[test]
    fn operation_and_cwd_restore_failures_are_both_reported() {
        let error = finalize_overlay_transaction_with(
            MountRollback::default(),
            Err(anyhow::anyhow!("child mount failed")),
            Err(anyhow::anyhow!("cwd restore failed")),
            |_| Ok(()),
        )
        .unwrap_err();
        let rendered = format!("{error:#}");

        assert!(rendered.contains("child mount failed"));
        assert!(rendered.contains("cwd restore failed"));
    }

    #[test]
    fn successful_transaction_returns_complete_mount_order_to_executor() {
        let mut rollback = MountRollback::default();
        for target in ["/run/staging-a", "/system", "/system/vendor"] {
            rollback.record(target);
        }

        let targets =
            finalize_overlay_transaction_with(rollback, Ok(()), Ok(()), |_| Ok(())).unwrap();

        assert_eq!(
            targets,
            vec![
                PathBuf::from("/run/staging-a"),
                PathBuf::from("/system"),
                PathBuf::from("/system/vendor"),
            ]
        );
    }
}
