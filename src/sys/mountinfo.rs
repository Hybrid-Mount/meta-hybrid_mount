// SPDX-License-Identifier: GPL-3.0-only

//! Mountinfo snapshot shared by mount confirmation and rollback queries.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::errors::{Error, Result};

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
        descendants.sort_by(|left, right| {
            right
                .components()
                .count()
                .cmp(&left.components().count())
                .then_with(|| right.cmp(left))
        });
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
        if crate::sys::faults::should_fail_mountinfo_read() {
            return Err(Error::msg("injected mountinfo read failure"));
        }
        let process = procfs::process::Process::myself()
            .map_err(|err| Error::msg(format!("get self process for mountinfo: {err}")))?;
        let mountinfo = process
            .mountinfo()
            .map_err(|err| Error::msg(format!("read mountinfo: {err}")))?;
        Ok(Self::from_records(
            mountinfo
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
