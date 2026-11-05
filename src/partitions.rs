// SPDX-License-Identifier: GPL-3.0-only

//! Read-only partition layout: module names are logical, targets follow the
//! visible mount namespace. No backend may invent a second mapping.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::errors::{Error, Result};
use crate::sys::mountinfo::MountEntry;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathEvidence {
    pub canonical: PathBuf,
    pub dev: u64,
    pub ino: u64,
    pub visible_mount_id: i32,
}

#[derive(Debug, Clone, Default)]
pub struct LayoutSnapshot {
    pub mounts: Vec<MountEntry>,
    pub paths: BTreeMap<PathBuf, PathEvidence>,
    /// Missing paths are absent; failed probes are never treated as missing.
    pub path_errors: BTreeMap<PathBuf, String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PartitionDiagnostic {
    pub partition: String,
    pub code: String,
    pub reason: String,
    pub modules: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct OperationDiagnostic {
    pub module_id: String,
    pub logical_path: String,
    pub target: Option<PathBuf>,
    pub code: String,
    pub reason: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PartitionIdentity {
    pub canonical: PathBuf,
    pub dev: u64,
    pub ino: u64,
    pub subtree: BTreeMap<PathBuf, (u64, u64, String, PathBuf)>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PartitionLayout {
    pub roots: BTreeMap<String, PathBuf>,
    pub identities: BTreeMap<String, PartitionIdentity>,
    pub errors: BTreeMap<String, PartitionDiagnostic>,
    /// Visible submount boundaries, not stable cross-operation identities.
    pub mount_points: BTreeSet<PathBuf>,
}

impl PartitionLayout {
    #[cfg(test)]
    pub fn from_promoted(names: &BTreeSet<String>) -> Self {
        let mut layout = Self::default();
        layout.roots.insert("system".into(), "/system".into());
        for name in names {
            layout
                .roots
                .insert(name.clone(), PathBuf::from("/").join(name));
        }
        layout
    }
    pub fn logical_partition<'a>(&self, relative: &'a str) -> &'a str {
        let mut parts = relative.split('/');
        let first = parts.next().unwrap_or("system");
        if first == "system"
            && let Some(name) = parts.next()
            && name != "system"
            && (self.roots.contains_key(name) || self.errors.contains_key(name))
        {
            return name;
        }
        first
    }

    pub fn root_len(&self, relative: &str) -> usize {
        usize::from(relative.starts_with("system/") && self.logical_partition(relative) != "system")
            + 1
    }

    pub fn map_target(&self, relative: &str) -> Result<(String, PathBuf)> {
        if relative.is_empty()
            || relative
                .split('/')
                .any(|c| c.is_empty() || matches!(c, "." | "..") || c.contains(['\\', '\0', ':']))
            || Path::new(relative)
                .components()
                .any(|c| !matches!(c, Component::Normal(_)))
        {
            return Err(Error::msg(format!("invalid module path: {relative:?}")));
        }
        let partition = self.logical_partition(relative);
        if let Some(error) = self.errors.get(partition) {
            return Err(Error::msg(format!(
                "partition {partition} skipped: {}",
                error.reason
            )));
        }
        let fallback = PathBuf::from("/").join(partition);
        let root = self.roots.get(partition).unwrap_or(&fallback);
        let suffix = relative
            .split('/')
            .skip(self.root_len(relative))
            .collect::<Vec<_>>()
            .join("/");
        Ok((
            partition.to_owned(),
            if suffix.is_empty() {
                root.clone()
            } else {
                root.join(suffix)
            },
        ))
    }

    pub fn scan_names(&self) -> Vec<String> {
        self.roots
            .keys()
            .chain(self.errors.keys())
            .filter(|n| n.as_str() != "system")
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }
}

fn visible_mount<'a>(snapshot: &'a LayoutSnapshot, path: &PathEvidence) -> Option<&'a MountEntry> {
    snapshot
        .mounts
        .iter()
        .find(|m| m.mnt_id == path.visible_mount_id && path.canonical.starts_with(&m.mount_point))
}

fn identity(snapshot: &LayoutSnapshot, path: &PathEvidence) -> Result<PartitionIdentity> {
    let mut subtree = BTreeMap::new();
    for mount in &snapshot.mounts {
        if !mount.mount_point.starts_with(&path.canonical) || mount.mount_point == path.canonical {
            continue;
        }
        if let Some(error) = snapshot.path_errors.get(&mount.mount_point) {
            return Err(Error::msg(format!(
                "cannot inspect child mount {}: {error}",
                mount.mount_point.display()
            )));
        }
        let probe = snapshot.paths.get(&mount.mount_point).ok_or_else(|| {
            Error::msg(format!(
                "missing child mount identity: {}",
                mount.mount_point.display()
            ))
        })?;
        if probe.visible_mount_id != mount.mnt_id {
            continue;
        }
        if let Ok(relative) = mount.mount_point.strip_prefix(&path.canonical) {
            subtree.insert(
                relative.to_owned(),
                (
                    probe.dev,
                    probe.ino,
                    mount.majmin.clone(),
                    mount.root.clone(),
                ),
            );
        }
    }
    Ok(PartitionIdentity {
        canonical: path.canonical.clone(),
        dev: path.dev,
        ino: path.ino,
        subtree,
    })
}

fn choose_root(snapshot: &LayoutSnapshot, name: &str, system: &Path) -> Result<PathEvidence> {
    let top = PathBuf::from("/").join(name);
    let nested = PathBuf::from("/system").join(name);
    for candidate in [&top, &nested] {
        if let Some(error) = snapshot.path_errors.get(candidate) {
            return Err(Error::msg(format!("{}: {error}", candidate.display())));
        }
    }
    let a = snapshot.paths.get(&top);
    let b = snapshot.paths.get(&nested);
    for probe in [a, b].into_iter().flatten() {
        if probe.canonical != top && probe.canonical != system.join(name) {
            return Err(Error::msg(format!(
                "partition alias resolves outside allowed roots: {}",
                probe.canonical.display()
            )));
        }
        if visible_mount(snapshot, probe).is_none() {
            return Err(Error::msg("visible mount identity is not in the snapshot"));
        }
    }
    match (a, b) {
        (Some(a), Some(b)) => {
            if (a.dev, a.ino) != (b.dev, b.ino) {
                return Err(Error::msg(
                    "candidate paths expose different directory trees",
                ));
            }
            if a.canonical == b.canonical {
                return Ok(a.clone());
            }
            let am = visible_mount(snapshot, a).ok_or_else(|| Error::msg("missing root mount"))?;
            let bm = visible_mount(snapshot, b).ok_or_else(|| Error::msg("missing root mount"))?;
            let ar = am.root.join(
                a.canonical
                    .strip_prefix(&am.mount_point)
                    .map_err(|e| Error::msg(e.to_string()))?,
            );
            let br = bm.root.join(
                b.canonical
                    .strip_prefix(&bm.mount_point)
                    .map_err(|e| Error::msg(e.to_string()))?,
            );
            if am.majmin != bm.majmin
                || ar != br
                || identity(snapshot, a)?.subtree != identity(snapshot, b)?.subtree
            {
                return Err(Error::msg(
                    "bind aliases have different filesystem roots or child mount views",
                ));
            }
            Ok(if am.mount_point == a.canonical { a } else { b }.clone())
        }
        (None, Some(b)) => Ok(b.clone()),
        (Some(a), None)
            if visible_mount(snapshot, a).is_some_and(|m| m.mount_point == a.canonical) =>
        {
            Ok(a.clone())
        }
        (Some(_), None) => Err(Error::msg(
            "top-level directory has no exact visible partition mount",
        )),
        (None, None) => Err(Error::msg("neither partition root exists")),
    }
}

pub fn discover_layout(
    snapshot: &LayoutSnapshot,
    managed: &[String],
    extras: &[String],
) -> PartitionLayout {
    let mut layout = PartitionLayout::default();
    let system = snapshot
        .paths
        .get(Path::new("/system"))
        .map_or_else(|| PathBuf::from("/system"), |p| p.canonical.clone());
    layout.roots.insert("system".into(), system.clone());
    if let Some(probe) = snapshot.paths.get(Path::new("/system")) {
        match identity(snapshot, probe) {
            Ok(id) => {
                layout.identities.insert("system".into(), id);
            }
            Err(e) => {
                layout.errors.insert(
                    "system".into(),
                    PartitionDiagnostic {
                        partition: "system".into(),
                        code: "layout_unavailable".into(),
                        reason: e.to_string(),
                        modules: vec![],
                    },
                );
            }
        }
    }
    for name in managed {
        let chosen = choose_root(snapshot, name, &system).and_then(|p| identity(snapshot, &p));
        match chosen {
            Ok(id) => {
                layout.roots.insert(name.clone(), id.canonical.clone());
                layout.identities.insert(name.clone(), id);
            }
            Err(e) => {
                layout.errors.insert(
                    name.clone(),
                    PartitionDiagnostic {
                        partition: name.clone(),
                        code: "layout_unavailable".into(),
                        reason: e.to_string(),
                        modules: vec![],
                    },
                );
            }
        }
    }
    for name in extras {
        if let Some(probe) = snapshot.paths.get(&PathBuf::from("/").join(name)) {
            match identity(snapshot, probe) {
                Ok(id) => {
                    layout.roots.insert(name.clone(), id.canonical.clone());
                    layout.identities.insert(name.clone(), id);
                }
                Err(e) => {
                    layout.errors.insert(
                        name.clone(),
                        PartitionDiagnostic {
                            partition: name.clone(),
                            code: "layout_unavailable".into(),
                            reason: e.to_string(),
                            modules: vec![],
                        },
                    );
                }
            }
        }
    }
    layout.mount_points = snapshot
        .mounts
        .iter()
        .filter(|m| {
            snapshot
                .paths
                .get(&m.mount_point)
                .is_some_and(|p| p.visible_mount_id == m.mnt_id)
        })
        .map(|m| m.mount_point.clone())
        .collect();
    layout
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn probe_path(path: &Path) -> std::io::Result<PathEvidence> {
    use std::os::fd::AsRawFd;
    let canonical = std::fs::canonicalize(path)?;
    let fd = rustix::fs::open(
        &canonical,
        rustix::fs::OFlags::PATH | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )?;
    let stat = rustix::fs::fstat(&fd)?;
    let contents = std::fs::read_to_string(format!("/proc/self/fdinfo/{}", fd.as_raw_fd()))?;
    let visible_mount_id = contents
        .lines()
        .find_map(|line| {
            line.strip_prefix("mnt_id:")
                .and_then(|n| n.trim().parse().ok())
        })
        .ok_or_else(|| std::io::Error::other("fdinfo did not report mount identity"))?;
    Ok(PathEvidence {
        canonical,
        dev: stat.st_dev,
        ino: stat.st_ino,
        visible_mount_id,
    })
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn capture_layout_snapshot(names: &[String], mounts: &[MountEntry]) -> Result<LayoutSnapshot> {
    let mut snapshot = LayoutSnapshot {
        mounts: mounts.to_vec(),
        ..LayoutSnapshot::default()
    };
    let mut candidates = BTreeSet::from([PathBuf::from("/"), PathBuf::from("/system")]);
    for name in names {
        candidates.insert(PathBuf::from("/").join(name));
        candidates.insert(PathBuf::from("/system").join(name));
    }
    for path in &candidates {
        match probe_path(path) {
            Ok(probe) => {
                snapshot.paths.insert(path.clone(), probe);
            }
            Err(err)
                if err.kind() == std::io::ErrorKind::NotFound
                    && !std::fs::symlink_metadata(path)
                        .is_ok_and(|m| m.file_type().is_symlink()) => {}
            Err(err) => {
                snapshot.path_errors.insert(path.clone(), err.to_string());
            }
        }
    }
    let roots = snapshot
        .paths
        .values()
        .map(|p| p.canonical.clone())
        .filter(|p| p != Path::new("/"))
        .collect::<Vec<_>>();
    for mount in mounts
        .iter()
        .filter(|m| roots.iter().any(|root| m.mount_point.starts_with(root)))
    {
        if snapshot.paths.contains_key(&mount.mount_point) {
            continue;
        }
        match probe_path(&mount.mount_point) {
            Ok(probe) => {
                snapshot.paths.insert(mount.mount_point.clone(), probe);
            }
            Err(err) => {
                snapshot
                    .path_errors
                    .insert(mount.mount_point.clone(), err.to_string());
            }
        }
    }
    Ok(snapshot)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn detect(config: &crate::config::Config) -> Result<PartitionLayout> {
    let managed = crate::defs::MANAGED_PARTITIONS
        .iter()
        .map(|n| (*n).to_owned())
        .collect::<Vec<_>>();
    let extras = config.extra_mount_partitions(Path::new("/"));
    let names = managed.iter().chain(&extras).cloned().collect::<Vec<_>>();
    let mounts = crate::sys::mountinfo::mount_entries()?;
    let snapshot = capture_layout_snapshot(&names, &mounts)?;
    Ok(discover_layout(&snapshot, &managed, &extras))
}

#[cfg(test)]
mod tests;
