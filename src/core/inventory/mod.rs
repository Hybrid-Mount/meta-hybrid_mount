// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

pub mod model;
pub mod scanner;

pub use scanner::*;

pub use crate::conf::config::MountMode;
use crate::defs;

pub fn is_reserved_module_dir(id: &str) -> bool {
    matches!(
        id,
        "hybrid-mount" | "lost+found" | ".git" | ".idea" | ".vscode"
    )
}

pub fn has_mount_block_marker(module_path: &std::path::Path) -> bool {
    module_path.join(defs::DISABLE_FILE_NAME).exists()
        || module_path.join(defs::REMOVE_FILE_NAME).exists()
        || module_path.join(defs::SKIP_MOUNT_FILE_NAME).exists()
}
