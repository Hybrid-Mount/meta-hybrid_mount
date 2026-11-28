use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::errors::{Error, Result};

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
