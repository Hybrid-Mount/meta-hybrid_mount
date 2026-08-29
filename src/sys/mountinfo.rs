// SPDX-License-Identifier: GPL-3.0-only

//! Mountinfo snapshot shared by mount confirmation and rollback queries.

use std::path::{Path, PathBuf};

#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::errors::{Error, Result};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MountSnapshot {
    points: Vec<PathBuf>,
}

impl MountSnapshot {
    pub fn from_paths(points: Vec<PathBuf>) -> Self {
        let mut points = points;
        points.sort();
        points.dedup();
        Self { points }
    }

    #[cfg(test)]
    pub fn points(&self) -> &[PathBuf] {
        &self.points
    }

    pub fn contains(&self, path: &Path) -> bool {
        self.points.iter().any(|point| point == path)
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub fn read() -> Result<Self> {
        let process = procfs::process::Process::myself()
            .map_err(|err| Error::msg(format!("get self process for mountinfo: {err}")))?;
        let mountinfo = process
            .mountinfo()
            .map_err(|err| Error::msg(format!("read mountinfo: {err}")))?;
        Ok(Self::from_paths(
            mountinfo
                .into_iter()
                .map(|entry| entry.mount_point)
                .collect(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::MountSnapshot;
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

    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[test]
    fn read_snapshot_contains_root_mount() {
        let snapshot = MountSnapshot::read().unwrap();
        assert!(snapshot.contains(PathBuf::from("/").as_path()));
    }
}
