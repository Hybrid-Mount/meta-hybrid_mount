// ReHybrid-Mount
//
// SPDX-License-Identifier: GPL-3.0-only

//! OverlayFS 挂载编排(行为对齐 v4.2.0 `e20f9c19`):
//! - fsopen("overlay") 主路径,传统 `mount(2)` 转义 fallback;
//! - lowerdir 超过 64 层时,尾部层先叠成 staging 再作为新层;
//! - 根挂载后按 `/proc/self/mountinfo` 的子挂载逐个重建 overlay,
//!   失败时立即 `unmount` 回滚根挂载。
//!
//! Stage 3 脚手架:入口在 Stage 5 CLI 接入前暂未被二进制入口使用;
//! 接入完成后移除本豁免,恢复 dead_code 检查。
#![allow(dead_code)]

#[cfg(any(target_os = "linux", target_os = "android"))]
use std::ffi::CString;
#[cfg(any(target_os = "linux", target_os = "android"))]
use std::path::{Path, PathBuf};
#[cfg(any(target_os = "linux", target_os = "android"))]
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(any(target_os = "linux", target_os = "android"))]
use procfs::process::Process;
#[cfg(any(target_os = "linux", target_os = "android"))]
use rustix::fd::AsFd;
#[cfg(any(target_os = "linux", target_os = "android"))]
use rustix::mount::{MountFlags, MoveMountFlags, OpenTreeFlags, mount, move_mount, open_tree};

#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::defs;
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::errors::{Error, Result};
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::overlayfs::utils;
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::utils::ensure_dir_exists;
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::utils::ksu::send_unmountable;

/// overlayfs 单次挂载最多接受的层数(v4.2.0 行为)。
pub const MAX_LAYERS: usize = 64;

/// 转义 lowerdir 单个路径中的 `\`、`,`、`:`(传统 mount fallback 专用)。
pub fn escape_mount_option_value(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        if matches!(ch, '\\' | ',' | ':') {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    escaped
}

/// 一次 staging 拆分计划:`remaining_layers` 是拆分后剩余层数,
/// `layers` 是本次要先叠成 staging 的尾部层。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagingChunk {
    pub remaining_layers: usize,
    pub layers: Vec<String>,
}

/// 把超过 64 层的 lowerdir 序列拆成多个 staging 步骤(v4.2.0 拆分语义)。
pub fn plan_staging_chunks(layers: &[String]) -> Vec<StagingChunk> {
    let mut current = layers.to_vec();
    let mut chunks = Vec::new();

    while current.len() > MAX_LAYERS {
        let split_idx = current.len().saturating_sub(MAX_LAYERS - 1);
        let staging_layers = current.drain(split_idx..).collect();
        chunks.push(StagingChunk {
            remaining_layers: current.len(),
            layers: staging_layers,
        });
    }

    chunks
}

/// 计算根挂载点下子挂载的相对路径;非子孙路径返回 `None`。
pub fn child_relative_path(root: &str, mount_point: &str) -> Option<String> {
    if mount_point == root {
        return None;
    }
    let stripped = mount_point.strip_prefix(root)?;
    stripped.starts_with('/').then(|| stripped.to_owned())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn collect_child_mount_points(root_path: &Path) -> Result<Vec<String>> {
    let mounts = Process::myself()
        .map_err(|err| Error::msg(format!("get mountinfo: {err}")))?
        .mountinfo()
        .map_err(|err| Error::msg(format!("get mountinfo: {err}")))?;

    let mut mount_seq: Vec<String> = mounts
        .into_iter()
        .filter(|entry| {
            let mount_point = &entry.mount_point;
            mount_point.starts_with(root_path) && mount_point != root_path
        })
        .filter_map(|entry| entry.mount_point.to_str().map(str::to_owned))
        .collect();

    mount_seq.sort();
    mount_seq.dedup();
    Ok(mount_seq)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn mount_overlay_core(
    lower_dirs: &[String],
    upperdir: Option<&Path>,
    workdir: Option<&Path>,
    dest: &Path,
    mount_source: &str,
) -> Result<()> {
    let lowerdir_config = lower_dirs.join(":");

    log::debug!(
        "overlay core mount: dest={}, layers={}, source={mount_source}",
        dest.display(),
        lower_dirs.len()
    );

    let upperdir_s = upperdir
        .filter(|upper| upper.exists())
        .map(|upper| upper.display().to_string());
    let workdir_s = workdir
        .filter(|work| work.exists())
        .map(|work| work.display().to_string());

    if let Err(err) = utils::fsopen_mount(
        upperdir_s.clone(),
        workdir_s.clone(),
        lowerdir_config.clone(),
        mount_source,
        dest,
    ) {
        log::warn!("fsopen failed, fallback to legacy mount: {err}");

        let safe_lower = lower_dirs
            .iter()
            .map(|path| escape_mount_option_value(path))
            .collect::<Vec<_>>()
            .join(":");
        let mut data = format!("lowerdir={safe_lower}");

        if let (Some(upperdir), Some(workdir)) = (upperdir_s, workdir_s) {
            data = format!(
                "{data},upperdir={},workdir={}",
                escape_mount_option_value(&upperdir),
                escape_mount_option_value(&workdir)
            );
        }

        mount(
            mount_source,
            dest,
            "overlay",
            MountFlags::empty(),
            Some(
                CString::new(data)
                    .map_err(|_| Error::msg("overlay mount data contains NUL"))?
                    .as_c_str(),
            ),
        )
        .map_err(|err| Error::msg(format!("legacy overlay mount {}: {err}", dest.display())))?;
    }

    log::info!("overlay mount success: {}", dest.display());
    Ok(())
}

/// 把 lowerdirs + lowest 叠到 dest;超过 64 层时先做 staging。
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn mount_overlayfs(
    lower_dirs: &[String],
    lowest: &str,
    upperdir: Option<PathBuf>,
    workdir: Option<PathBuf>,
    dest: &Path,
    mount_source: &str,
) -> Result<()> {
    let mut current_layers = lower_dirs.to_vec();
    current_layers.push(lowest.to_owned());

    for chunk in plan_staging_chunks(&current_layers) {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let staging_dir = Path::new(defs::STATE_DIR)
            .join(format!("staging_{timestamp}_{}", chunk.remaining_layers));

        ensure_dir_exists(&staging_dir)?;
        mount_overlay_core(&chunk.layers, None, None, &staging_dir, mount_source)?;
        log::debug!(
            "staging layer created: path={}, input_layers={}",
            staging_dir.display(),
            chunk.layers.len()
        );

        send_unmountable(&staging_dir);
        current_layers = current_layers[..chunk.remaining_layers].to_vec();
        current_layers.push(staging_dir.to_string_lossy().into_owned());
    }

    mount_overlay_core(
        &current_layers,
        upperdir.as_deref(),
        workdir.as_deref(),
        dest,
        mount_source,
    )
}

/// 递归 bind mount:优先 open_tree + move_mount,失败回退传统 bind(v4.2.0 行为)。
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn bind_mount(from: &Path, to: &Path) -> Result<()> {
    log::info!("bind mount: src={}, dst={}", from.display(), to.display());

    let tree = open_tree(
        rustix::fs::CWD,
        from,
        OpenTreeFlags::OPEN_TREE_CLOEXEC
            | OpenTreeFlags::OPEN_TREE_CLONE
            | OpenTreeFlags::AT_RECURSIVE,
    );

    match tree {
        Ok(tree) => {
            if move_mount(
                tree.as_fd(),
                "",
                rustix::fs::CWD,
                to,
                MoveMountFlags::MOVE_MOUNT_F_EMPTY_PATH,
            )
            .is_err()
            {
                mount(from, to, "", MountFlags::BIND | MountFlags::REC, None).map_err(|err| {
                    Error::msg(format!(
                        "bind mount {} -> {}: {err}",
                        from.display(),
                        to.display()
                    ))
                })?;
            }
        }
        Err(_) => {
            mount(from, to, "", MountFlags::BIND | MountFlags::REC, None).map_err(|err| {
                Error::msg(format!(
                    "bind mount {} -> {}: {err}",
                    from.display(),
                    to.display()
                ))
            })?;
        }
    }

    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn mount_overlay_child(
    mount_point: &str,
    relative: &str,
    module_roots: &[String],
    stock_root: &str,
    mount_source: &str,
) -> Result<()> {
    if !module_roots
        .iter()
        .any(|lower| Path::new(&format!("{lower}{relative}")).exists())
    {
        return bind_mount(Path::new(stock_root), Path::new(mount_point));
    }

    if !Path::new(stock_root).is_dir() {
        return Ok(());
    }

    let mut lower_dirs = Vec::new();
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

    mount_overlayfs(
        &lower_dirs,
        stock_root,
        None,
        None,
        Path::new(mount_point),
        mount_source,
    )
    .map_err(|err| {
        Error::msg(format!(
            "child overlay failed: mount_point={mount_point}: {err}"
        ))
    })?;
    send_unmountable(Path::new(mount_point));
    Ok(())
}

/// 挂载根 overlay 并重建其子挂载点;任一子挂载失败时立即卸载根回滚。
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn mount_overlay(
    root: &str,
    module_roots: &[String],
    workdir: Option<PathBuf>,
    upperdir: Option<PathBuf>,
    mount_source: &str,
) -> Result<()> {
    log::info!("overlay mount root: target={root}");

    let old_cwd = std::env::current_dir().ok();
    std::env::set_current_dir(root).map_err(|err| Error::msg(format!("chdir to {root}: {err}")))?;
    let stock_root = ".";

    let root_path = Path::new(root);
    let mount_seq = collect_child_mount_points(root_path)?;

    let root_result = mount_overlayfs(
        module_roots,
        root,
        upperdir,
        workdir,
        root_path,
        mount_source,
    );

    if let Err(err) = root_result {
        if let Some(cwd) = &old_cwd {
            std::env::set_current_dir(cwd).ok();
        }
        return Err(Error::msg(format!(
            "mount overlayfs for root failed: {err}"
        )));
    }

    for mount_point in &mount_seq {
        let Some(relative) = child_relative_path(root, mount_point) else {
            continue;
        };
        let stock_root = format!("{stock_root}{relative}");
        if !Path::new(&stock_root).exists() {
            continue;
        }

        if let Err(err) = mount_overlay_child(
            mount_point,
            &relative,
            module_roots,
            &stock_root,
            mount_source,
        ) {
            log::warn!(
                "child mount failed, reverting root: mount_point={mount_point}, error={err}"
            );
            utils::umount_dir(root_path)?;
            if let Some(cwd) = old_cwd {
                std::env::set_current_dir(&cwd).ok();
            }
            return Err(err);
        }
    }

    if let Some(cwd) = old_cwd {
        std::env::set_current_dir(&cwd).ok();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layer(n: usize) -> String {
        format!("/layer{n}")
    }

    #[test]
    fn escape_mount_option_value_escapes_overlay_separators() {
        assert_eq!(escape_mount_option_value("/a,b:/c\\d"), "/a\\,b\\:/c\\\\d");
    }

    #[test]
    fn escaped_lowerdir_preserves_layer_separators() {
        let lower_dirs = ["/a,b".to_owned(), "/c:d".to_owned(), "/e\\f".to_owned()];
        let lowerdir = lower_dirs
            .iter()
            .map(|path| escape_mount_option_value(path))
            .collect::<Vec<_>>()
            .join(":");

        assert_eq!(lowerdir, "/a\\,b:/c\\:d:/e\\\\f");
    }

    #[test]
    fn layers_within_limit_need_no_staging() {
        let layers: Vec<String> = (0..MAX_LAYERS).map(layer).collect();
        assert!(plan_staging_chunks(&layers).is_empty());
    }

    #[test]
    fn one_extra_layer_splits_one_chunk() {
        let layers: Vec<String> = (0..MAX_LAYERS + 1).map(layer).collect();
        let chunks = plan_staging_chunks(&layers);

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].remaining_layers, 2);
        assert_eq!(chunks[0].layers.len(), MAX_LAYERS - 1);
        assert_eq!(chunks[0].layers[0], layer(2));
    }

    #[test]
    fn many_layers_split_repeatedly() {
        let layers: Vec<String> = (0..MAX_LAYERS * 2).map(layer).collect();
        let chunks = plan_staging_chunks(&layers);

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].remaining_layers, MAX_LAYERS + 1);
        assert_eq!(chunks[0].layers.len(), MAX_LAYERS - 1);
        assert_eq!(chunks[1].remaining_layers, 2);
        assert_eq!(chunks[1].layers.len(), MAX_LAYERS - 1);
    }

    #[test]
    fn child_relative_path_computes_suffix() {
        assert_eq!(
            child_relative_path("/system", "/system/priv-app"),
            Some("/priv-app".to_owned())
        );
        assert_eq!(child_relative_path("/system", "/system"), None);
        assert_eq!(child_relative_path("/system", "/product"), None);
        assert_eq!(child_relative_path("/system", "/system_ext/app"), None);
    }
}
