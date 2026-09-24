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
    /// Peer group id from the `shared:` field, present only while the mount is shared.
    ///
    /// Hybrid Mount reads it back to confirm that its own targets left every peer group they
    /// could have inherited from the mount they were cloned from.
    pub shared: Option<u32>,
    /// Mount id from the `master:` field, present only while the mount is a slave.
    pub master: Option<u32>,
}

#[cfg(any(target_os = "linux", target_os = "android"))]
impl MountEntry {
    /// Keeps the mountinfo fields Hybrid Mount reads, including the propagation state that decides
    /// whether a target still belongs to a peer group.
    fn from_mount_info(info: &procfs::process::MountInfo) -> Self {
        let mut shared = None;
        let mut master = None;
        for field in &info.opt_fields {
            match field {
                procfs::process::MountOptFields::Shared(id) => shared = Some(*id),
                procfs::process::MountOptFields::Master(id) => master = Some(*id),
                _ => {}
            }
        }

        Self {
            mount_point: info.mount_point.clone(),
            fs_type: info.fs_type.clone(),
            mount_source: info.mount_source.clone(),
            mnt_id: info.mnt_id,
            majmin: info.majmin.clone(),
            shared,
            master,
        }
    }

    /// Human-readable propagation state, used by boot diagnostics.
    pub fn propagation(&self) -> String {
        match (self.shared, self.master) {
            (Some(group), _) => format!("shared:{group}"),
            (None, Some(master)) => format!("slave:{master}"),
            (None, None) => "private".to_owned(),
        }
    }
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
        .iter()
        .map(MountEntry::from_mount_info)
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
    #[cfg(any(target_os = "linux", target_os = "android"))]
    use super::MountEntry;
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

    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[test]
    fn mount_entry_reads_propagation_fields() {
        use procfs::process::MountInfo;

        let shared = MountEntry::from_mount_info(
            &MountInfo::from_line(
                "36 1 0:32 / /system rw,relatime shared:7 - ext4 /dev/block/sda1 rw",
            )
            .unwrap(),
        );
        assert_eq!(shared.mount_point, PathBuf::from("/system"));
        assert_eq!(shared.shared, Some(7));
        assert_eq!(shared.master, None);
        assert_eq!(shared.propagation(), "shared:7");

        // A slave carries `master:` without a peer group of its own.
        let slave = MountEntry::from_mount_info(
            &MountInfo::from_line("37 36 0:32 / /system/etc rw master:7 - ext4 /dev/block/sda1 rw")
                .unwrap(),
        );
        assert_eq!(slave.shared, None);
        assert_eq!(slave.master, Some(7));
        assert_eq!(slave.propagation(), "slave:7");

        // Ordinary mounts and unbindable ones carry no propagation id at all.
        for line in [
            "38 1 0:33 / /data rw,relatime - ext4 /dev/block/sda2 rw",
            "39 1 0:34 / /mnt rw,relatime unbindable - tmpfs tmpfs rw",
        ] {
            let entry = MountEntry::from_mount_info(&MountInfo::from_line(line).unwrap());
            assert_eq!(entry.shared, None);
            assert_eq!(entry.master, None);
            assert_eq!(entry.propagation(), "private");
        }
    }
}
