use std::collections::BTreeMap;
use std::path::PathBuf;

use super::{ModuleId, MountTree, OperationDiagnostic, PartitionDiagnostic, PartitionLayout};

/// One overlay mount operation, structured like v4.2.0's `OverlayOperation`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayOperation {
    pub partition: String,
    pub target: String,
    pub lowerdirs: Vec<PathBuf>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MountPlan {
    /// Shared tree for staging, Magic Mount and VFS; OverlayFS uses `overlay_ops`.
    pub tree: MountTree,
    /// Directory-level overlay: target mountpoint to ordered lowerdirs, sorted by module id.
    pub overlay_ops: Vec<OverlayOperation>,
    /// File-level overlay rules: parent directory target to file sources, which the executor turns into shallow layers.
    pub overlay_files: BTreeMap<String, Vec<PathBuf>>,
    pub overlay_module_ids: Vec<ModuleId>,
    pub magic_module_ids: Vec<ModuleId>,
    pub vfs_module_ids: Vec<ModuleId>,
    pub partition_errors: Vec<PartitionDiagnostic>,
    pub operation_errors: Vec<OperationDiagnostic>,
    pub partition_layout: PartitionLayout,
}

impl MountPlan {
    /// VFS module ids as strings, for the persisted state snapshot and the WebUI contract.
    pub fn vfs_module_id_strings(&self) -> Vec<String> {
        self.vfs_module_ids
            .iter()
            .map(ToString::to_string)
            .collect()
    }
}
