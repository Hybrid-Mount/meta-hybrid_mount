// SPDX-License-Identifier: GPL-3.0-only

//! Builders shared by the VFS unit tests.

use crate::config::Mode;
use crate::module_id::ModuleId;
use crate::mount_tree::{MountSource, NodeFileType};
use std::path::PathBuf;

/// A module source entry whose in-module path is also its target path.
pub(crate) fn source(
    module: &str,
    relative: &str,
    file_type: NodeFileType,
    backend: Mode,
) -> MountSource {
    MountSource {
        module_id: ModuleId::try_from(module).unwrap(),
        relative: relative.to_owned(),
        source_path: PathBuf::from(format!("/data/adb/modules/{module}/{relative}")),
        file_type,
        replace: false,
        backend,
    }
}
