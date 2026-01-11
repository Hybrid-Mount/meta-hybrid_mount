// Copyright 2025 Meta-Hybrid Mount Authors
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{collections::HashSet, fs, path::Path};

use anyhow::Result;
use rayon::prelude::*;
use walkdir::WalkDir;

use crate::{
    core::inventory::{Module, MountMode},
    defs, utils,
};

pub fn perform_sync(modules: &[Module], target_base: &Path) -> Result<()> {
    tracing::info!("Starting smart module sync to {}", target_base.display());

    prune_orphaned_modules(modules, target_base)?;

    modules.par_iter().for_each(|module| {
        if matches!(module.rules.default_mode, MountMode::Magic) {
            tracing::debug!("Skipping sync for Magic Mount module: {}", module.id);

            return;
        }

        let dst = target_base.join(&module.id);

        let has_content = defs::BUILTIN_PARTITIONS.iter().any(|p| {
            let part_path = module.source_path.join(p);

            part_path.exists() && has_files_recursive(&part_path)
        });

        if has_content && should_sync(&module.source_path, &dst) {
            tracing::info!("Syncing module: {} (Updated/New)", module.id);

            if dst.exists()
                && let Err(e) = fs::remove_dir_all(&dst)
            {
                tracing::warn!("Failed to clean target dir for {}: {}", module.id, e);
            }

            if let Err(e) = utils::sync_dir(&module.source_path, &dst, true) {
                tracing::error!("Failed to sync module {}: {}", module.id, e);
            } else {
                // Apply trusted.overlay.opaque xattrs if .replace files exist
                if let Err(e) = apply_overlay_opaque_flags(&dst) {
                    tracing::warn!(
                        "Failed to apply overlay opaque xattrs for {}: {}",
                        module.id,
                        e
                    );
                }
            }
        } else {
            tracing::debug!("Skipping module: {}", module.id);
        }
    });

    Ok(())
}

fn apply_overlay_opaque_flags(root: &Path) -> Result<()> {
    for entry in WalkDir::new(root).min_depth(1).into_iter().flatten() {
        #[allow(clippy::collapsible_if)]
        if entry.file_type().is_file() && entry.file_name() == defs::REPLACE_DIR_FILE_NAME {
            if let Some(parent) = entry.path().parent() {
                utils::set_overlay_opaque(parent)?;
                tracing::debug!("Set overlay opaque xattr on: {}", parent.display());
            }
        }
    }
    Ok(())
}

fn prune_orphaned_modules(modules: &[Module], target_base: &Path) -> Result<()> {
    if !target_base.exists() {
        return Ok(());
    }

    let active_ids: HashSet<&str> = modules.iter().map(|m| m.id.as_str()).collect();

    let entries: Vec<_> = fs::read_dir(target_base)?.filter_map(|e| e.ok()).collect();

    entries.par_iter().for_each(|entry| {
        let path = entry.path();

        let name_os = entry.file_name();

        let name = name_os.to_string_lossy();

        if name != "lost+found" && name != "meta-hybrid" && !active_ids.contains(name.as_ref()) {
            tracing::info!("Pruning orphaned module storage: {}", name);

            if path.is_dir() {
                if let Err(e) = fs::remove_dir_all(&path) {
                    tracing::warn!("Failed to remove orphan dir {}: {}", name, e);
                }
            } else if let Err(e) = fs::remove_file(&path) {
                tracing::warn!("Failed to remove orphan file {}: {}", name, e);
            }
        }
    });

    Ok(())
}

fn should_sync(src: &Path, dst: &Path) -> bool {
    if !dst.exists() {
        return true;
    }

    let src_prop = src.join("module.prop");

    let dst_prop = dst.join("module.prop");

    if !src_prop.exists() || !dst_prop.exists() {
        return true;
    }

    match (fs::read(&src_prop), fs::read(&dst_prop)) {
        (Ok(s), Ok(d)) => s != d,
        _ => true,
    }
}

fn has_files_recursive(path: &Path) -> bool {
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            if entry.file_type().is_ok() {
                return true;
            }
        }
    }

    false
}
