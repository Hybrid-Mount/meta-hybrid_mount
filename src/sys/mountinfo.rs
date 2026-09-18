// SPDX-License-Identifier: GPL-3.0-only

//! Mountinfo snapshot shared by mount confirmation and rollback queries.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::errors::{Error, Result};

/// One mount as `/proc/self/mountinfo` reports it, reduced to the fields Hybrid Mount
/// reads. Every mountinfo consumer goes through [`mount_entries`] so the fault-injection
/// switch reaches all of them, not just the snapshot path.
#[cfg(any(target_os = "linux", target_os = "android"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountEntry {
    pub mount_point: PathBuf,
    pub fs_type: String,
    pub mount_source: Option<String>,
    pub mnt_id: i32,
    /// Device major:minor, as reported by the kernel.
    pub majmin: String,
}

/// Reads every mount visible to this process.
///
/// The cause is preserved as a `CausalError::Procfs` (rather than flattened into a
/// formatted string) so `Error::classify()` keeps reporting the real error class to
/// callers that decide between retry and manual recovery.
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn mount_entries() -> Result<Vec<MountEntry>> {
    if crate::sys::faults::should_fail_mountinfo_read() {
        return Err(Error::msg("injected mountinfo read failure"));
    }
    Ok(read_mountinfo()?
        .into_iter()
        .map(|entry| MountEntry {
            mount_point: entry.mount_point,
            fs_type: entry.fs_type,
            mount_source: entry.mount_source,
            mnt_id: entry.mnt_id,
            majmin: entry.majmin,
        })
        .collect())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn read_mountinfo() -> Result<Vec<procfs::process::MountInfo>> {
    let process = procfs::process::Process::myself().map_err(|err| {
        Error::Mount(Box::new(crate::errors::ContextError::new(
            "get self process for mountinfo",
            None,
            err,
        )))
    })?;
    let mountinfo = process.mountinfo().map_err(|err| {
        Error::Mount(Box::new(crate::errors::ContextError::new(
            "read mountinfo",
            None,
            err,
        )))
    })?;
    Ok(mountinfo.0)
}

/// The mount whose mount point is exactly `path`, when one exists.
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn mount_entry_at(path: &Path) -> Result<Option<MountEntry>> {
    Ok(mount_entries()?
        .into_iter()
        .find(|entry| entry.mount_point == path))
}

/// Deepest-first, then reverse-lexicographic. Unmounting children before their parents is
/// what makes a detach cascade safe, so this lives here rather than being respelled.
pub fn deepest_first(paths: &mut [PathBuf]) {
    paths.sort_by(|left, right| deepest_first_order(left, right));
}

/// The ordering relation behind [`deepest_first`], for callers holding borrowed paths.
fn deepest_first_order(left: &Path, right: &Path) -> Ordering {
    right
        .components()
        .count()
        .cmp(&left.components().count())
        .then_with(|| right.cmp(left))
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MountSnapshot {
    points: Vec<PathBuf>,
    ids: BTreeMap<PathBuf, BTreeSet<i32>>,
}

impl MountSnapshot {
    pub fn from_records(records: Vec<(PathBuf, i32)>) -> Self {
        let mut ids: BTreeMap<PathBuf, BTreeSet<i32>> = BTreeMap::new();
        for (path, mnt_id) in records {
            ids.entry(path).or_default().insert(mnt_id);
        }
        let mut points = ids.keys().cloned().collect::<Vec<_>>();
        points.sort();
        Self { points, ids }
    }

    #[cfg(test)]
    pub fn from_paths(points: Vec<PathBuf>) -> Self {
        Self::from_records(points.into_iter().map(|point| (point, 0)).collect())
    }

    #[cfg(test)]
    pub fn points(&self) -> &[PathBuf] {
        &self.points
    }

    pub fn contains(&self, path: &Path) -> bool {
        self.points.iter().any(|point| point == path)
    }

    pub fn descendants(&self, root: &Path) -> Vec<&Path> {
        let mut descendants = self
            .points
            .iter()
            .map(PathBuf::as_path)
            .filter(|point| point.starts_with(root) && *point != root)
            .collect::<Vec<_>>();
        descendants.sort_by(|left, right| deepest_first_order(left, right));
        descendants
    }

    pub fn subtree_ids(&self, root: &Path) -> BTreeMap<PathBuf, BTreeSet<i32>> {
        self.ids
            .iter()
            .filter(|(path, _)| path.as_path().starts_with(root))
            .map(|(path, ids)| (path.clone(), ids.clone()))
            .collect()
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub fn read() -> Result<Self> {
        Ok(Self::from_records(
            mount_entries()?
                .into_iter()
                .map(|entry| (entry.mount_point, entry.mnt_id))
                .collect(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::MountSnapshot;
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::PathBuf;

    #[test]
    fn from_paths_sorts_and_deduplicates() {
        let snapshot = MountSnapshot::from_paths(vec![
            PathBuf::from("/system/etc"),
            PathBuf::from("/system"),
            PathBuf::from("/system"),
        ]);

        assert_eq!(
            snapshot.points(),
            &[PathBuf::from("/system"), PathBuf::from("/system/etc")]
        );
    }

    #[test]
    fn contains_matches_exact_mount_points_only() {
        let snapshot =
            MountSnapshot::from_paths(vec![PathBuf::from("/system"), PathBuf::from("/system/etc")]);

        assert!(snapshot.contains(PathBuf::from("/system").as_path()));
        assert!(snapshot.contains(PathBuf::from("/system/etc").as_path()));
        assert!(!snapshot.contains(PathBuf::from("/system/etc/hosts").as_path()));
        assert!(!snapshot.contains(PathBuf::from("/product").as_path()));
    }

    #[test]
    fn descendants_are_deepest_first() {
        let snapshot = MountSnapshot::from_paths(vec![
            PathBuf::from("/system"),
            PathBuf::from("/system/etc/hosts"),
            PathBuf::from("/system/etc"),
            PathBuf::from("/system/bin"),
            PathBuf::from("/product"),
        ]);

        assert_eq!(
            snapshot.descendants(PathBuf::from("/system").as_path()),
            vec![
                PathBuf::from("/system/etc/hosts").as_path(),
                PathBuf::from("/system/etc").as_path(),
                PathBuf::from("/system/bin").as_path(),
            ]
        );
    }

    #[test]
    fn subtree_ids_preserve_stacked_mount_ids() {
        let snapshot = MountSnapshot::from_records(vec![
            (PathBuf::from("/system"), 10),
            (PathBuf::from("/system"), 20),
            (PathBuf::from("/system/etc"), 30),
            (PathBuf::from("/product"), 40),
        ]);

        assert_eq!(
            snapshot.subtree_ids(PathBuf::from("/system").as_path()),
            BTreeMap::from([
                (PathBuf::from("/system"), BTreeSet::from([10, 20])),
                (PathBuf::from("/system/etc"), BTreeSet::from([30])),
            ])
        );
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[test]
    fn read_snapshot_contains_root_mount() {
        let snapshot = MountSnapshot::read().unwrap();
        assert!(snapshot.contains(PathBuf::from("/").as_path()));
    }
}
