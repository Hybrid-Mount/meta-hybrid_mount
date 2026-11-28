#[cfg(unix)]
use std::collections::BTreeMap;
#[cfg(any(target_os = "linux", target_os = "android"))]
use std::fs;
#[cfg(unix)]
use std::path::{Path, PathBuf};

use crate::defs;
#[cfg(unix)]
use crate::errors::{Error, Result};
#[cfg(unix)]
use crate::mount::model::MountPlan;
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::platform;
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::platform::sys::mountinfo::describe_path_mount;

#[cfg(unix)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ShallowOverlaySource {
    pub(crate) source: PathBuf,
    pub(crate) destination_relative: PathBuf,
}

#[cfg(unix)]
pub(crate) type ShallowOverlaySources = BTreeMap<PathBuf, Vec<ShallowOverlaySource>>;

#[cfg(unix)]
pub(crate) type OverlayExecutionPlan = (Vec<usize>, ShallowOverlaySources);

#[cfg(any(target_os = "linux", target_os = "android"))]
pub(crate) struct OverlayPlans<'a> {
    pub(crate) mount: &'a MountPlan,
    pub(crate) execution: &'a OverlayExecutionPlan,
}

#[cfg(unix)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OverlayDirectoryTarget {
    Existing,
    Missing {
        mount_target: PathBuf,
        destination_relative: PathBuf,
    },
}

/// A directory contributed by a module may not exist in the stock partition
/// (for example `system/product/fonts` on a device without `/product/fonts`).
/// OverlayFS still needs a real mount point, so introduce the missing subtree
/// through the nearest existing non-root ancestor instead.
#[cfg(unix)]
pub(crate) fn resolve_overlay_directory_target(target: &Path) -> Result<OverlayDirectoryTarget> {
    if !target.is_absolute() {
        return Err(Error::msg(format!(
            "overlay target is not absolute: {}",
            target.display()
        )));
    }

    match std::fs::metadata(target) {
        Ok(metadata) if metadata.is_dir() => return Ok(OverlayDirectoryTarget::Existing),
        Ok(_) => {
            return Err(Error::msg(format!(
                "overlay directory target exists but is not a directory: {}",
                target.display()
            )));
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => {
            return Err(Error::msg(format!(
                "inspect overlay target {}: {err}",
                target.display()
            )));
        }
    }

    let mut ancestor = target.parent();
    while let Some(candidate) = ancestor {
        // Mounting a synthetic layer over `/` is far too broad and indicates
        // that the expected Android partition root itself is unavailable.
        if candidate == Path::new("/") {
            break;
        }

        match std::fs::metadata(candidate) {
            Ok(metadata) if metadata.is_dir() => {
                let destination_relative = target
                    .strip_prefix(candidate)
                    .map_err(|err| {
                        Error::msg(format!(
                            "derive shallow overlay path {} below {}: {err}",
                            target.display(),
                            candidate.display()
                        ))
                    })?
                    .to_path_buf();
                if destination_relative.as_os_str().is_empty() {
                    return Err(Error::msg(format!(
                        "empty shallow overlay destination for {}",
                        target.display()
                    )));
                }
                return Ok(OverlayDirectoryTarget::Missing {
                    mount_target: candidate.to_path_buf(),
                    destination_relative,
                });
            }
            Ok(_) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                return Err(Error::msg(format!(
                    "inspect overlay ancestor {}: {err}",
                    candidate.display()
                )));
            }
        }
        ancestor = candidate.parent();
    }

    Err(Error::msg(format!(
        "overlay target has no existing non-root ancestor: {}",
        target.display()
    )))
}

/// OverlayFS takes the merged directory inode metadata from the highest
/// lowerdir. A module directory carrying `adb_data_file` (or any other
/// unsuitable label) must therefore not be allowed to relabel an existing
/// stock mount target such as `/system/bin`.
#[cfg(unix)]
fn normalize_direct_overlay_layer_metadata_with(
    plan: &MountPlan,
    mut clone_metadata: impl FnMut(&Path, &Path) -> Result<()>,
) -> Result<usize> {
    let mut normalized = 0;

    for operation in &plan.overlay_ops {
        let target = Path::new(&operation.target);
        if resolve_overlay_directory_target(target)? != OverlayDirectoryTarget::Existing {
            continue;
        }

        for lowerdir in &operation.lowerdirs {
            clone_metadata(target, lowerdir)?;
            normalized += 1;
        }
    }

    Ok(normalized)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub(crate) fn normalize_direct_overlay_layer_metadata(plan: &MountPlan) -> Result<usize> {
    normalize_direct_overlay_layer_metadata_with(plan, |target, lowerdir| {
        crate::platform::sys::fs::clone_directory_metadata(target, lowerdir).map_err(|err| {
            Error::msg(format!(
                "normalize direct overlay layer metadata {} -> {}: {err}",
                target.display(),
                lowerdir.display()
            ))
        })?;
        log::debug!(
            "overlay layer root metadata normalized: target={}, layer={}",
            target.display(),
            lowerdir.display()
        );
        Ok(())
    })
}

#[cfg(unix)]
pub(crate) fn build_overlay_execution_plan(plan: &MountPlan) -> Result<OverlayExecutionPlan> {
    let mut direct_operations = Vec::new();
    let mut shallow = ShallowOverlaySources::new();

    for (target, sources) in &plan.overlay_files {
        let target_path = Path::new(target);
        let (mount_target, prefix) = match resolve_overlay_directory_target(target_path)? {
            OverlayDirectoryTarget::Existing => (target_path.to_path_buf(), PathBuf::new()),
            OverlayDirectoryTarget::Missing {
                mount_target,
                destination_relative,
            } => {
                log::info!(
                    "shallow overlay parent rerouted: requested_target={}, mount_target={}, relative={}",
                    target_path.display(),
                    mount_target.display(),
                    destination_relative.display()
                );
                (mount_target, destination_relative)
            }
        };

        for source in sources {
            let file_name = source.file_name().ok_or_else(|| {
                Error::msg(format!(
                    "overlay file source has no file name: {}",
                    source.display()
                ))
            })?;
            shallow
                .entry(mount_target.clone())
                .or_default()
                .push(ShallowOverlaySource {
                    source: source.clone(),
                    destination_relative: prefix.join(file_name),
                });
        }
    }

    for (index, operation) in plan.overlay_ops.iter().enumerate() {
        match resolve_overlay_directory_target(Path::new(&operation.target))? {
            OverlayDirectoryTarget::Existing => direct_operations.push(index),
            OverlayDirectoryTarget::Missing {
                mount_target,
                destination_relative,
            } => {
                log::info!(
                    "overlay directory rerouted to shallow parent: index={}, requested_target={}, mount_target={}, relative={}, layers={}",
                    index,
                    operation.target,
                    mount_target.display(),
                    destination_relative.display(),
                    operation.lowerdirs.len()
                );
                let entries = shallow.entry(mount_target).or_default();
                entries.extend(operation.lowerdirs.iter().cloned().map(|source| {
                    ShallowOverlaySource {
                        source,
                        destination_relative: destination_relative.clone(),
                    }
                }));
            }
        }
    }

    for entries in shallow.values_mut() {
        entries.sort_by(|left, right| {
            left.source
                .cmp(&right.source)
                .then_with(|| left.destination_relative.cmp(&right.destination_relative))
        });
    }

    Ok((direct_operations, shallow))
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub(crate) fn mount_layers(
    plans: OverlayPlans<'_>,
    register_unmountable: bool,
    storage_root: Option<&Path>,
    transient_root: Option<&Path>,
    effective_mount_source: &str,
    on_effect: &mut dyn FnMut(super::exec::MountEffect),
) -> Result<(usize, usize, Vec<String>)> {
    use super::exec::mount_overlay;

    let plan = plans.mount;
    let mut overlay_dir_mounts = 0;
    let mut shallow_overlay_mounts = 0;
    let mut active_mounts = Vec::new();
    let (direct_operations, shallow) = plans.execution;

    for &operation_index in direct_operations {
        let op = &plan.overlay_ops[operation_index];
        let lowerdirs = op
            .lowerdirs
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        let staging_root = transient_root
            .ok_or_else(|| Error::msg("overlay mount requires a runtime temporary session"))?;
        let mount_source = overlay_mount_source(&op.target, effective_mount_source);
        log::debug!(
            "overlay apply start: index={}, partition={}, target={}, source={}, layers={}, register_unmountable={}",
            operation_index,
            op.partition,
            op.target,
            mount_source,
            lowerdirs.len(),
            register_unmountable
        );
        for (layer_index, lowerdir) in op.lowerdirs.iter().enumerate() {
            log::debug!(
                "overlay apply lowerdir: operation={}, layer={}, path={}, exists={}, is_dir={}, mount={}",
                operation_index,
                layer_index,
                lowerdir.display(),
                lowerdir.exists(),
                lowerdir.is_dir(),
                describe_path_mount(lowerdir)
            );
        }
        if crate::platform::sys::faults::should_fail_next_overlay_mount() {
            return Err(Error::msg(format!(
                "injected overlay mount failure: target={}",
                op.target
            )));
        }
        mount_overlay(
            &op.target,
            &lowerdirs,
            None,
            None,
            staging_root,
            mount_source,
            register_unmountable,
            on_effect,
        )
        .map_err(|err| {
            Error::msg(format!(
                "overlay mount failed: partition={}, target={}: {err}",
                op.partition, op.target
            ))
        })?;
        if register_unmountable {
            platform::ksu::send_unmountable(Path::new(&op.target));
        }
        log::debug!(
            "overlay apply complete: index={}, target={}, target_mount={}",
            operation_index,
            op.target,
            describe_path_mount(Path::new(&op.target))
        );
        active_mounts.push(op.target.clone());
        overlay_dir_mounts += 1;
    }

    if !shallow.is_empty() {
        let storage_root = storage_root
            .ok_or_else(|| Error::msg("shallow overlays require prepared overlay storage"))?;
        shallow_overlay_mounts = mount_overlay_files(
            shallow,
            register_unmountable,
            storage_root,
            transient_root.ok_or_else(|| {
                Error::msg("shallow overlay requires a runtime temporary session")
            })?,
            effective_mount_source,
            &mut active_mounts,
            on_effect,
        )?;
    }

    active_mounts.sort();
    active_mounts.dedup();
    Ok((overlay_dir_mounts, shallow_overlay_mounts, active_mounts))
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn mount_overlay_files(
    files: &ShallowOverlaySources,
    register_unmountable: bool,
    storage_root: &Path,
    transient_root: &Path,
    effective_mount_source: &str,
    active_mounts: &mut Vec<String>,
    on_effect: &mut dyn FnMut(crate::mount::overlayfs::exec::MountEffect),
) -> Result<usize> {
    use crate::mount::overlayfs::exec::mount_overlay;

    let staging_root = crate::platform::sys::temp::create_random_dir(storage_root)?;

    let mut overlay_mounts = 0;
    let mut total_layers = 0;
    log::info!(
        "shallow overlay phase start: targets={}, staging_root={}, staging_mount={}",
        files.len(),
        staging_root.display(),
        describe_path_mount(storage_root)
    );
    for (target_index, (target, sources)) in files.iter().enumerate() {
        let target_string = target.to_string_lossy();
        log::debug!(
            "shallow overlay prepare: index={}, target={}, sources={}",
            target_index,
            target.display(),
            sources.len()
        );
        let mut lowerdirs = Vec::new();
        for (index, entry) in sources.iter().enumerate() {
            let layer_dir = crate::platform::sys::temp::create_random_dir(&staging_root)?;
            let dest =
                prepare_shallow_destination(target, &layer_dir, &entry.destination_relative)?;
            log::debug!(
                "shallow overlay source: target_index={}, layer={}, source={}, exists={}, source_mount={}, relative={}, destination={}",
                target_index,
                index,
                entry.source.display(),
                entry.source.exists(),
                describe_path_mount(&entry.source),
                entry.destination_relative.display(),
                dest.display()
            );
            crate::mount::overlayfs::staging::copy_prepared_entry(&entry.source, &dest)?;

            lowerdirs.push(layer_dir.to_string_lossy().into_owned());
        }

        let mount_source = overlay_mount_source(&target_string, effective_mount_source);
        if crate::platform::sys::faults::should_fail_next_overlay_mount() {
            return Err(Error::msg(format!(
                "injected shallow overlay mount failure: target={}",
                target.display()
            )));
        }
        mount_overlay(
            &target_string,
            &lowerdirs,
            None,
            None,
            transient_root,
            mount_source,
            register_unmountable,
            on_effect,
        )
        .map_err(|err| {
            Error::msg(format!(
                "shallow overlay mount failed: target={}: {err}",
                target.display()
            ))
        })?;
        if register_unmountable {
            platform::ksu::send_unmountable(target);
        }
        log::debug!(
            "shallow overlay complete: index={}, target={}, layers={}, target_mount={}",
            target_index,
            target.display(),
            lowerdirs.len(),
            describe_path_mount(target)
        );
        active_mounts.push(target_string.into_owned());
        total_layers += lowerdirs.len();
        overlay_mounts += 1;
    }

    log::info!(
        "shallow overlay phase complete: targets={}, layers={}",
        files.len(),
        total_layers
    );
    Ok(overlay_mounts)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub(crate) fn prepare_shallow_destination(
    target: &Path,
    layer_dir: &Path,
    destination_relative: &Path,
) -> Result<PathBuf> {
    if destination_relative.as_os_str().is_empty() || destination_relative.is_absolute() {
        return Err(Error::msg(format!(
            "invalid shallow overlay destination: {}",
            destination_relative.display()
        )));
    }

    crate::platform::sys::fs::clone_directory_metadata(target, layer_dir)?;

    let mut structural_dir = layer_dir.to_path_buf();
    if let Some(parent) = destination_relative.parent() {
        for component in parent.components() {
            let std::path::Component::Normal(name) = component else {
                return Err(Error::msg(format!(
                    "unsafe shallow overlay destination: {}",
                    destination_relative.display()
                )));
            };
            structural_dir.push(name);
            fs::create_dir(&structural_dir)?;
            crate::platform::sys::fs::clone_directory_metadata(target, &structural_dir)?;
        }
    }

    Ok(layer_dir.join(destination_relative))
}

pub(crate) fn overlay_mount_source<'a>(target: &str, configured: &'a str) -> &'a str {
    if defs::IGNORE_UNMOUNT_PARTITIONS
        .iter()
        .any(|ignored| ignored.trim() == target.trim())
    {
        "overlay"
    } else {
        configured
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn existing_overlay_directory_stays_direct() {
        let fixture = std::env::temp_dir().join(format!(
            "hybrid-mount-existing-overlay-target-{}",
            std::process::id()
        ));
        let target = fixture.join("product/fonts");
        std::fs::remove_dir_all(&fixture).ok();
        std::fs::create_dir_all(&target).unwrap();

        assert_eq!(
            resolve_overlay_directory_target(&target).unwrap(),
            OverlayDirectoryTarget::Existing
        );

        std::fs::remove_dir_all(&fixture).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn missing_overlay_directory_uses_nearest_existing_parent() {
        let fixture = std::env::temp_dir().join(format!(
            "hybrid-mount-missing-overlay-target-{}",
            std::process::id()
        ));
        let product = fixture.join("product");
        let target = product.join("fonts/google");
        std::fs::remove_dir_all(&fixture).ok();
        std::fs::create_dir_all(&product).unwrap();

        assert_eq!(
            resolve_overlay_directory_target(&target).unwrap(),
            OverlayDirectoryTarget::Missing {
                mount_target: product,
                destination_relative: PathBuf::from("fonts/google"),
            }
        );

        std::fs::remove_dir_all(&fixture).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn non_directory_overlay_target_is_rejected() {
        let fixture = std::env::temp_dir().join(format!(
            "hybrid-mount-file-overlay-target-{}",
            std::process::id()
        ));
        let target = fixture.join("product/fonts");
        std::fs::remove_dir_all(&fixture).ok();
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, "not a directory").unwrap();

        let err = resolve_overlay_directory_target(&target).unwrap_err();
        assert!(err.to_string().contains("not a directory"), "{err}");

        std::fs::remove_dir_all(&fixture).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn direct_overlay_layer_root_uses_stock_directory_label() {
        use crate::mount::model::OverlayOperation;
        use std::collections::BTreeMap;

        let fixture = std::env::temp_dir().join(format!(
            "hybrid-mount-overlay-root-label-{}",
            std::process::id()
        ));
        let stock_bin = fixture.join("system/bin");
        let droidspaces_bin = fixture.join("staging/droidspaces/system/bin");
        let other_bin = fixture.join("staging/other/system/bin");
        std::fs::remove_dir_all(&fixture).ok();
        std::fs::create_dir_all(&stock_bin).unwrap();
        std::fs::create_dir_all(&droidspaces_bin).unwrap();
        std::fs::create_dir_all(&other_bin).unwrap();

        let mut plan = MountPlan::default();
        plan.overlay_ops.push(OverlayOperation {
            partition: "system".to_owned(),
            target: stock_bin.to_string_lossy().into_owned(),
            lowerdirs: vec![droidspaces_bin.clone(), other_bin.clone()],
        });

        let system_file = "u:object_r:system_file:s0".to_owned();
        let adb_data_file = "u:object_r:adb_data_file:s0".to_owned();
        let mut labels = BTreeMap::from([
            (stock_bin.clone(), system_file.clone()),
            (droidspaces_bin.clone(), adb_data_file.clone()),
            (other_bin.clone(), adb_data_file),
        ]);

        let normalized =
            normalize_direct_overlay_layer_metadata_with(&plan, |source, destination| {
                let label = labels.get(source).cloned().ok_or_else(|| {
                    Error::msg(format!("missing test label for {}", source.display()))
                })?;
                labels.insert(destination.to_path_buf(), label);
                Ok(())
            })
            .unwrap();

        assert_eq!(normalized, 2);
        assert_eq!(labels.get(&droidspaces_bin), Some(&system_file));
        assert_eq!(labels.get(&other_bin), Some(&system_file));

        std::fs::remove_dir_all(&fixture).unwrap();
    }

    #[test]
    fn ignored_overlay_partition_uses_neutral_mount_source() {
        assert_eq!(overlay_mount_source("/system/lib", "KSU"), "overlay");
        assert_eq!(overlay_mount_source("/vendor/lib64", "APatch"), "overlay");
    }

    #[test]
    fn regular_overlay_partition_keeps_backend_mount_source() {
        assert_eq!(overlay_mount_source("/system", "KSU"), "KSU");
        assert_eq!(overlay_mount_source("/product", "APatch"), "APatch");
    }
}
