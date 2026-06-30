// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::{
    collections::HashSet,
    fs,
    os::unix::fs::{FileTypeExt, MetadataExt},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use walkdir::WalkDir;

use super::common::build_managed_partitions;
use crate::{
    conf::config,
    core::{
        inventory::Module,
        ops::plan::{KasumiAddRule, KasumiMergeRule, MountPlan},
    },
    defs,
    domain::MountMode,
    utils,
};

#[derive(Debug, Default)]
pub(super) struct CompiledRules {
    pub(super) add_rules: Vec<KasumiAddRule>,
    pub(super) merge_rules: Vec<KasumiMergeRule>,
    pub(super) hide_rules: Vec<String>,
}

fn mirror_module_root(config: &config::Config, module: &Module) -> Result<PathBuf> {
    let module_root = config.kasumi.mirror_path.join(&module.id);
    if module_root.exists() {
        Ok(module_root)
    } else {
        bail!(
            "missing Kasumi mirror content for module {} at {}",
            module.id,
            module_root.display()
        )
    }
}

fn build_dtype(path: &Path) -> Result<(i32, bool)> {
    let metadata = fs::symlink_metadata(path).with_context(|| {
        format!(
            "failed to read metadata for kasumi source {}",
            path.display()
        )
    })?;
    let file_type = metadata.file_type();

    if file_type.is_char_device() && metadata.rdev() == 0 {
        return Ok((libc::DT_UNKNOWN as i32, true));
    }

    let d_type = if file_type.is_file() {
        libc::DT_REG
    } else if file_type.is_symlink() {
        libc::DT_LNK
    } else if file_type.is_dir() {
        libc::DT_DIR
    } else if file_type.is_block_device() {
        libc::DT_BLK
    } else if file_type.is_char_device() {
        libc::DT_CHR
    } else if file_type.is_fifo() {
        libc::DT_FIFO
    } else if file_type.is_socket() {
        libc::DT_SOCK
    } else {
        libc::DT_UNKNOWN
    };

    Ok((d_type as i32, false))
}

pub(super) fn log_compiled_rule_summary(compiled: &CompiledRules, user_hide_paths: &[PathBuf]) {
    crate::scoped_log!(
        debug,
        "mount:kasumi",
        "compiled rules: add_rules={}, merge_rules={}, hide_rules={}, user_hide_rules={}",
        compiled.add_rules.len(),
        compiled.merge_rules.len(),
        compiled.hide_rules.len(),
        user_hide_paths.len()
    );
}

fn relative_mode(module: &Module, relative: &Path) -> MountMode {
    let relative_str = relative.to_string_lossy();
    module.rules.get_mode(relative_str.as_ref())
}

pub(super) fn compile_rules(
    modules: &[Module],
    plan: &MountPlan,
    config: &config::Config,
) -> Result<CompiledRules> {
    let system_root = Path::new("/");
    let managed_partitions = build_managed_partitions(config);
    let active_ids: HashSet<&str> = plan.kasumi_module_ids.iter().map(String::as_str).collect();
    let mut compiled = CompiledRules::default();
    let mut managed_partition_list: Vec<String> = managed_partitions.into_iter().collect();
    managed_partition_list.sort();

    for module in modules.iter().rev() {
        if !active_ids.contains(module.id.as_str()) {
            continue;
        }

        let module_root = mirror_module_root(config, module)?;
        let mut scanned_partition_roots: HashSet<PathBuf> = HashSet::new();
        let mut symlink_directory_skips = 0usize;

        for partition_name in &managed_partition_list {
            let partition_root = module_root.join(partition_name);
            if !partition_root.is_dir() {
                continue;
            }
            let normalized_partition_root = utils::resolve_link_path(&partition_root);
            if !scanned_partition_roots.insert(normalized_partition_root) {
                crate::scoped_log!(
                    debug,
                    "mount:kasumi",
                    "partition root dedupe: module={}, partition={}, root={}",
                    module.id,
                    partition_name,
                    partition_root.display()
                );
                continue;
            }

            let mut iterator = WalkDir::new(&partition_root)
                .follow_links(false)
                .into_iter();

            while let Some(entry_result) = iterator.next() {
                let entry = match entry_result {
                    Ok(entry) => entry,
                    Err(err) => {
                        crate::scoped_log!(
                            warn,
                            "mount:kasumi",
                            "walk failed: module={}, partition={}, error={}",
                            module.id,
                            partition_name,
                            err
                        );
                        continue;
                    }
                };

                if entry.depth() == 0 {
                    continue;
                }

                let path = entry.path();
                let relative = match path.strip_prefix(&module_root) {
                    Ok(relative) => relative,
                    Err(err) => {
                        crate::scoped_log!(
                            warn,
                            "mount:kasumi",
                            "relative path failed: module={}, path={}, error={}",
                            module.id,
                            path.display(),
                            err
                        );
                        continue;
                    }
                };

                if !matches!(relative_mode(module, relative), MountMode::Kasumi) {
                    continue;
                }

                if utils::path_file_name_eq_ignore_ascii_case(path, defs::REPLACE_DIR_FILE_NAME) {
                    continue;
                }

                let resolved_virtual_path =
                    utils::resolve_path_with_root(system_root, &Path::new("/").join(relative));
                let target_key = resolved_virtual_path.display().to_string();

                if entry.file_type().is_dir() {
                    if resolved_virtual_path.is_dir() {
                        compiled.merge_rules.push(KasumiMergeRule {
                            target: target_key,
                            source: path.to_path_buf(),
                        });
                        iterator.skip_current_dir();
                    }
                    continue;
                }

                if entry.file_type().is_symlink()
                    && resolved_virtual_path.exists()
                    && resolved_virtual_path.is_dir()
                {
                    symlink_directory_skips += 1;
                    continue;
                }

                let (file_type, hide_only) = build_dtype(path)?;
                if hide_only {
                    compiled.hide_rules.push(target_key);
                    continue;
                }

                compiled.add_rules.push(KasumiAddRule {
                    target: target_key,
                    source: path.to_path_buf(),
                    file_type,
                });
            }
        }

        if symlink_directory_skips > 0 {
            crate::scoped_log!(
                warn,
                "mount:kasumi",
                "symlink skip summary: module={}, reason=directory_target, count={}",
                module.id,
                symlink_directory_skips
            );
        }
    }

    Ok(compiled)
}
