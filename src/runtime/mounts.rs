// SPDX-License-Identifier: GPL-3.0-only

//! Exact mount ownership and conservative, repeatable cleanup.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use super::ledger::OwnedMount;
use crate::errors::{ContextError, Error, Result};

fn failure(target: &str, detail: &str) -> Error {
    Error::Mount(Box::new(ContextError::new(
        "verify runtime mount ownership",
        Some(PathBuf::from(target)),
        detail.to_owned(),
    )))
}

fn same_identity(left: &OwnedMount, right: &OwnedMount) -> bool {
    left.id == right.id
        && left.target == right.target
        && left.device == right.device
        && left.fs_type == right.fs_type
        && left.source == right.source
}

fn capture_snapshot(
    targets: &[String],
    baseline: &[OwnedMount],
    current: &[OwnedMount],
) -> Result<Vec<OwnedMount>> {
    let mut captured = Vec::new();
    for entry in current
        .iter()
        .filter(|entry| targets.contains(&entry.target))
    {
        if let Some(previous) = baseline.iter().find(|old| old.id == entry.id) {
            if !same_identity(previous, entry) {
                return Err(failure(
                    &entry.target,
                    "baseline mount identity changed during capture",
                ));
            }
            continue;
        }
        let mut owned = entry.clone();
        owned.baseline_ids = baseline
            .iter()
            .filter(|old| old.target == entry.target)
            .map(|old| old.id)
            .collect();
        captured.push(owned);
    }
    Ok(captured)
}

/// Validates all recorded targets before any detach, including targets whose owned
/// mount has already disappeared. Baseline IDs distinguish retry from replacement.
fn preflight<'a>(owned: &'a [OwnedMount], current: &[OwnedMount]) -> Result<Vec<&'a OwnedMount>> {
    let mut by_id = BTreeMap::new();
    for entry in current {
        if by_id.insert(entry.id, entry).is_some() {
            return Err(failure(&entry.target, "duplicate live mount ID"));
        }
    }
    let mut owned_ids = BTreeSet::new();
    for expected in owned {
        if !Path::new(&expected.target).is_absolute()
            || expected.id <= 0
            || !owned_ids.insert(expected.id)
            || expected.baseline_ids.contains(&expected.id)
        {
            return Err(failure(&expected.target, "invalid saved mount identity"));
        }
        if let Some(actual) = by_id.get(&expected.id)
            && !same_identity(expected, actual)
        {
            return Err(failure(&expected.target, "owned mount identity changed"));
        }
    }
    for expected in owned {
        for actual in current
            .iter()
            .filter(|entry| entry.target == expected.target)
        {
            if !owned_ids.contains(&actual.id) && !expected.baseline_ids.contains(&actual.id) {
                return Err(failure(
                    &expected.target,
                    "foreign or replacement mount at owned target",
                ));
            }
        }
    }
    let mut active = Vec::new();
    for expected in owned.iter().filter(|entry| by_id.contains_key(&entry.id)) {
        for actual in current {
            if actual.target != expected.target
                && Path::new(&actual.target).starts_with(&expected.target)
                && !owned_ids.contains(&actual.id)
            {
                return Err(failure(
                    &actual.target,
                    "foreign descendant blocks owned mount cleanup",
                ));
            }
        }
        active.push(expected);
    }
    active.sort_by(|left, right| {
        Path::new(&right.target)
            .components()
            .count()
            .cmp(&Path::new(&left.target).components().count())
            .then_with(|| right.target.cmp(&left.target))
    });
    Ok(active)
}

fn verify_visible(
    active: &[&OwnedMount],
    visible_id: &mut impl FnMut(&Path) -> Result<i32>,
) -> Result<()> {
    let targets: BTreeSet<&str> = active.iter().map(|entry| entry.target.as_str()).collect();
    for target in targets {
        let visible = visible_id(Path::new(target))?;
        if !active
            .iter()
            .any(|entry| entry.target == target && entry.id == visible)
        {
            return Err(failure(target, "visible mount is not an owned mount"));
        }
    }
    Ok(())
}

fn cleanup_with(
    owned: &[OwnedMount],
    mut list: impl FnMut() -> Result<Vec<OwnedMount>>,
    mut visible_id: impl FnMut(&Path) -> Result<i32>,
    mut detach: impl FnMut(&Path) -> Result<()>,
) -> Result<()> {
    let mut current = list()?;
    loop {
        let active = preflight(owned, &current)?;
        verify_visible(&active, &mut visible_id)?;
        let Some(next) = active.first() else {
            return Ok(());
        };
        let target = Path::new(&next.target);
        // Mountinfo order and numeric IDs do not identify the top of a stack.
        let visible = visible_id(target)?;
        if !active
            .iter()
            .any(|entry| entry.target == next.target && entry.id == visible)
        {
            return Err(failure(&next.target, "visible mount changed before detach"));
        }
        detach(target)?;
        current = list()?;
        if current.iter().any(|entry| entry.id == visible) {
            return Err(failure(
                &next.target,
                "detached mount is still present on readback",
            ));
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn identities(entries: &[crate::sys::mountinfo::MountEntry]) -> Result<Vec<OwnedMount>> {
    entries
        .iter()
        .map(|entry| {
            Ok(OwnedMount {
                target: entry
                    .mount_point
                    .to_str()
                    .ok_or_else(|| failure("", "mount target is not UTF-8"))?
                    .to_owned(),
                id: entry.mnt_id,
                device: entry.majmin.clone(),
                fs_type: entry.fs_type.clone(),
                source: entry.mount_source.clone(),
                baseline_ids: Vec::new(),
            })
        })
        .collect()
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn snapshot() -> Result<Vec<OwnedMount>> {
    identities(&crate::sys::mountinfo::mount_entries()?)
}

/// Captures only new mount IDs at exact committed targets; source labels do not
/// establish ownership. Call while the runtime operation lock is held.
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn capture(
    targets: &[String],
    baseline: &[crate::sys::mountinfo::MountEntry],
) -> Result<Vec<OwnedMount>> {
    capture_snapshot(targets, &identities(baseline)?, &snapshot()?)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn visible_mount_id(path: &Path) -> Result<i32> {
    use rustix::fs::{Mode, OFlags, open};
    use std::os::fd::AsRawFd;
    let fd = open(path, OFlags::PATH | OFlags::CLOEXEC, Mode::empty()).map_err(|source| {
        Error::Mount(Box::new(ContextError::new(
            "open visible mount",
            Some(path.to_owned()),
            source,
        )))
    })?;
    // fdinfo mnt_id predates STATX_MNT_ID, so this also works on older Android kernels.
    let fdinfo = PathBuf::from(format!("/proc/self/fdinfo/{}", fd.as_raw_fd()));
    let text = std::fs::read_to_string(&fdinfo).map_err(|source| {
        Error::IoContext(Box::new(crate::errors::IoError::new(
            "read visible mount ID",
            Some(fdinfo),
            source,
        )))
    })?;
    text.lines()
        .find_map(|line| line.strip_prefix("mnt_id:"))
        .and_then(|value| value.trim().parse::<i32>().ok())
        .filter(|id| *id > 0)
        .ok_or_else(|| failure(&path.to_string_lossy(), "visible mount ID unavailable"))
}

/// Preflights the complete mount set before callers mutate other resource types.
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn validate(owned: &[OwnedMount]) -> Result<()> {
    let current = snapshot()?;
    let active = preflight(owned, &current)?;
    verify_visible(&active, &mut visible_mount_id)
}

/// Detaches owned mounts, including overlayfs, deepest first and verifies each
/// disappearance. Already removed IDs are safe to retry only with known baselines.
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn cleanup(owned: &[OwnedMount]) -> Result<()> {
    cleanup_with(owned, snapshot, visible_mount_id, |path| {
        rustix::mount::unmount(path, rustix::mount::UnmountFlags::DETACH).map_err(|source| {
            Error::Mount(Box::new(ContextError::new(
                "detach owned runtime mount",
                Some(path.to_owned()),
                source,
            )))
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::path::PathBuf;

    fn mount(id: i32, target: &str) -> OwnedMount {
        OwnedMount {
            target: target.into(),
            id,
            device: "0:42".into(),
            fs_type: "overlay".into(),
            source: Some("overlay".into()),
            baseline_ids: vec![],
        }
    }

    #[derive(Default)]
    struct Mounts {
        live: RefCell<Vec<OwnedMount>>,
        detached: RefCell<Vec<i32>>,
    }

    impl Mounts {
        fn new(live: Vec<OwnedMount>) -> Self {
            Self {
                live: RefCell::new(live),
                ..Self::default()
            }
        }
        fn list(&self) -> Result<Vec<OwnedMount>> {
            Ok(self.live.borrow().clone())
        }
        fn visible(&self, path: &Path) -> Result<i32> {
            self.live
                .borrow()
                .iter()
                .rev()
                .find(|m| Path::new(&m.target) == path)
                .map(|m| m.id)
                .ok_or_else(|| Error::msg("missing visible mount"))
        }
        fn detach(&self, path: &Path) -> Result<()> {
            let id = self.visible(path)?;
            self.live.borrow_mut().retain(|m| m.id != id);
            self.detached.borrow_mut().push(id);
            Ok(())
        }
        fn cleanup(&self, owned: &[OwnedMount]) -> Result<()> {
            cleanup_with(
                owned,
                || self.list(),
                |path| self.visible(path),
                |path| self.detach(path),
            )
        }
    }

    #[test]
    fn capture_records_only_new_ids_at_exact_targets_including_overlay() {
        let old = mount(1, "/system");
        let live = vec![
            old.clone(),
            mount(2, "/system"),
            mount(3, "/system/etc"),
            mount(4, "/vendor"),
        ];
        let captured = capture_snapshot(&["/system".into()], &[old], &live).unwrap();
        let mut expected = mount(2, "/system");
        expected.baseline_ids = vec![1];
        assert_eq!(captured, vec![expected]);
    }

    #[test]
    fn capture_refuses_reused_baseline_identity() {
        let old = mount(1, "/system");
        let mut changed = old.clone();
        changed.device = "1:2".into();
        assert!(capture_snapshot(&["/system".into()], &[old], &[changed]).is_err());
    }

    #[test]
    fn cleanup_detaches_overlay_children_before_parents() {
        let owned = vec![mount(1, "/system"), mount(2, "/system/etc")];
        let fake = Mounts::new(owned.clone());
        fake.cleanup(&owned).unwrap();
        assert_eq!(*fake.detached.borrow(), vec![2, 1]);
    }

    #[test]
    fn cleanup_refuses_changed_identity_before_any_detach() {
        for changed in [
            OwnedMount {
                id: 99,
                ..mount(1, "/system")
            },
            OwnedMount {
                device: "1:9".into(),
                ..mount(1, "/system")
            },
            OwnedMount {
                fs_type: "tmpfs".into(),
                ..mount(1, "/system")
            },
            OwnedMount {
                source: Some("foreign".into()),
                ..mount(1, "/system")
            },
            OwnedMount {
                target: "/vendor".into(),
                ..mount(1, "/system")
            },
        ] {
            let fake = Mounts::new(vec![changed]);
            assert!(fake.cleanup(&[mount(1, "/system")]).is_err());
            assert!(fake.detached.borrow().is_empty());
        }
    }

    #[test]
    fn cleanup_refuses_foreign_stacked_mounts() {
        let owned = mount(1, "/system");
        let fake = Mounts::new(vec![owned.clone(), mount(2, "/system")]);
        assert!(fake.cleanup(&[owned]).is_err());
        assert!(fake.detached.borrow().is_empty());
    }

    #[test]
    fn cleanup_preflights_all_roots_before_detaching_anything() {
        let owned = vec![mount(1, "/system"), mount(2, "/vendor/etc")];
        let fake = Mounts::new(vec![
            owned[0].clone(),
            owned[1].clone(),
            mount(3, "/system/foreign"),
        ]);
        assert!(fake.cleanup(&owned).is_err());
        assert!(fake.detached.borrow().is_empty());
    }

    #[test]
    fn cleanup_does_not_treat_sibling_prefix_as_descendant() {
        let owned = mount(1, "/system");
        let fake = Mounts::new(vec![owned.clone(), mount(2, "/system_ext")]);
        fake.cleanup(&[owned]).unwrap();
        assert_eq!(*fake.live.borrow(), vec![mount(2, "/system_ext")]);
    }

    #[test]
    fn cleanup_drains_owned_stack_but_preserves_baseline_mount() {
        let old = mount(1, "/system");
        let first = OwnedMount {
            baseline_ids: vec![1],
            ..mount(2, "/system")
        };
        let top = OwnedMount {
            baseline_ids: vec![1],
            ..mount(3, "/system")
        };
        let fake = Mounts::new(vec![old.clone(), first.clone(), top.clone()]);
        fake.cleanup(&[first, top]).unwrap();
        assert_eq!(*fake.live.borrow(), vec![old]);
        assert_eq!(*fake.detached.borrow(), vec![3, 2]);
    }

    #[test]
    fn cleanup_retry_tolerates_absent_owned_ids_and_exposed_baseline() {
        let owned = OwnedMount {
            baseline_ids: vec![1],
            ..mount(2, "/system")
        };
        let fake = Mounts::new(vec![mount(1, "/system")]);
        fake.cleanup(&[owned.clone(), mount(3, "/vendor")]).unwrap();
        fake.cleanup(&[owned]).unwrap();
        assert!(fake.detached.borrow().is_empty());
    }

    #[test]
    fn cleanup_never_detaches_baseline_covering_an_owned_mount() {
        let owned = OwnedMount {
            baseline_ids: vec![1],
            ..mount(2, "/system")
        };
        let fake = Mounts::new(vec![owned.clone(), mount(1, "/system")]);
        assert!(fake.cleanup(&[owned]).is_err());
        assert!(fake.detached.borrow().is_empty());
    }

    #[test]
    fn cleanup_rejects_success_without_mount_disappearance() {
        let owned = mount(1, "/system");
        let fake = Mounts::new(vec![owned.clone()]);
        assert!(cleanup_with(&[owned], || fake.list(), |p| fake.visible(p), |_| Ok(())).is_err());
    }

    #[test]
    fn cleanup_preserves_detach_error_and_allows_partial_retry() {
        let owned = vec![mount(1, "/system"), mount(2, "/system/etc")];
        let fake = Mounts::new(owned.clone());
        let error = cleanup_with(
            &owned,
            || fake.list(),
            |p| fake.visible(p),
            |p| {
                if p == Path::new("/system") {
                    return Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied).into());
                }
                fake.detach(p)
            },
        )
        .unwrap_err();
        assert!(
            matches!(error, Error::Io(ref err) if err.kind() == std::io::ErrorKind::PermissionDenied)
        );
        fake.cleanup(&owned).unwrap();
        assert_eq!(*fake.detached.borrow(), vec![2, 1]);
    }

    #[test]
    fn cleanup_preserves_list_error_without_detach() {
        let detached = RefCell::new(Vec::<PathBuf>::new());
        let error = cleanup_with(
            &[mount(1, "/system")],
            || Err(Error::msg("read failure")),
            |_| Ok(1),
            |p| {
                detached.borrow_mut().push(p.into());
                Ok(())
            },
        )
        .unwrap_err();
        assert!(error.to_string().contains("read failure"));
        assert!(detached.borrow().is_empty());
    }

    #[test]
    fn cleanup_stops_when_readback_discovers_a_new_foreign_descendant() {
        let owned = vec![mount(1, "/system"), mount(2, "/system/etc")];
        let fake = Mounts::new(owned.clone());
        let result = cleanup_with(
            &owned,
            || fake.list(),
            |path| fake.visible(path),
            |path| {
                fake.detach(path)?;
                fake.live.borrow_mut().push(mount(3, "/system/foreign"));
                Ok(())
            },
        );
        assert!(result.is_err());
        assert_eq!(*fake.detached.borrow(), vec![2]);
    }

    #[test]
    fn cleanup_refuses_unreadable_visible_identity_without_detach() {
        let owned = mount(1, "/system");
        let fake = Mounts::new(vec![owned.clone()]);
        let error = cleanup_with(
            &[owned],
            || fake.list(),
            |_| Err(Error::msg("fdinfo unavailable")),
            |path| fake.detach(path),
        )
        .unwrap_err();
        assert!(error.to_string().contains("fdinfo unavailable"));
        assert!(fake.detached.borrow().is_empty());
    }
}
