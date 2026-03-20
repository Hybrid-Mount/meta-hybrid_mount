// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{collections::HashSet, fs, os::unix::fs::symlink, path::Path};

use anyhow::{Context, Result};
use walkdir::WalkDir;

use crate::{
    core::inventory::Module,
    defs,
    sys::fs::{prune_empty_dirs, set_overlay_opaque, sync_dir},
};

pub fn normalize_module_layout<P: AsRef<Path>>(module_dir: P) -> Result<()> {
    let base = module_dir.as_ref();

    let mappings = vec![
        ("system/vendor", "vendor"),
        ("system/product", "product"),
        ("system/system_ext", "system_ext"),
        ("system/odm", "odm"),
        ("system/oem", "oem"),
    ];

    for (src_rel, dst_rel) in mappings {
        let src_path = base.join(src_rel);
        let dst_path = base.join(dst_rel);

        if src_path.is_dir() && !dst_path.exists() {
            symlink(&src_path, &dst_path)?;
        }
    }

    Ok(())
}

pub fn perform_sync(modules: &[Module], target_base: &Path) -> Result<()> {
    log::info!("Starting smart module sync to {}", target_base.display());

    prune_orphaned_modules(modules, target_base)?;

    for module in modules {
        if let Err(e) = normalize_module_layout(&module.source_path) {
            log::warn!("Failed to normalize layout for {}: {}", module.id, e);
        }

        let dst = target_base.join(&module.id);
        let dst_backup = target_base.join(format!(".backup_{}", module.id));

        let has_content = defs::BUILTIN_PARTITIONS.iter().any(|p| {
            let part_path = module.source_path.join(p);

            part_path.exists() && has_files_recursive(&part_path)
        });

        if has_content && should_sync(&module.source_path, &dst) {
            log::info!("Syncing module: {} (Updated/New)", module.id);

            let tmp_dst = target_base.join(format!(".tmp_{}", module.id));

            if tmp_dst.exists() {
                let _ = fs::remove_dir_all(&tmp_dst);
            }

            if let Err(e) = sync_dir(&module.source_path, &tmp_dst, true) {
                log::error!("Failed to sync module {}: {}", module.id, e);
                let _ = fs::remove_dir_all(&tmp_dst);
                return Err(e).with_context(|| format!("Failed to sync module {}", module.id));
            }

            if let Err(e) = prune_empty_dirs(&tmp_dst) {
                log::warn!("Failed to prune empty dirs for {}: {}", module.id, e);
            }

            if let Err(e) = apply_overlay_opaque_flags(&tmp_dst) {
                log::warn!(
                    "Failed to apply overlay opaque xattrs for {}: {}",
                    module.id,
                    e
                );
            }

            let mut backup_created = false;
            if dst.exists() {
                if let Err(e) = fs::rename(&dst, &dst_backup) {
                    log::error!("Failed to backup existing module {}: {}", module.id, e);
                    let _ = fs::remove_dir_all(&tmp_dst);
                    return Err(e)
                        .with_context(|| format!("Failed to back up module {}", module.id));
                }
                backup_created = true;
            }

            if let Err(e) = fs::rename(&tmp_dst, &dst) {
                log::error!("Failed to commit atomic sync for {}: {}", module.id, e);
                if backup_created {
                    let _ = fs::rename(&dst_backup, &dst);
                }
                let _ = fs::remove_dir_all(&tmp_dst);
                return Err(e)
                    .with_context(|| format!("Failed to commit synced module {}", module.id));
            }

            if backup_created && let Err(e) = fs::remove_dir_all(&dst_backup) {
                log::warn!("Failed to clean up backup for {}: {}", module.id, e);
            }
        } else {
            log::debug!("Skipping module: {}", module.id);
        }
    }

    Ok(())
}

fn apply_overlay_opaque_flags(root: &Path) -> Result<()> {
    for entry in WalkDir::new(root).min_depth(1).into_iter().flatten() {
        if entry.file_type().is_file()
            && entry.file_name() == defs::REPLACE_DIR_FILE_NAME
            && let Some(parent) = entry.path().parent()
        {
            set_overlay_opaque(parent)?;
            log::debug!("Set overlay opaque xattr on: {}", parent.display());
        }
    }
    Ok(())
}

fn prune_orphaned_modules(modules: &[Module], target_base: &Path) -> Result<()> {
    if !target_base.exists() {
        return Ok(());
    }

    let active_ids: HashSet<&str> = modules.iter().map(|m| m.id.as_str()).collect();

    for entry in target_base.read_dir()?.flatten() {
        let path = entry.path();

        let name_os = entry.file_name();

        let name = name_os.to_string_lossy();

        if name != "lost+found"
            && name != "hybrid_mount"
            && !name.starts_with('.')
            && !active_ids.contains(name.as_ref())
        {
            log::info!("Pruning orphaned module storage: {}", name);

            if path.is_dir() {
                if let Err(e) = fs::remove_dir_all(&path) {
                    log::warn!("Failed to remove orphan dir {}: {}", name, e);
                }
            } else if let Err(e) = fs::remove_file(&path) {
                log::warn!("Failed to remove orphan file {}: {}", name, e);
            }
        }
    }

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

    if let (Ok(s), Ok(d)) = (fs::read(&src_prop), fs::read(&dst_prop)) {
        if s != d {
            return true;
        }
    } else {
        return true;
    }

    let src_post_fs = src.join("post-fs-data.sh");
    let dst_post_fs = dst.join("post-fs-data.sh");

    if src_post_fs.exists() && dst_post_fs.exists() {
        if let (Ok(s_meta), Ok(d_meta)) = (fs::metadata(&src_post_fs), fs::metadata(&dst_post_fs))
            && let (Ok(s_time), Ok(d_time)) = (s_meta.modified(), d_meta.modified())
            && s_time > d_time
        {
            return true;
        }
    } else if src_post_fs.exists() != dst_post_fs.exists() {
        return true;
    }

    false
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
