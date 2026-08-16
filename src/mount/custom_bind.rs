// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
#[cfg(any(target_os = "linux", target_os = "android"))]
use rustix::mount::{MountFlags, mount_bind, mount_remount};

use crate::{
    conf::schema::CustomBindMount,
    sys::mount::{MountRollback, detach_mount},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomBindKind {
    File,
    Directory,
}

#[derive(Debug, Clone)]
pub struct AppliedCustomBind {
    pub source: PathBuf,
    pub target: PathBuf,
    pub kind: CustomBindKind,
}

pub fn apply_custom_bind_mounts(
    mounts: &[CustomBindMount],
    disable_umount: bool,
) -> Result<Vec<AppliedCustomBind>> {
    apply_custom_bind_mounts_with(
        mounts,
        disable_umount,
        apply_one,
        |target| crate::mount::umount_mgr::send_umountable(target),
        detach_mount,
    )
}

fn apply_custom_bind_mounts_with<A, R, D>(
    mounts: &[CustomBindMount],
    disable_umount: bool,
    mut apply: A,
    mut register_umountable: R,
    mut detach: D,
) -> Result<Vec<AppliedCustomBind>>
where
    A: FnMut(&CustomBindMount) -> Result<AppliedCustomBind>,
    R: FnMut(&Path) -> Result<()>,
    D: FnMut(&Path) -> Result<()>,
{
    validate_target_order(mounts)?;

    let mut applied_mounts = Vec::with_capacity(mounts.len());
    let mut rollback = MountRollback::default();

    for mount in mounts {
        let applied = match apply(mount).with_context(|| {
            format!(
                "failed to apply custom bind {} -> {}",
                mount.source.display(),
                mount.target.display()
            )
        }) {
            Ok(applied) => applied,
            Err(error) => {
                return Err(attach_batch_rollback(&mut rollback, &mut detach, error));
            }
        };
        crate::scoped_log!(
            info,
            "custom_bind",
            "mounted: source={}, target={}",
            applied.source.display(),
            applied.target.display()
        );
        rollback.record(applied.target.clone());
        applied_mounts.push(applied);
    }

    if !disable_umount {
        for target in registration_targets(&applied_mounts) {
            if let Err(error) = register_umountable(target).with_context(|| {
                format!(
                    "failed to register custom bind target as umountable: {}",
                    target.display()
                )
            }) {
                return Err(attach_batch_rollback(&mut rollback, &mut detach, error));
            }
        }
    }

    Ok(applied_mounts)
}

fn validate_target_order(mounts: &[CustomBindMount]) -> Result<()> {
    let mut unique_targets = HashSet::with_capacity(mounts.len());
    let mut earlier_targets: Vec<&Path> = Vec::with_capacity(mounts.len());

    for mount in mounts {
        if !unique_targets.insert(mount.target.as_path()) {
            bail!(
                "duplicate custom bind target is not allowed: {}",
                mount.target.display()
            );
        }

        if let Some(earlier_target) = earlier_targets
            .iter()
            .find(|earlier_target| earlier_target.starts_with(mount.target.as_path()))
        {
            bail!(
                "unsafe custom bind target order: later target {} is an ancestor of earlier target {}; configure parent targets before their children",
                mount.target.display(),
                earlier_target.display()
            );
        }

        earlier_targets.push(mount.target.as_path());
    }

    Ok(())
}

fn registration_targets(mounts: &[AppliedCustomBind]) -> Vec<&Path> {
    let mut targets = mounts
        .iter()
        .map(|mount| mount.target.as_path())
        .collect::<Vec<_>>();

    // KernelSU processes the list in reverse. Register parents first so the
    // resulting detach order is children before parents.
    targets.sort_unstable_by(|left, right| {
        left.components()
            .count()
            .cmp(&right.components().count())
            .then_with(|| left.cmp(right))
    });
    targets
}

fn attach_batch_rollback<D>(
    rollback: &mut MountRollback,
    detach: &mut D,
    error: anyhow::Error,
) -> anyhow::Error
where
    D: FnMut(&Path) -> Result<()>,
{
    match rollback.rollback_with(detach) {
        Ok(()) => error,
        Err(rollback_error) => error.context(format!(
            "additionally failed to roll back custom bind mounts: {rollback_error:#}"
        )),
    }
}

fn apply_one(mount: &CustomBindMount) -> Result<AppliedCustomBind> {
    let kind = validate_mount_paths(&mount.source, &mount.target)?;
    bind_mount_checked(&mount.source, &mount.target)?;

    Ok(AppliedCustomBind {
        source: mount.source.clone(),
        target: mount.target.clone(),
        kind,
    })
}

fn validate_mount_paths(source: &Path, target: &Path) -> Result<CustomBindKind> {
    if !source.is_absolute() {
        bail!("custom bind source must be an absolute path");
    }
    if !target.is_absolute() {
        bail!("custom bind target must be an absolute path");
    }
    if source == target {
        bail!("custom bind source and target must differ");
    }

    let source_meta = fs::metadata(source)
        .with_context(|| format!("failed to inspect source {}", source.display()))?;
    let target_meta = fs::metadata(target)
        .with_context(|| format!("failed to inspect target {}", target.display()))?;

    match (source_meta.is_dir(), target_meta.is_dir()) {
        (true, true) => Ok(CustomBindKind::Directory),
        (false, false) => Ok(CustomBindKind::File),
        (true, false) => bail!("custom bind source is a directory but target is not"),
        (false, true) => bail!("custom bind source is not a directory but target is"),
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn bind_mount_checked(source: &Path, target: &Path) -> Result<()> {
    mount_bind(source, target).with_context(|| {
        format!(
            "failed to bind mount {} to {}",
            source.display(),
            target.display()
        )
    })?;

    finish_bound_mount_with(
        target,
        || {
            mount_remount(target, MountFlags::RDONLY | MountFlags::BIND, "").with_context(
                || {
                    format!(
                        "failed to remount custom bind readonly: {}",
                        target.display()
                    )
                },
            )?;
            Ok(())
        },
        detach_mount,
    )
}

fn finish_bound_mount_with<F, D>(target: &Path, finish: F, mut detach: D) -> Result<()>
where
    F: FnOnce() -> Result<()>,
    D: FnMut(&Path) -> Result<()>,
{
    let Err(error) = finish() else {
        return Ok(());
    };

    match detach(target) {
        Ok(()) => Err(error),
        Err(rollback_error) => Err(error.context(format!(
            "additionally failed to detach custom bind {}: {rollback_error:#}",
            target.display()
        ))),
    }
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn bind_mount_checked(_source: &Path, _target: &Path) -> Result<()> {
    bail!("custom bind mounts are only supported on linux/android")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_rejects_relative_paths() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("target");
        fs::write(&target, b"").unwrap();

        assert!(validate_mount_paths(Path::new("relative"), &target).is_err());
        assert!(validate_mount_paths(&target, Path::new("relative")).is_err());
    }

    #[test]
    fn validate_rejects_type_mismatch() {
        let temp = tempfile::tempdir().unwrap();
        let source_dir = temp.path().join("source");
        let target_file = temp.path().join("target");
        fs::create_dir(&source_dir).unwrap();
        fs::write(&target_file, b"").unwrap();

        assert!(validate_mount_paths(&source_dir, &target_file).is_err());
    }

    #[test]
    fn validate_accepts_file_to_file() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        fs::write(&source, b"").unwrap();
        fs::write(&target, b"").unwrap();

        assert_eq!(
            validate_mount_paths(&source, &target).unwrap(),
            CustomBindKind::File
        );
    }

    #[test]
    fn validate_accepts_dir_to_dir() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&target).unwrap();

        assert_eq!(
            validate_mount_paths(&source, &target).unwrap(),
            CustomBindKind::Directory
        );
    }

    #[test]
    fn finish_failure_detaches_the_new_bind_and_preserves_errors() {
        let target = Path::new("/system/etc/hosts");
        let mut detached = Vec::new();

        let error = finish_bound_mount_with(
            target,
            || anyhow::bail!("readonly remount failed"),
            |path| {
                detached.push(path.to_path_buf());
                anyhow::bail!("detach failed")
            },
        )
        .unwrap_err();

        assert_eq!(detached, vec![target.to_path_buf()]);
        let chain = format!("{error:#}");
        assert!(chain.contains("readonly remount failed"));
        assert!(chain.contains("detach failed"));
    }

    #[test]
    fn batch_failure_rolls_back_prior_binds_in_reverse_order() {
        let mounts = vec![
            CustomBindMount {
                source: PathBuf::from("/source/one"),
                target: PathBuf::from("/target/one"),
            },
            CustomBindMount {
                source: PathBuf::from("/source/two"),
                target: PathBuf::from("/target/two"),
            },
            CustomBindMount {
                source: PathBuf::from("/source/three"),
                target: PathBuf::from("/target/three"),
            },
        ];
        let mut applied = 0usize;
        let mut detached = Vec::new();

        let error = apply_custom_bind_mounts_with(
            &mounts,
            false,
            |mount| {
                applied += 1;
                if applied == 3 {
                    anyhow::bail!("third bind failed");
                }
                Ok(AppliedCustomBind {
                    source: mount.source.clone(),
                    target: mount.target.clone(),
                    kind: CustomBindKind::File,
                })
            },
            |_| Ok(()),
            |path| {
                detached.push(path.to_path_buf());
                Ok(())
            },
        )
        .unwrap_err();

        assert!(format!("{error:#}").contains("third bind failed"));
        assert_eq!(
            detached,
            vec![PathBuf::from("/target/two"), PathBuf::from("/target/one")]
        );
    }

    #[test]
    fn duplicate_targets_are_rejected_before_any_mount_is_applied() {
        let mounts = vec![
            CustomBindMount {
                source: PathBuf::from("/source/one"),
                target: PathBuf::from("/target/shared"),
            },
            CustomBindMount {
                source: PathBuf::from("/source/two"),
                target: PathBuf::from("/target/shared"),
            },
        ];
        let mut apply_calls = 0usize;
        let mut registration_calls = 0usize;
        let mut detach_calls = 0usize;

        let error = apply_custom_bind_mounts_with(
            &mounts,
            false,
            |_| {
                apply_calls += 1;
                unreachable!("duplicate preflight must run before mounting")
            },
            |_| {
                registration_calls += 1;
                Ok(())
            },
            |_| {
                detach_calls += 1;
                Ok(())
            },
        )
        .unwrap_err();

        assert!(format!("{error:#}").contains("duplicate custom bind target"));
        assert_eq!(apply_calls, 0);
        assert_eq!(registration_calls, 0);
        assert_eq!(detach_calls, 0);
    }

    #[test]
    fn child_before_parent_is_rejected_before_any_mount_is_applied() {
        let mounts = vec![
            CustomBindMount {
                source: PathBuf::from("/source/hosts"),
                target: PathBuf::from("/system/etc/hosts"),
            },
            CustomBindMount {
                source: PathBuf::from("/source/system"),
                target: PathBuf::from("/system"),
            },
        ];
        let mut apply_calls = 0usize;

        let error = apply_custom_bind_mounts_with(
            &mounts,
            false,
            |_| {
                apply_calls += 1;
                unreachable!("target-order preflight must run before mounting")
            },
            |_| unreachable!("registration cannot precede target-order validation"),
            |_| unreachable!("no mount exists to detach after preflight failure"),
        )
        .unwrap_err();

        let chain = format!("{error:#}");
        assert!(chain.contains("unsafe custom bind target order"));
        assert!(chain.contains("/system/etc/hosts"));
        assert!(chain.contains("/system"));
        assert_eq!(apply_calls, 0);
    }

    #[test]
    fn parent_before_child_and_non_nested_targets_are_allowed() {
        let mounts = [
            ("/source/system", "/system"),
            ("/source/hosts", "/system/etc/hosts"),
            ("/source/system-ext", "/system_ext"),
            ("/source/vendor", "/vendor"),
        ]
        .map(|(source, target)| CustomBindMount {
            source: PathBuf::from(source),
            target: PathBuf::from(target),
        });

        validate_target_order(&mounts).unwrap();
    }

    #[test]
    fn batch_applies_in_config_order_then_registers_shallow_to_deep() {
        let targets = [
            "/vendor",
            "/vendor/etc",
            "/system",
            "/system/etc/hosts",
            "/system/bin",
        ];
        let mounts = targets
            .iter()
            .enumerate()
            .map(|(index, target)| CustomBindMount {
                source: PathBuf::from(format!("/source/{index}")),
                target: PathBuf::from(target),
            })
            .collect::<Vec<_>>();
        let mut applied_order = Vec::new();
        let mut registration_order = Vec::new();

        let applied = apply_custom_bind_mounts_with(
            &mounts,
            false,
            |mount| {
                applied_order.push(mount.target.clone());
                Ok(AppliedCustomBind {
                    source: mount.source.clone(),
                    target: mount.target.clone(),
                    kind: CustomBindKind::File,
                })
            },
            |target| {
                registration_order.push(target.to_path_buf());
                Ok(())
            },
            |_| Ok(()),
        )
        .unwrap();

        assert_eq!(
            applied_order,
            targets.map(PathBuf::from),
            "mount application must preserve config order"
        );
        assert_eq!(applied.len(), targets.len());
        assert_eq!(
            registration_order,
            [
                "/system",
                "/vendor",
                "/system/bin",
                "/vendor/etc",
                "/system/etc/hosts",
            ]
            .map(PathBuf::from),
            "KernelSU reverses this list, yielding deep-to-shallow detach order"
        );
    }

    #[test]
    fn registration_failure_rolls_back_batch_and_preserves_both_errors() {
        let mounts = [
            ("/source/system", "/system"),
            ("/source/hosts", "/system/etc/hosts"),
            ("/source/vendor", "/vendor"),
        ]
        .map(|(source, target)| CustomBindMount {
            source: PathBuf::from(source),
            target: PathBuf::from(target),
        });
        let mut registered = Vec::new();
        let mut detached = Vec::new();

        let error = apply_custom_bind_mounts_with(
            &mounts,
            false,
            |mount| {
                Ok(AppliedCustomBind {
                    source: mount.source.clone(),
                    target: mount.target.clone(),
                    kind: CustomBindKind::File,
                })
            },
            |target| {
                registered.push(target.to_path_buf());
                if target == Path::new("/vendor") {
                    anyhow::bail!("registration transport failed");
                }
                Ok(())
            },
            |target| {
                detached.push(target.to_path_buf());
                if target == Path::new("/system/etc/hosts") {
                    anyhow::bail!("detach transport failed");
                }
                Ok(())
            },
        )
        .unwrap_err();

        assert_eq!(
            registered,
            ["/system", "/vendor"].map(PathBuf::from),
            "registration must not begin until all mounts have succeeded"
        );
        assert_eq!(
            detached,
            ["/vendor", "/system/etc/hosts", "/system"].map(PathBuf::from)
        );
        let chain = format!("{error:#}");
        assert!(chain.contains("failed to register custom bind target as umountable"));
        assert!(chain.contains("registration transport failed"));
        assert!(chain.contains("detach transport failed"));
    }

    #[test]
    fn disabled_umount_skips_batch_registration() {
        let mounts = [CustomBindMount {
            source: PathBuf::from("/source/one"),
            target: PathBuf::from("/target/one"),
        }];

        let applied = apply_custom_bind_mounts_with(
            &mounts,
            true,
            |mount| {
                Ok(AppliedCustomBind {
                    source: mount.source.clone(),
                    target: mount.target.clone(),
                    kind: CustomBindKind::File,
                })
            },
            |_| unreachable!("registration must be skipped when disable_umount is set"),
            |_| Ok(()),
        )
        .unwrap();

        assert_eq!(applied.len(), 1);
    }
}
