// SPDX-License-Identifier: GPL-3.0-only

//! OverlayFS 挂载编排(行为对齐 v4.2.0 `e20f9c19`):
//! - fsopen("overlay") 主路径,传统 `mount(2)` 转义 fallback;
//! - lowerdir 超过 64 层时,尾部层先叠成 staging 再作为新层;
//! - 根挂载后按 `/proc/self/mountinfo` 的子挂载逐个重建 overlay,
//!   失败时交由流水线事务回滚已登记目标。

#[cfg(any(target_os = "linux", target_os = "android"))]
use std::ffi::CString;
#[cfg(any(target_os = "linux", target_os = "android"))]
use std::path::{Path, PathBuf};

#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::errors::{Error, Result};
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::overlayfs::utils;
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::utils::ksu::send_unmountable;
#[cfg(any(target_os = "linux", target_os = "android"))]
use procfs::process::Process;
#[cfg(any(target_os = "linux", target_os = "android"))]
use rustix::fd::AsFd;
#[cfg(any(target_os = "linux", target_os = "android"))]
use rustix::mount::{
    MountFlags, MoveMountFlags, OpenTreeFlags, UnmountFlags, mount, move_mount, open_tree, unmount,
};

#[cfg(any(target_os = "linux", target_os = "android"))]
#[derive(Debug)]
pub enum MountEffect {
    Target(String),
    Staging(PathBuf),
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub(crate) fn cleanup_staging_mount(path: PathBuf) -> Result<()> {
    // 正常清理路径不得把 mountinfo 查询失败当成“未挂载”。
    if crate::sys::mount::is_mounted(&path)?
        && let Err(err) = unmount(&path, UnmountFlags::DETACH)
        && !matches!(err, rustix::io::Errno::NOENT | rustix::io::Errno::INVAL)
    {
        return Err(Error::msg(format!(
            "detach intermediate overlay staging failed: path={}, error={err}",
            path.display()
        )));
    }
    if crate::sys::mount::is_mounted(&path)? {
        return Err(Error::msg(format!(
            "intermediate overlay staging still mounted after detach: path={}",
            path.display()
        )));
    }

    if crate::sys::faults::should_fail_staging_remove() {
        return Err(Error::msg(format!(
            "injected intermediate overlay staging remove failure: path={}",
            path.display()
        )));
    }

    match std::fs::remove_dir_all(&path) {
        Ok(()) => {
            log::debug!(
                "intermediate overlay staging removed: path={}",
                path.display()
            );
            Ok(())
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(Error::msg(format!(
            "remove intermediate overlay staging failed: path={}, error={err}",
            path.display()
        ))),
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[derive(Debug)]
struct CurrentDirGuard {
    original: PathBuf,
}

#[cfg(any(target_os = "linux", target_os = "android"))]
impl CurrentDirGuard {
    fn change_to(path: &Path) -> Result<Self> {
        let original = std::env::current_dir()
            .map_err(|err| Error::msg(format!("read current directory: {err}")))?;
        std::env::set_current_dir(path)
            .map_err(|err| Error::msg(format!("chdir to {}: {err}", path.display())))?;
        Ok(Self { original })
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
impl Drop for CurrentDirGuard {
    fn drop(&mut self) {
        if let Err(err) = std::env::set_current_dir(&self.original) {
            log::error!(
                "failed to restore current directory to {}: {err}",
                self.original.display()
            );
        }
    }
}

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
    let upperdir_s = upperdir
        .filter(|upper| upper.exists())
        .map(|upper| upper.display().to_string());
    let workdir_s = workdir
        .filter(|work| work.exists())
        .map(|work| work.display().to_string());

    log::debug!(
        "overlay core mount request: dest={}, layers={}, source={}, lowerdirs={}, upperdir={}, workdir={}",
        dest.display(),
        lower_dirs.len(),
        mount_source,
        lower_dirs.join(" | "),
        upperdir_s.as_deref().unwrap_or("none"),
        workdir_s.as_deref().unwrap_or("none")
    );

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

    log::debug!("overlay mount success: {}", dest.display());
    Ok(())
}

/// 把 lowerdirs + lowest 叠到 dest;超过 64 层时先做 staging。
#[cfg(any(target_os = "linux", target_os = "android"))]
#[allow(clippy::too_many_arguments)]
pub fn mount_overlayfs(
    lower_dirs: &[String],
    lowest: &str,
    upperdir: Option<PathBuf>,
    workdir: Option<PathBuf>,
    dest: &Path,
    staging_root: &Path,
    mount_source: &str,
    on_effect: &mut dyn FnMut(MountEffect),
) -> Result<()> {
    let mut current_layers = lower_dirs.to_vec();
    current_layers.push(lowest.to_owned());

    for chunk in plan_staging_chunks(&current_layers) {
        let staging_dir = crate::sys::temp::create_random_dir(staging_root)?;
        mount_overlay_core(&chunk.layers, None, None, &staging_dir, mount_source)?;
        log::debug!(
            "staging layer created: path={}, input_layers={}",
            staging_dir.display(),
            chunk.layers.len()
        );
        let staging_layer = staging_dir.to_string_lossy().into_owned();
        on_effect(MountEffect::Staging(staging_dir));
        current_layers = current_layers[..chunk.remaining_layers].to_vec();
        current_layers.push(staging_layer);
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
    log::debug!("bind mount: src={}, dst={}", from.display(), to.display());

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
#[allow(clippy::too_many_arguments)]
fn mount_overlay_child(
    mount_point: &str,
    relative: &str,
    module_roots: &[String],
    stock_root: &str,
    staging_root: &Path,
    mount_source: &str,
    register_unmountable: bool,
    on_effect: &mut dyn FnMut(MountEffect),
) -> Result<()> {
    if !module_roots
        .iter()
        .any(|lower| Path::new(&format!("{lower}{relative}")).exists())
    {
        bind_mount(Path::new(stock_root), Path::new(mount_point))?;
        if register_unmountable {
            send_unmountable(Path::new(mount_point));
        }
        on_effect(MountEffect::Target(mount_point.to_owned()));
        return Ok(());
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
        staging_root,
        mount_source,
        on_effect,
    )
    .map_err(|err| {
        Error::msg(format!(
            "child overlay failed: mount_point={mount_point}: {err}"
        ))
    })?;
    if register_unmountable {
        send_unmountable(Path::new(mount_point));
    }
    on_effect(MountEffect::Target(mount_point.to_owned()));
    Ok(())
}

/// 挂载根 overlay 并重建其子挂载点;失败时由流水线事务回滚已登记目标。
#[cfg(any(target_os = "linux", target_os = "android"))]
#[allow(clippy::too_many_arguments)]
pub fn mount_overlay(
    root: &str,
    module_roots: &[String],
    workdir: Option<PathBuf>,
    upperdir: Option<PathBuf>,
    staging_root: &Path,
    mount_source: &str,
    register_unmountable: bool,
    on_effect: &mut dyn FnMut(MountEffect),
) -> Result<()> {
    log::debug!("overlay mount root: target={root}");

    let root_path = Path::new(root);
    let _current_dir = CurrentDirGuard::change_to(root_path)?;
    let stock_root = ".";
    let mount_seq = collect_child_mount_points(root_path)?;

    let root_result = mount_overlayfs(
        module_roots,
        root,
        upperdir,
        workdir,
        root_path,
        staging_root,
        mount_source,
        on_effect,
    );

    if let Err(err) = root_result {
        return Err(Error::msg(format!(
            "mount overlayfs for root failed: {err}"
        )));
    }
    on_effect(MountEffect::Target(root.to_owned()));

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
            staging_root,
            mount_source,
            register_unmountable,
            on_effect,
        ) {
            log::warn!(
                "child mount failed, deferring rollback: mount_point={mount_point}, error={err}"
            );
            return Err(err);
        }
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
    fn layer_count_boundaries_match_the_64_layer_contract() {
        for (count, expected_chunks) in [(0, 0), (1, 0), (63, 0), (64, 0), (65, 1), (128, 2)] {
            let layers: Vec<String> = (0..count).map(layer).collect();
            assert_eq!(
                plan_staging_chunks(&layers).len(),
                expected_chunks,
                "unexpected staging plan for {count} layers"
            );
        }
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

    #[cfg(target_os = "linux")]
    #[test]
    fn injected_staging_remove_failure_is_reported() {
        let _fault_guard = crate::sys::faults::test_lock();
        let path = std::env::temp_dir().join(format!(
            "hybrid-mount-overlay-staging-fault-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();

        crate::sys::faults::enable_staging_remove_failure();
        let err = cleanup_staging_mount(path.clone()).unwrap_err();
        crate::sys::faults::reset();

        assert!(err.to_string().contains("injected"), "{err}");
        std::fs::remove_dir_all(&path).ok();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn cleanup_propagates_mountinfo_probe_failures() {
        let _fault_guard = crate::sys::faults::test_lock();
        let path = std::env::temp_dir().join(format!(
            "hybrid-mount-overlay-mountinfo-fault-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();

        crate::sys::faults::enable_mountinfo_read_failure();
        let err = cleanup_staging_mount(path.clone()).unwrap_err();
        crate::sys::faults::reset();

        assert!(err.to_string().contains("mountinfo"), "{err}");
        std::fs::remove_dir_all(&path).ok();
    }
}
