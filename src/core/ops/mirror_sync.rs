// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::path::Path;

use anyhow::{Context, Result};

use crate::{
    core::{failure::ModuleStageFailure, inventory::Module},
    partitions,
    sys::fs::{PreparedDir, finalize_copied_tree, prune_orphaned_children, sync_dir},
};

pub fn sync_modules(modules: &[Module], target_base: &Path) -> Result<()> {
    crate::scoped_log!(
        info,
        "mirror_sync",
        "start: target={}",
        target_base.display()
    );
    let managed_partitions = partitions::managed_partition_names();

    prune_orphaned_children(
        target_base,
        modules.iter().map(|module| module.id.as_str()),
        &["lost+found", "hybrid_mount"],
        "mirror_sync",
    )?;

    for module in modules {
        if !has_managed_mount_root(module, &managed_partitions) {
            crate::scoped_log!(
                debug,
                "mirror_sync",
                "module skip: id={}, reason=no_managed_partition_root",
                module.id
            );
            continue;
        }

        crate::scoped_log!(info, "mirror_sync", "module start: id={}", module.id);

        let prepared = PreparedDir::new(target_base, &module.id)
            .map_err(|err| module_sync_error(module, err))
            .with_context(|| format!("Failed to initialize sync staging for {}", module.id))?;

        let sync_stats = sync_dir(
            &module.source_path,
            prepared.tmp_path(),
            &managed_partitions,
        )
        .map_err(|err| {
            crate::scoped_log!(
                error,
                "mirror_sync",
                "module sync failed: id={}, error={}",
                module.id,
                err
            );
            module_sync_error(module, err)
        })
        .with_context(|| format!("Failed to sync module {}", module.id))?;

        if !sync_stats.has_mount_content {
            crate::scoped_log!(
                debug,
                "mirror_sync",
                "module skip: id={}, reason=no_mount_content_after_sync",
                module.id
            );
            continue;
        }

        finalize_copied_tree(&module.id, prepared.tmp_path(), &sync_stats.opaque_dirs)?;
        prepared
            .commit()
            .map_err(|err| {
                crate::scoped_log!(
                    error,
                    "mirror_sync",
                    "commit prepared module failed: id={}, error={}",
                    module.id,
                    err
                );
                module_sync_error(module, err)
            })
            .with_context(|| format!("Failed to commit synced module {}", module.id))?;
    }

    Ok(())
}

fn module_sync_error(module: &Module, err: anyhow::Error) -> anyhow::Error {
    ModuleStageFailure::sync_one(&module.id, err).into()
}

fn has_managed_mount_root(module: &Module, managed_partitions: &[String]) -> bool {
    managed_partitions
        .iter()
        .any(|partition| module.source_path.join(partition).is_dir())
}
