// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{collections::HashSet, path::Path};

use anyhow::Result;

use crate::{conf::config, core::runtime_state::MountStatistics, mount::magic_mount};

pub(super) fn mount_magic(
    ids: &[String],
    config: &config::Config,
    tempdir: &Path,
) -> Result<(Vec<String>, MountStatistics)> {
    let magic_ws_path = tempdir.join("magic_workspace");

    crate::scoped_log!(
        debug,
        "executor:magic",
        "prepare workspace: path={}",
        magic_ws_path.display()
    );

    if !magic_ws_path.exists() {
        std::fs::create_dir_all(&magic_ws_path)?;
    }

    let module_ids: HashSet<String> = ids.iter().cloned().collect();

    let stats = magic_mount::magic_mount(
        &magic_ws_path,
        tempdir,
        &config.mountsource,
        &config.partitions,
        module_ids,
        !config.disable_umount,
    )?;

    crate::scoped_log!(
        debug,
        "executor:magic",
        "complete: module_count={}",
        ids.len()
    );

    Ok((ids.to_vec(), stats))
}
