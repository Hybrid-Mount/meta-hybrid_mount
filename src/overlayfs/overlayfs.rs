// SPDX-License-Identifier: GPL-3.0-only

//! OverlayFS mount orchestration (behaviour aligned with v4.2.0 `e20f9c19`):
//! - fsopen("overlay") is the primary path, with an escaped traditional `mount(2)` fallback;
//! - past 64 lowerdirs, the trailing layers are stacked into staging first and used as a new layer;
//! - child mount views are prepared in staging before one recursive attachment;
//!   failure never publishes a parent which lost its child mounts.

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
    // The normal cleanup path must not treat a failed mountinfo lookup as "not mounted".
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

/// Maximum layers accepted in a single overlayfs mount (v4.2.0 behaviour).
pub const MAX_LAYERS: usize = 64;

/// Escapes `\`, `,` and `:` inside a single lowerdir path, for the traditional mount fallback only.
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

/// Collapse low-priority tails until one OverlayFS mount can hold the result.
/// Each generated staging layer is inserted immediately so the next iteration
/// includes it instead of planning against stale original inputs.
fn collapse_staging_layers<E>(
    current_layers: &mut Vec<String>,
    mut create_staging_layer: impl FnMut(&[String]) -> std::result::Result<String, E>,
) -> std::result::Result<(), E> {
    while current_layers.len() > MAX_LAYERS {
        let split_idx = current_layers.len() - (MAX_LAYERS - 1);
        let staging_layers = current_layers.split_off(split_idx);
        let staging_layer = create_staging_layer(&staging_layers)?;
        current_layers.push(staging_layer);
    }

    Ok(())
}

/// Computes a sub-mount's path relative to the root mountpoint; `None` when it is not a descendant.
pub fn child_relative_path(root: &str, mount_point: &str) -> Option<String> {
    let relative = std::path::Path::new(mount_point).strip_prefix(root).ok()?;
    (!relative.as_os_str().is_empty()).then(|| format!("/{}", relative.to_string_lossy()))
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn collect_child_mount_points(root_path: &Path) -> Result<Vec<String>> {
    let mounts = crate::sys::mountinfo::mount_entries()?;

    let mut mount_seq: Vec<String> = mounts
        .into_iter()
        .filter(|entry| {
            child_relative_path(
                &root_path.to_string_lossy(),
                &entry.mount_point.to_string_lossy(),
            )
            .is_some()
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

/// Stacks lowerdirs plus lowest onto dest, staging first when there are more than 64 layers.
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

    collapse_staging_layers(&mut current_layers, |staging_layers| {
        let staging_dir = crate::sys::temp::create_random_dir(staging_root)?;
        mount_overlay_core(staging_layers, None, None, &staging_dir, mount_source)?;
        log::debug!(
            "staging layer created: path={}, input_layers={}",
            staging_dir.display(),
            staging_layers.len()
        );
        let staging_layer = staging_dir.to_string_lossy().into_owned();
        on_effect(MountEffect::Staging(staging_dir));
        Ok::<_, Error>(staging_layer)
    })?;

    mount_overlay_core(
        &current_layers,
        upperdir.as_deref(),
        workdir.as_deref(),
        dest,
        mount_source,
    )
}

/// Recursive bind mount: open_tree + move_mount first, falling back to a traditional bind (v4.2.0 behaviour).
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

    // Both failure paths below need the same traditional bind, with the same error text.
    let bind = || -> Result<()> {
        mount(from, to, "", MountFlags::BIND | MountFlags::REC, None).map_err(|err| {
            Error::msg(format!(
                "bind mount {} -> {}: {err}",
                from.display(),
                to.display()
            ))
        })
    };

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
                bind()?;
            }
        }
        Err(_) => {
            bind()?;
        }
    }

    Ok(())
}

/// Prepare the complete visible subtree away from the live target. Native
/// child mounts are cloned recursively; modified children are prepared against
/// their original mount, never against an already-covered relative path.
#[cfg(any(target_os = "linux", target_os = "android"))]
#[allow(clippy::too_many_arguments)]
fn prepare_overlay_view(
    destination: &Path,
    stock: &Path,
    module_roots: &[String],
    children: &[String],
    upperdir: Option<PathBuf>,
    workdir: Option<PathBuf>,
    staging_root: &Path,
    mount_source: &str,
    on_effect: &mut dyn FnMut(MountEffect),
) -> Result<()> {
    mount_overlayfs(
        module_roots,
        &stock.to_string_lossy(),
        upperdir,
        workdir,
        destination,
        staging_root,
        mount_source,
        on_effect,
    )?;
    let mut direct = Vec::<PathBuf>::new();
    for child in children
        .iter()
        .map(Path::new)
        .filter(|p| p.starts_with(stock) && *p != stock)
    {
        if direct.iter().any(|parent| child.starts_with(parent)) {
            continue;
        }
        direct.push(child.to_owned());
        let relative = child
            .strip_prefix(stock)
            .map_err(|e| Error::msg(e.to_string()))?;
        let target = destination.join(relative);
        let mut checked = destination.to_owned();
        for component in relative.components() {
            checked.push(component);
            if std::fs::symlink_metadata(&checked)?
                .file_type()
                .is_symlink()
            {
                return Err(Error::msg(format!(
                    "child mount target became a symlink: {}",
                    checked.display()
                )));
            }
        }
        let mut layers = Vec::new();
        for root in module_roots {
            let layer = Path::new(root).join(relative);
            if std::fs::symlink_metadata(&layer).is_ok() {
                layers.push(layer);
            }
        }
        if layers.is_empty() {
            bind_mount(child, &target)?;
        } else {
            if crate::sys::faults::should_fail_next_child_overlay_mount() {
                return Err(Error::msg(format!(
                    "injected child overlay mount failure: {}",
                    child.display()
                )));
            }
            if child.is_dir() {
                if layers.iter().any(|p| !p.is_dir()) {
                    return Err(Error::msg(format!(
                        "module changes child mount type: {}",
                        child.display()
                    )));
                }
                let layers = layers
                    .iter()
                    .map(|p| p.to_string_lossy().into_owned())
                    .collect::<Vec<_>>();
                prepare_overlay_view(
                    &target,
                    child,
                    &layers,
                    children,
                    None,
                    None,
                    staging_root,
                    mount_source,
                    on_effect,
                )?;
            } else if layers[0].is_file()
                && !std::fs::symlink_metadata(&layers[0])?
                    .file_type()
                    .is_symlink()
            {
                bind_mount(&layers[0], &target)?;
            } else {
                return Err(Error::msg(format!(
                    "cannot preserve file child mount: {}",
                    child.display()
                )));
            }
        }
    }
    Ok(())
}

/// Publish only after every child mount is ready. Failure leaves the original
/// live tree intact; staging effects remain owned by the enclosing transaction.
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
    let root_path = Path::new(root);
    let children = collect_child_mount_points(root_path)?;
    if children.is_empty() {
        mount_overlayfs(
            module_roots,
            root,
            upperdir,
            workdir,
            root_path,
            staging_root,
            mount_source,
            on_effect,
        )?;
    } else {
        let prepared = crate::sys::temp::create_random_dir(staging_root)?;
        on_effect(MountEffect::Staging(prepared.clone()));
        prepare_overlay_view(
            &prepared,
            root_path,
            module_roots,
            &children,
            upperdir,
            workdir,
            staging_root,
            mount_source,
            on_effect,
        )?;
        // bind_mount clones the complete recursive tree in one attachment.
        // The original remains untouched until this call succeeds.
        bind_mount(&prepared, root_path)?;
    }
    on_effect(MountEffect::Target(root.to_owned()));
    for child in children {
        if register_unmountable {
            send_unmountable(Path::new(&child));
        }
        on_effect(MountEffect::Target(child));
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
        let mut layers: Vec<String> = (0..MAX_LAYERS).map(layer).collect();
        let mut staging_steps = 0;

        collapse_staging_layers(&mut layers, |_| {
            staging_steps += 1;
            Ok::<_, std::convert::Infallible>(String::new())
        })
        .unwrap();

        assert_eq!(staging_steps, 0);
    }

    #[test]
    fn layer_count_boundaries_match_the_64_layer_contract() {
        for (count, expected_steps) in [
            (0, 0),
            (1, 0),
            (63, 0),
            (64, 0),
            (65, 1),
            (126, 1),
            (127, 2),
            (128, 2),
        ] {
            let mut layers: Vec<String> = (0..count).map(layer).collect();
            let mut chunk_sizes = Vec::new();
            collapse_staging_layers(&mut layers, |staging_layers| {
                chunk_sizes.push(staging_layers.len());
                Ok::<_, std::convert::Infallible>(format!("/staging-{}", chunk_sizes.len()))
            })
            .unwrap();

            assert_eq!(
                (
                    chunk_sizes.len(),
                    chunk_sizes.iter().all(|size| *size <= MAX_LAYERS),
                    layers.len() <= MAX_LAYERS,
                ),
                (expected_steps, true, true),
                "unexpected staging plan for {count} layers"
            );
        }
    }

    #[test]
    fn one_extra_layer_splits_one_chunk() {
        let mut layers: Vec<String> = (0..=MAX_LAYERS).map(layer).collect();
        let mut staging_inputs = Vec::new();
        collapse_staging_layers(&mut layers, |staging_layers| {
            staging_inputs.push(staging_layers.to_vec());
            Ok::<_, std::convert::Infallible>("/staging-0".to_owned())
        })
        .unwrap();

        assert_eq!(
            (layers, staging_inputs),
            (
                vec![layer(0), layer(1), "/staging-0".to_owned()],
                vec![(2..=MAX_LAYERS).map(layer).collect::<Vec<_>>()],
            )
        );
    }

    #[test]
    fn repeated_staging_preserves_every_layer_in_order() {
        for count in 0..=MAX_LAYERS * 4 {
            let original = (0..count)
                .map(|index| format!("[{index}]"))
                .collect::<Vec<_>>();
            let expected = original.concat();
            let mut reduced = original;

            collapse_staging_layers(&mut reduced, |staging_layers| {
                Ok::<_, std::convert::Infallible>(staging_layers.concat())
            })
            .unwrap();

            assert_eq!(
                reduced.concat(),
                expected,
                "staging changed layer coverage or order for {count} layers"
            );
        }
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

    #[cfg(target_os = "linux")]
    #[test]
    fn staged_overlay_preserves_nested_and_file_mounts_and_new_files() {
        let _guard = crate::sys::faults::test_lock();
        if !crate::test_support::require_mount_namespace() {
            return;
        }
        let fixture = crate::test_support::Fixture::new("overlay-staged-view");
        let target = fixture.join("target");
        let child = target.join("child");
        let nested = child.join("nested");
        let layer = fixture.join("layer");
        let staging = fixture.join("staging");
        for path in [&child, &layer.join("child/nested"), &staging] {
            std::fs::create_dir_all(path).unwrap();
        }
        mount("tmpfs", &child, "tmpfs", MountFlags::empty(), None).unwrap();
        std::fs::create_dir(&nested).unwrap();
        mount("tmpfs", &nested, "tmpfs", MountFlags::empty(), None).unwrap();
        std::fs::write(child.join("stock"), "child-stock").unwrap();
        std::fs::write(nested.join("stock"), "nested-stock").unwrap();
        std::fs::write(target.join("file"), "unmounted").unwrap();
        let original_file = fixture.join("original");
        std::fs::write(&original_file, "file-stock").unwrap();
        bind_mount(&original_file, &target.join("file")).unwrap();
        for (path, value) in [
            ("parent-new", "parent-module"),
            ("child/new", "child-module"),
            ("child/nested/new", "nested-module"),
            ("file", "file-module"),
        ] {
            std::fs::write(layer.join(path), value).unwrap();
        }
        let mut effects = Vec::new();
        mount_overlay(
            &target.to_string_lossy(),
            &[layer.to_string_lossy().into_owned()],
            None,
            None,
            &staging,
            "overlay",
            false,
            &mut |e| effects.push(e),
        )
        .unwrap();
        for (path, value) in [
            ("parent-new", "parent-module"),
            ("child/stock", "child-stock"),
            ("child/new", "child-module"),
            ("child/nested/stock", "nested-stock"),
            ("child/nested/new", "nested-module"),
            ("file", "file-module"),
        ] {
            assert_eq!(std::fs::read_to_string(target.join(path)).unwrap(), value);
        }
        let visible = collect_child_mount_points(&target).unwrap();
        assert!(visible.contains(&nested.to_string_lossy().into_owned()));
        assert!(visible.contains(&target.join("file").to_string_lossy().into_owned()));
        unmount(&target, UnmountFlags::DETACH).unwrap();
        assert!(!target.join("parent-new").exists());
        assert_eq!(
            std::fs::read_to_string(child.join("stock")).unwrap(),
            "child-stock"
        );
        assert_eq!(
            std::fs::read_to_string(target.join("file")).unwrap(),
            "file-stock"
        );
        for point in [nested, target.join("file"), child] {
            if crate::sys::mount::is_mounted(&point).unwrap() {
                unmount(&point, UnmountFlags::DETACH).unwrap();
            }
        }
        for effect in effects.into_iter().rev() {
            if let MountEffect::Staging(path) = effect {
                cleanup_staging_mount(path).unwrap();
            }
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn child_failure_never_publishes_the_parent_overlay() {
        let _guard = crate::sys::faults::test_lock();
        if !crate::test_support::require_mount_namespace() {
            return;
        }
        let fixture = crate::test_support::Fixture::new("overlay-atomic-child");
        let target = fixture.join("target");
        let child = target.join("child");
        let layer = fixture.join("layer");
        let staging = fixture.join("staging");
        for path in [&child, &layer.join("child"), &staging] {
            std::fs::create_dir_all(path).unwrap();
        }
        mount("tmpfs", &child, "tmpfs", MountFlags::empty(), None).unwrap();
        std::fs::write(child.join("stock"), "native").unwrap();
        std::fs::write(layer.join("child/new"), "module").unwrap();
        std::fs::write(layer.join("parent-new"), "module").unwrap();
        assert!(
            collect_child_mount_points(&target)
                .unwrap()
                .contains(&child.to_string_lossy().into_owned()),
            "mountinfo must describe the caller's namespace"
        );
        crate::sys::faults::enable_next_child_overlay_mount_failure();
        let mut effects = Vec::new();
        let result = mount_overlay(
            &target.to_string_lossy(),
            &[layer.to_string_lossy().into_owned()],
            None,
            None,
            &staging,
            "overlay",
            false,
            &mut |e| effects.push(e),
        );
        let parent_was_untouched = !target.join("parent-new").exists();
        crate::sys::faults::reset();
        if crate::sys::mount::is_mounted(&target).unwrap() {
            unmount(&target, UnmountFlags::DETACH).unwrap();
        }
        if crate::sys::mount::is_mounted(&child).unwrap() {
            unmount(&child, UnmountFlags::DETACH).unwrap();
        }
        for effect in effects.iter().rev() {
            if let MountEffect::Staging(path) = effect {
                cleanup_staging_mount(path.clone()).unwrap();
            }
        }
        assert!(
            result.is_err(),
            "child failure must not become partial success"
        );
        assert!(
            parent_was_untouched,
            "parent was exposed before children were ready"
        );
        assert!(!effects.iter().any(|e| matches!(e, MountEffect::Target(_))));
    }
}
