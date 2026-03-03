use std::{collections::HashSet, fs, path::Path};

use anyhow::Result;
use rayon::prelude::*;
use walkdir::WalkDir;

use crate::{
    core::inventory::Module,
    defs,
    sys::fs::{prune_empty_dirs, set_overlay_opaque, sync_dir},
};

pub fn perform_sync(modules: &[Module], target_base: &Path) -> Result<()> {
    log::info!("Starting smart module sync to {}", target_base.display());

    prune_orphaned_modules(modules, target_base)?;

    modules.par_iter().for_each(|module| {
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
                return;
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
                    return;
                }
                backup_created = true;
            }

            if let Err(e) = fs::rename(&tmp_dst, &dst) {
                log::error!("Failed to commit atomic sync for {}: {}", module.id, e);
                if backup_created {
                    let _ = fs::rename(&dst_backup, &dst);
                }
                let _ = fs::remove_dir_all(&tmp_dst);
                return;
            }

            if backup_created && let Err(e) = fs::remove_dir_all(&dst_backup) {
                log::warn!("Failed to clean up backup for {}: {}", module.id, e);
            }
        } else {
            log::debug!("Skipping module: {}", module.id);
        }
    });

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

    let entries: Vec<_> = fs::read_dir(target_base)?.filter_map(|e| e.ok()).collect();

    entries.par_iter().for_each(|entry| {
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

    if !matches!((fs::read(&src_prop), fs::read(&dst_prop)), (Ok(s), Ok(d)) if s == d) {
        return true;
    }

    match (collect_snapshot(src), collect_snapshot(dst)) {
        (Ok(src_snapshot), Ok(dst_snapshot)) => src_snapshot != dst_snapshot,
        _ => true,
    }
}

fn collect_snapshot(root: &Path) -> Result<Vec<String>> {
    let mut snapshot = Vec::new();

    for entry in WalkDir::new(root).into_iter().flatten() {
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        let file_type = entry.file_type();
        if file_type.is_dir() {
            snapshot.push(format!("d:{}", relative));
            continue;
        }

        if file_type.is_symlink() {
            let target = fs::read_link(path)
                .ok()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            snapshot.push(format!("l:{}:{}", relative, target));
            continue;
        }

        let metadata = fs::symlink_metadata(path)?;
        let content_hash = fs::read(path)
            .ok()
            .map(|data| {
                use std::hash::{Hash, Hasher};
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                data.hash(&mut hasher);
                hasher.finish()
            })
            .unwrap_or_default();
        snapshot.push(format!(
            "f:{}:{}:{}",
            relative,
            metadata.len(),
            content_hash
        ));
    }

    snapshot.sort();
    Ok(snapshot)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn create_temp_dir(name: &str) -> std::path::PathBuf {
        let base = std::env::temp_dir();
        let unique = format!(
            "hybrid_mount_sync_test_{}_{}_{}",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        let dir = base.join(unique);
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn write_file(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        let mut file = fs::File::create(path).expect("create file");
        file.write_all(content.as_bytes()).expect("write file");
    }

    #[test]
    fn should_sync_when_dst_missing() {
        let src = create_temp_dir("dst_missing_src");
        let dst = create_temp_dir("dst_missing_dst");
        let dst_target = dst.join("not_exists");

        write_file(&src.join("module.prop"), "id=a\n");

        assert!(should_sync(&src, &dst_target));

        let _ = fs::remove_dir_all(src);
        let _ = fs::remove_dir_all(dst);
    }

    #[test]
    fn should_sync_when_payload_changes_with_same_module_prop() {
        let src = create_temp_dir("payload_src");
        let dst = create_temp_dir("payload_dst");

        write_file(&src.join("module.prop"), "id=a\nname=A\n");
        write_file(&dst.join("module.prop"), "id=a\nname=A\n");

        write_file(&src.join("system/bin/app_process"), "v2");
        write_file(&dst.join("system/bin/app_process"), "v1");

        assert!(should_sync(&src, &dst));

        let _ = fs::remove_dir_all(src);
        let _ = fs::remove_dir_all(dst);
    }

    #[test]
    fn should_not_sync_when_snapshots_equal() {
        let src = create_temp_dir("equal_src");
        let dst = create_temp_dir("equal_dst");

        write_file(&src.join("module.prop"), "id=a\nname=A\n");
        write_file(&dst.join("module.prop"), "id=a\nname=A\n");

        write_file(&src.join("system/etc/test.conf"), "same");
        write_file(&dst.join("system/etc/test.conf"), "same");

        assert!(!should_sync(&src, &dst));

        let _ = fs::remove_dir_all(src);
        let _ = fs::remove_dir_all(dst);
    }
}
