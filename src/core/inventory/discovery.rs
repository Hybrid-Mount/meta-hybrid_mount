// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::{
    fs,
    io::{self, BufRead, BufReader},
    path::{Path, PathBuf},
};

use anyhow::Result;

use crate::{conf::config, core::inventory, domain::ModuleRules, utils::validate_module_id};

#[derive(Debug, Clone)]
pub struct Module {
    pub id: String,
    pub source_path: PathBuf,
    pub rules: ModuleRules,
}

pub fn scan(source_dir: &Path, cfg: &config::Config) -> Result<Vec<Module>> {
    if !source_dir.exists() {
        return Ok(Vec::new());
    }

    let mut modules = Vec::new();
    let mut skipped_reserved = 0usize;
    let mut skipped_non_directories = 0usize;
    let mut skipped_blocked = 0usize;
    let mut skipped_blacklisted = 0usize;
    let mut skipped_invalid = 0usize;
    let mut skipped_missing_prop = 0usize;

    for entry in fs::read_dir(source_dir)? {
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
        let id = entry.file_name().to_string_lossy().into_owned();

        if inventory::is_reserved_module_dir(&id) {
            skipped_reserved += 1;
            crate::scoped_log!(debug, "scanner", "skip: module={}, reason=reserved_dir", id);
            continue;
        }

        if validate_module_id(&id).is_err() {
            skipped_invalid += 1;
            crate::scoped_log!(
                warn,
                "scanner",
                "skip: module={}, reason=invalid_dir_name",
                id
            );
            continue;
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
        if !module_prop_id_matches_dir(&prop, &id)? {
            skipped_invalid += 1;
            crate::scoped_log!(
                warn,
                "scanner",
                "skip: module={}, reason=module_prop_id_mismatch",
                id
            );
            continue;
        }

        if cfg.module_blacklist.contains(&id) {
            skipped_blacklisted += 1;
            crate::scoped_log!(debug, "scanner", "skip: module={}, reason=blacklisted", id);
            continue;
        }

        let block_markers = inventory::mount_block_markers(&path);
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
            rules: inventory::load_module_rules(cfg, &id),
        });
    }

    crate::scoped_log!(
        info,
        "scanner",
        "complete: total_dirs={}, active_modules={}, skipped_reserved={}, skipped_non_directories={}, skipped_blocked={}, skipped_blacklisted={}, skipped_invalid={}, skipped_missing_prop={}",
        modules.len()
            + skipped_reserved
            + skipped_non_directories
            + skipped_blocked
            + skipped_blacklisted
            + skipped_invalid
            + skipped_missing_prop,
        modules.len(),
        skipped_reserved,
        skipped_non_directories,
        skipped_blocked,
        skipped_blacklisted,
        skipped_invalid,
        skipped_missing_prop
    );

    modules.sort_by(|a, b| a.id.cmp(&b.id));

    Ok(modules)
}

pub fn module_prop_id_matches_dir(prop: &Path, dir_id: &str) -> Result<bool> {
    Ok(read_module_prop_id(prop)?
        .as_deref()
        .is_some_and(|prop_id| validate_module_id(prop_id).is_ok() && prop_id == dir_id))
}

fn read_module_prop_id(prop: &Path) -> Result<Option<String>> {
    let file = fs::File::open(prop)?;
    for line_result in BufReader::new(file).lines() {
        let line = match line_result {
            Ok(line) => line,
            Err(err) if err.kind() == io::ErrorKind::InvalidData => {
                crate::scoped_log!(
                    warn,
                    "scanner",
                    "module.prop read skipped: path={}, reason=invalid_utf8",
                    prop.display()
                );
                return Ok(None);
            }
            Err(err) => return Err(err.into()),
        };
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

    #[test]
    fn scan_skips_non_directory_entries() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("stray.txt"), b"not a module").unwrap();
        let valid = temp.path().join("valid");
        fs::create_dir(&valid).unwrap();
        write_prop(&valid, "valid");

        let modules = scan(temp.path(), &config::Config::default()).unwrap();

        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0].id, "valid");
    }

    #[test]
    fn scan_skips_missing_module_prop() {
        let temp = TempDir::new().unwrap();
        fs::create_dir(temp.path().join("alpha")).unwrap();

        let modules = scan(temp.path(), &config::Config::default()).unwrap();

        assert!(modules.is_empty());
    }

    #[test]
    fn scan_skips_invalid_module_dir_name() {
        let temp = TempDir::new().unwrap();
        let module_dir = temp.path().join("bad:name");
        fs::create_dir(&module_dir).unwrap();
        write_prop(&module_dir, "bad:name");

        let modules = scan(temp.path(), &config::Config::default()).unwrap();

        assert!(modules.is_empty());
    }

    #[test]
    fn scan_requires_prop_id_to_match_directory_when_present() {
        let temp = TempDir::new().unwrap();
        let module_dir = temp.path().join("alpha");
        fs::create_dir(&module_dir).unwrap();
        write_prop(&module_dir, "beta");

        let modules = scan(temp.path(), &config::Config::default()).unwrap();

        assert!(modules.is_empty());
    }

    #[test]
    fn scan_requires_module_prop_id() {
        let temp = TempDir::new().unwrap();
        let module_dir = temp.path().join("alpha");
        fs::create_dir(&module_dir).unwrap();
        write_prop_content(&module_dir, "name=Alpha\n");

        let modules = scan(temp.path(), &config::Config::default()).unwrap();

        assert!(modules.is_empty());
    }

    #[test]
    fn scan_rejects_invalid_module_prop_id() {
        let temp = TempDir::new().unwrap();
        let module_dir = temp.path().join("alpha");
        fs::create_dir(&module_dir).unwrap();
        write_prop_content(&module_dir, "id=1alpha\n");

        let modules = scan(temp.path(), &config::Config::default()).unwrap();

        assert!(modules.is_empty());
    }

    #[test]
    fn scan_accepts_valid_module_with_matching_prop_id() {
        let temp = TempDir::new().unwrap();
        let module_dir = temp.path().join("alpha");
        fs::create_dir(&module_dir).unwrap();
        write_prop(&module_dir, "alpha");

        let modules = scan(temp.path(), &config::Config::default()).unwrap();

        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0].id, "alpha");
    }

    #[test]
    fn scan_trims_module_prop_id_and_ignores_comments() {
        let temp = TempDir::new().unwrap();
        let module_dir = temp.path().join("alpha");
        fs::create_dir(&module_dir).unwrap();
        write_prop_content(&module_dir, "# id=wrong\n  id = alpha  \n");

        let modules = scan(temp.path(), &config::Config::default()).unwrap();

        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0].id, "alpha");
    }
}
