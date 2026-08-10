// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::{
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};

use crate::{conf::config, core::inventory, domain::ModuleRules, utils::validate_module_id};

#[derive(Debug, Clone)]
pub struct Module {
    pub id: String,
    pub source_path: PathBuf,
    pub rules: ModuleRules,
}

#[derive(Debug, Clone, Default)]
pub struct InventorySummary {
    pub skip_mount_modules: Vec<String>,
    pub blacklisted_modules: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct InventorySnapshot {
    pub modules: Vec<Module>,
    pub summary: InventorySummary,
}

pub fn scan(cfg: &config::Config) -> Result<Vec<Module>> {
    Ok(scan_snapshot(cfg)?.modules)
}

pub fn scan_snapshot(cfg: &config::Config) -> Result<InventorySnapshot> {
    let source_dir = &cfg.moduledir;
    let timer = crate::utils::StageTimer::start("scanner", "inventory_scan");
    if !source_dir.is_dir() {
        bail!(
            "module source directory is unavailable: {}",
            source_dir.display()
        );
    }

    let mut modules = Vec::new();
    let mut summary = InventorySummary {
        blacklisted_modules: cfg
            .module_blacklist
            .iter()
            .filter(|id| source_dir.join(id).is_dir())
            .cloned()
            .collect(),
        ..Default::default()
    };
    summary.blacklisted_modules.sort();
    let mut skipped_reserved = 0usize;
    let mut skipped_non_directories = 0usize;
    let mut skipped_blocked = 0usize;
    let mut skipped_blacklisted = 0usize;
    let mut skipped_missing_prop = 0usize;
    let mut root_entries_scanned = 0usize;
    let mut marker_directory_scans = 0usize;

    for entry in fs::read_dir(source_dir)? {
        root_entries_scanned += 1;
        let entry = entry?;
        let file_type = entry.file_type()?;
        if !file_type.is_dir() {
            skipped_non_directories += 1;
            crate::scoped_log!(
                warn,
                "scanner",
                "skip: path={}, reason=non_directory_entry",
                entry.path().display()
            );
            continue;
        }

        let path = entry.path();
        let id = entry
            .file_name()
            .into_string()
            .map_err(|_| anyhow::anyhow!("module directory name is not valid UTF-8"))?;

        if inventory::is_reserved_module_dir(&id) {
            skipped_reserved += 1;
            crate::scoped_log!(debug, "scanner", "skip: module={}, reason=reserved_dir", id);
            continue;
        }

        marker_directory_scans += 1;
        let block_markers = inventory::mount_block_markers(&path)?;
        if block_markers.contains(&crate::defs::SKIP_MOUNT_FILE_NAME) {
            summary.skip_mount_modules.push(id.clone());
        }

        let prop = path.join("module.prop");
        if !prop.is_file() {
            skipped_missing_prop += 1;
            crate::scoped_log!(
                debug,
                "scanner",
                "skip: module={}, reason=missing_module_prop",
                id
            );
            continue;
        }
        validate_module_id(&id).with_context(|| format!("invalid module directory name: {id}"))?;
        validate_module_prop_id(&prop, &id)?;

        if cfg.module_blacklist.contains(&id) {
            skipped_blacklisted += 1;
            crate::scoped_log!(debug, "scanner", "skip: module={}, reason=blacklisted", id);
            continue;
        }

        if !block_markers.is_empty() {
            skipped_blocked += 1;
            crate::scoped_log!(
                debug,
                "scanner",
                "skip: module={}, reason=block_marker, markers={}",
                id,
                block_markers.join(",")
            );
            continue;
        }

        modules.push(Module {
            id: id.clone(),
            source_path: path,
            rules: inventory::load_module_rules(cfg, &id)?,
        });
    }

    timer.finish();

    crate::scoped_log!(
        info,
        "scanner",
        "complete: total_entries={}, active_modules={}, skipped_reserved={}, skipped_non_directories={}, skipped_blocked={}, skipped_blacklisted={}, skipped_missing_prop={}, root_entries_scanned={}, marker_directory_scans={}",
        modules.len()
            + skipped_reserved
            + skipped_non_directories
            + skipped_blocked
            + skipped_blacklisted
            + skipped_missing_prop,
        modules.len(),
        skipped_reserved,
        skipped_non_directories,
        skipped_blocked,
        skipped_blacklisted,
        skipped_missing_prop,
        root_entries_scanned,
        marker_directory_scans
    );

    modules.sort_by(|a, b| a.id.cmp(&b.id));
    summary.skip_mount_modules.sort();
    summary.skip_mount_modules.dedup();

    Ok(InventorySnapshot { modules, summary })
}

pub(crate) fn validate_module_prop_id(prop: &Path, dir_id: &str) -> Result<()> {
    let prop_id = read_module_prop_id(prop)?
        .with_context(|| format!("module.prop has no id: {}", prop.display()))?;
    validate_module_id(&prop_id)
        .with_context(|| format!("module.prop contains invalid id {prop_id:?}"))?;
    if prop_id != dir_id {
        bail!("module.prop id {prop_id:?} does not match directory {dir_id:?}");
    }
    Ok(())
}

fn read_module_prop_id(prop: &Path) -> Result<Option<String>> {
    let file = fs::File::open(prop)?;
    for line_result in BufReader::new(file).lines() {
        let line = line_result?;
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = trimmed.split_once('=')
            && key.trim() == "id"
        {
            return Ok(Some(value.trim().to_string()));
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    fn write_prop(module_dir: &Path, id: &str) {
        fs::write(module_dir.join("module.prop"), format!("id={id}\n")).unwrap();
    }

    fn write_prop_content(module_dir: &Path, content: &str) {
        fs::write(module_dir.join("module.prop"), content).unwrap();
    }

    fn test_config(module_dir: &Path) -> config::Config {
        config::Config {
            moduledir: module_dir.to_path_buf(),
            ..Default::default()
        }
    }

    #[test]
    fn scan_skips_missing_module_prop_and_keeps_valid_modules() {
        let temp = TempDir::new().unwrap();
        fs::create_dir(temp.path().join("incomplete")).unwrap();
        let valid = temp.path().join("valid");
        fs::create_dir(&valid).unwrap();
        write_prop(&valid, "valid");

        let modules = scan(&test_config(temp.path())).unwrap();

        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0].id, "valid");
    }

    #[test]
    fn scan_skips_non_directory_entries_and_keeps_valid_modules() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("stray.txt"), b"not a module").unwrap();
        let valid = temp.path().join("valid");
        fs::create_dir(&valid).unwrap();
        write_prop(&valid, "valid");

        let modules = scan(&test_config(temp.path())).unwrap();

        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0].id, "valid");
    }

    #[test]
    fn scan_rejects_invalid_module_dir_name() {
        let temp = TempDir::new().unwrap();
        let module_dir = temp.path().join("bad:name");
        fs::create_dir(&module_dir).unwrap();
        write_prop(&module_dir, "bad:name");

        assert!(scan(&test_config(temp.path())).is_err());
    }

    #[test]
    fn scan_requires_prop_id_to_match_directory_when_present() {
        let temp = TempDir::new().unwrap();
        let module_dir = temp.path().join("alpha");
        fs::create_dir(&module_dir).unwrap();
        write_prop(&module_dir, "beta");

        assert!(scan(&test_config(temp.path())).is_err());
    }

    #[test]
    fn scan_requires_module_prop_id() {
        let temp = TempDir::new().unwrap();
        let module_dir = temp.path().join("alpha");
        fs::create_dir(&module_dir).unwrap();
        write_prop_content(&module_dir, "name=Alpha\n");

        assert!(scan(&test_config(temp.path())).is_err());
    }

    #[test]
    fn scan_rejects_invalid_module_prop_id() {
        let temp = TempDir::new().unwrap();
        let module_dir = temp.path().join("alpha");
        fs::create_dir(&module_dir).unwrap();
        write_prop_content(&module_dir, "id=1alpha\n");

        assert!(scan(&test_config(temp.path())).is_err());
    }

    #[test]
    fn scan_accepts_valid_module_with_matching_prop_id() {
        let temp = TempDir::new().unwrap();
        let module_dir = temp.path().join("alpha");
        fs::create_dir(&module_dir).unwrap();
        write_prop(&module_dir, "alpha");

        let modules = scan(&test_config(temp.path())).unwrap();

        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0].id, "alpha");
    }

    #[test]
    fn scan_trims_module_prop_id_and_ignores_comments() {
        let temp = TempDir::new().unwrap();
        let module_dir = temp.path().join("alpha");
        fs::create_dir(&module_dir).unwrap();
        write_prop_content(&module_dir, "# id=wrong\n  id = alpha  \n");

        let modules = scan(&test_config(temp.path())).unwrap();

        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0].id, "alpha");
    }

    #[test]
    fn snapshot_keeps_skip_and_blacklist_summary_without_rescanning() {
        let temp = TempDir::new().unwrap();
        let skipped = temp.path().join("skipped");
        let blacklisted = temp.path().join("blacklisted");
        fs::create_dir(&skipped).unwrap();
        fs::create_dir(&blacklisted).unwrap();
        write_prop(&skipped, "skipped");
        write_prop(&blacklisted, "blacklisted");
        fs::write(skipped.join("skip_mount"), b"").unwrap();

        let config = config::Config {
            moduledir: temp.path().to_path_buf(),
            module_blacklist: vec!["blacklisted".to_string()],
            ..Default::default()
        };
        let snapshot = scan_snapshot(&config).unwrap();

        assert!(snapshot.modules.is_empty());
        assert_eq!(snapshot.summary.skip_mount_modules, vec!["skipped"]);
        assert_eq!(snapshot.summary.blacklisted_modules, vec!["blacklisted"]);
    }
}
