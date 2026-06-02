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
    collections::{BTreeSet, HashSet},
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::{
    conf::config::Config,
    core::{inventory, runtime_state::RuntimeState},
    defs,
    domain::{ModuleRules, MountMode},
    utils,
};

#[derive(Debug, Clone, Serialize)]
pub struct ModuleListEntry {
    pub id: String,
    pub mode: MountMode,
    pub is_mounted: bool,
    pub enabled: bool,
    pub source_path: PathBuf,
    pub rules: ModuleRules,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mount_error: Option<String>,
    #[serde(skip_serializing_if = "is_false")]
    pub suggest_ignore: bool,
}

fn is_false(v: &bool) -> bool {
    !*v
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleApplyEntry {
    pub id: String,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub source_path: Option<PathBuf>,
    #[serde(default)]
    pub rules: ModuleRules,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModulesApplyPayload {
    pub updated: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct VersionPayload {
    pub version: String,
}

#[derive(Debug, Clone)]
struct ModuleMetadata {
    name: String,
    version: String,
    author: String,
    description: String,
}

const MAX_MODULE_PROP_BYTES: u64 = 64 * 1024;

pub fn build_modules_payload(
    config: &Config,
    state: &RuntimeState,
    path: Option<&Path>,
) -> Result<Vec<ModuleListEntry>> {
    if let Some(source_dir) = path {
        return build_scanned_modules_payload(config, state, source_dir);
    }

    Ok(build_runtime_modules_payload(config, state))
}

fn build_scanned_modules_payload(
    config: &Config,
    state: &RuntimeState,
    source_dir: &Path,
) -> Result<Vec<ModuleListEntry>> {
    if !source_dir.exists() {
        return Ok(Vec::new());
    }

    let runtime_index = RuntimeModuleIndex::new(state);
    let mut modules = Vec::new();
    for entry in fs::read_dir(source_dir)
        .with_context(|| format!("failed to read module directory {}", source_dir.display()))?
    {
        let entry = entry.with_context(|| {
            format!(
                "failed to enumerate module directory {}",
                source_dir.display()
            )
        })?;
        let file_type = entry.file_type().with_context(|| {
            format!(
                "failed to read module entry type {}",
                entry.path().display()
            )
        })?;
        if !file_type.is_dir() {
            continue;
        }

        let module_path = entry.path();
        let id = entry.file_name().to_string_lossy().into_owned();
        if inventory::is_reserved_module_dir(&id) {
            continue;
        }

        let rules = inventory::load_module_rules(config, &id);
        let is_blacklisted =
            runtime_index.is_blacklisted(&id) || config.module_blacklist.contains(&id);
        let enabled = !is_blacklisted && !inventory::has_mount_block_marker(&module_path);
        let runtime_mode = enabled.then(|| runtime_index.mode(&id)).flatten();
        let mode = if is_blacklisted {
            MountMode::Ignore
        } else {
            runtime_mode.unwrap_or(rules.default_mode)
        };

        let mount_error = if is_blacklisted {
            Some("blacklisted".to_string())
        } else {
            mount_error_reason(&runtime_index, &id, &module_path)
        };
        let suggest_ignore = mount_error.is_some() && has_suspicious_shell_commands(&module_path);

        modules.push(ModuleListEntry {
            id,
            mode,
            is_mounted: runtime_mode.is_some(),
            enabled,
            source_path: module_path,
            rules,
            mount_error,
            suggest_ignore,
        });
    }

    modules.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(modules)
}

fn build_runtime_modules_payload(config: &Config, state: &RuntimeState) -> Vec<ModuleListEntry> {
    let runtime_index = RuntimeModuleIndex::new(state);
    let mut ids = BTreeSet::new();
    ids.extend(state.overlay_modules.iter().cloned());
    ids.extend(state.magic_modules.iter().cloned());
    ids.extend(state.kasumi_modules.iter().cloned());
    ids.extend(state.skip_mount_modules.iter().cloned());
    ids.extend(state.blacklisted_modules.iter().cloned());
    ids.extend(state.mount_error_modules.iter().cloned());
    ids.extend(collect_mount_error_marker_modules(&config.moduledir));
    ids.extend(config.rules.keys().cloned());

    ids.into_iter()
        .filter(|id| !inventory::is_reserved_module_dir(id))
        .map(|id| {
            let source_path = config.moduledir.join(&id);
            let rules = inventory::load_module_rules(config, &id);
            let is_blacklisted = runtime_index.is_blacklisted(&id);
            let runtime_mode = if is_blacklisted {
                None
            } else {
                runtime_index.mode(&id)
            };
            let mode = if is_blacklisted {
                MountMode::Ignore
            } else {
                runtime_mode.unwrap_or(rules.default_mode)
            };
            let enabled = !is_blacklisted
                && runtime_index.enabled(&id)
                && !inventory::has_mount_block_marker(&source_path);

            let mount_error = if is_blacklisted {
                Some("blacklisted".to_string())
            } else {
                mount_error_reason(&runtime_index, &id, &source_path)
            };
            let suggest_ignore =
                mount_error.is_some() && has_suspicious_shell_commands(&source_path);

            ModuleListEntry {
                id,
                mode,
                is_mounted: runtime_mode.is_some(),
                enabled,
                source_path,
                rules,
                mount_error,
                suggest_ignore,
            }
        })
        .collect()
}

fn collect_mount_error_marker_modules(moduledir: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(moduledir) else {
        return Vec::new();
    };

    entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_dir()
                || !utils::dir_contains_entry_case_insensitive(&path, defs::MOUNT_ERROR_FILE_NAME)
            {
                return None;
            }

            let id = entry.file_name().to_string_lossy().into_owned();
            (!inventory::is_reserved_module_dir(&id)).then_some(id)
        })
        .collect()
}

fn mount_error_reason(
    runtime_index: &RuntimeModuleIndex<'_>,
    module_id: &str,
    module_path: &Path,
) -> Option<String> {
    runtime_index.mount_error_reason(module_id).or_else(|| {
        utils::dir_contains_entry_case_insensitive(module_path, defs::MOUNT_ERROR_FILE_NAME)
            .then(|| "mount_error marker present".to_string())
    })
}

/// Scans .sh files in the module directory for shell commands that suggest the
/// module performs its own mount operations (mount, bind mount, mkdir, touch).
/// When true, the user should consider setting the module to "ignore" mode
/// because Hybrid Mount cannot manage modules that do their own mounting.
const MAX_SH_SCAN_BYTES: u64 = 256 * 1024;

fn has_suspicious_shell_commands(module_path: &Path) -> bool {
    let Ok(entries) = fs::read_dir(module_path) else {
        return false;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(ext) = path.extension() else {
            continue;
        };
        if !ext.eq_ignore_ascii_case("sh") {
            continue;
        }

        let Ok(meta) = path.metadata() else {
            continue;
        };
        if !meta.is_file() || meta.len() > MAX_SH_SCAN_BYTES {
            continue;
        }

        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };

        if contains_mount_commands(&content) {
            return true;
        }
    }

    false
}

fn contains_mount_commands(content: &str) -> bool {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let first_word = trimmed
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_start_matches(['\\', '`']);

        match first_word {
            "mount" | "mkdir" | "touch" => return true,
            "busybox" => {
                let rest = &trimmed[first_word.len()..].trim_start();
                let sub_cmd = rest.split_whitespace().next().unwrap_or("");
                if matches!(sub_cmd, "mount" | "mkdir" | "touch") {
                    return true;
                }
            }
            _ => {}
        }

        if first_word.contains("mount") || first_word.contains("bind") {
            return true;
        }
    }
    false
}

pub fn apply_modules_payload(
    config_path: &Path,
    modules: &[ModuleApplyEntry],
) -> Result<ModulesApplyPayload> {
    let mut config = Config::load_optional_from_file(config_path)?;

    for module in modules {
        utils::validation::validate_module_id(&module.id)?;
        if let Some(ref sp) = module.source_path {
            let canonical_sp = sp
                .canonicalize()
                .with_context(|| format!("failed to canonicalize source_path {}", sp.display()))?;
            let canonical_moduledir = config
                .moduledir
                .canonicalize()
                .unwrap_or_else(|_| config.moduledir.clone());
            if !canonical_sp.starts_with(&canonical_moduledir) {
                bail!(
                    "source_path '{}' is outside moduledir '{}'",
                    sp.display(),
                    config.moduledir.display()
                );
            }
        }
        let module_path = module
            .source_path
            .clone()
            .unwrap_or_else(|| config.moduledir.join(&module.id));
        let disable_path = module_path.join(defs::DISABLE_FILE_NAME);

        if module.enabled == Some(false) {
            utils::remove_dir_entries_case_insensitive(&module_path, defs::DISABLE_FILE_NAME)?;
            fs::write(&disable_path, b"").with_context(|| {
                format!("failed to create disable marker {}", disable_path.display())
            })?;
        } else if module.enabled == Some(true) {
            utils::remove_dir_entries_case_insensitive(&module_path, defs::DISABLE_FILE_NAME)
                .with_context(|| {
                    format!("failed to remove disable marker {}", disable_path.display())
                })?;
        }

        config.rules.insert(module.id.clone(), module.rules.clone());
    }

    config.save_to_file(config_path)?;
    Ok(ModulesApplyPayload {
        updated: modules.len(),
    })
}

pub fn build_version_payload() -> VersionPayload {
    let metadata = read_module_metadata(Path::new(defs::HYBRID_MOUNT_MODULE_DIR), "hybrid_mount");
    VersionPayload {
        version: if metadata.version == "unknown" {
            env!("CARGO_PKG_VERSION").to_string()
        } else {
            metadata.version
        },
    }
}

struct RuntimeModuleIndex<'a> {
    overlay: HashSet<&'a str>,
    magic: HashSet<&'a str>,
    kasumi: HashSet<&'a str>,
    skipped: HashSet<&'a str>,
    blacklisted: HashSet<&'a str>,
    mount_errors: HashSet<&'a str>,
    mount_error_reasons: &'a std::collections::BTreeMap<String, String>,
}

impl<'a> RuntimeModuleIndex<'a> {
    fn new(state: &'a RuntimeState) -> Self {
        Self {
            overlay: state.overlay_modules.iter().map(String::as_str).collect(),
            magic: state.magic_modules.iter().map(String::as_str).collect(),
            kasumi: state.kasumi_modules.iter().map(String::as_str).collect(),
            skipped: state
                .skip_mount_modules
                .iter()
                .map(String::as_str)
                .collect(),
            blacklisted: state
                .blacklisted_modules
                .iter()
                .map(String::as_str)
                .collect(),
            mount_errors: state
                .mount_error_modules
                .iter()
                .map(String::as_str)
                .collect(),
            mount_error_reasons: &state.mount_error_reasons,
        }
    }

    fn mode(&self, module_id: &str) -> Option<MountMode> {
        [
            (&self.overlay, MountMode::Overlay),
            (&self.magic, MountMode::Magic),
            (&self.kasumi, MountMode::Kasumi),
        ]
        .into_iter()
        .find(|(set, _)| set.contains(module_id))
        .map(|(_, mode)| mode)
    }

    fn enabled(&self, module_id: &str) -> bool {
        !self.skipped.contains(module_id) && !self.blacklisted.contains(module_id)
    }

    fn is_blacklisted(&self, module_id: &str) -> bool {
        self.blacklisted.contains(module_id)
    }

    fn mount_error_reason(&self, module_id: &str) -> Option<String> {
        if !self.mount_errors.contains(module_id) {
            return None;
        }
        Some(
            self.mount_error_reasons
                .get(module_id)
                .cloned()
                .unwrap_or_else(|| "mount error recorded".to_string()),
        )
    }
}

fn read_module_metadata(module_path: &Path, module_id: &str) -> ModuleMetadata {
    let prop_path = module_path.join("module.prop");
    let Ok(metadata) = fs::symlink_metadata(&prop_path) else {
        return default_module_metadata(module_id);
    };
    if !metadata.file_type().is_file() {
        return default_module_metadata(module_id);
    }
    if metadata.len() > MAX_MODULE_PROP_BYTES {
        crate::scoped_log!(
            warn,
            "api:modules",
            "metadata fallback: module={}, path={}, reason=module_prop_too_large, bytes={}, max_bytes={}",
            module_id,
            prop_path.display(),
            metadata.len(),
            MAX_MODULE_PROP_BYTES
        );
        return default_module_metadata(module_id);
    }

    let raw = match read_module_prop_limited(&prop_path) {
        Ok(raw) => raw,
        Err(err) => {
            crate::scoped_log!(
                warn,
                "api:modules",
                "metadata fallback: module={}, path={}, reason=read_failed, error={}",
                module_id,
                prop_path.display(),
                err
            );
            return default_module_metadata(module_id);
        }
    };

    let mut metadata = default_module_metadata(module_id);
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "name" if !value.is_empty() => metadata.name = value.to_string(),
            "version" if !value.is_empty() => metadata.version = value.to_string(),
            "author" if !value.is_empty() => metadata.author = value.to_string(),
            "description" if !value.is_empty() => metadata.description = value.to_string(),
            _ => {}
        }
    }

    metadata
}

fn read_module_prop_limited(prop_path: &Path) -> io::Result<String> {
    let file = fs::File::open(prop_path)?;
    let mut reader = file.take(MAX_MODULE_PROP_BYTES);
    let mut raw = String::new();
    reader.read_to_string(&mut raw)?;
    Ok(raw)
}

fn default_module_metadata(module_id: &str) -> ModuleMetadata {
    ModuleMetadata {
        name: module_id.to_string(),
        version: "unknown".to_string(),
        author: "unknown".to_string(),
        description: "No description".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use super::*;
    use crate::{
        conf::schema::Config,
        core::runtime_state::RuntimeState,
        defs,
        domain::{DefaultMode, MountMode},
    };

    #[test]
    fn runtime_modules_payload_keeps_runtime_and_rules_without_metadata() {
        let mut config = Config {
            moduledir: PathBuf::from("/modules"),
            default_mode: DefaultMode::Magic,
            ..Default::default()
        };
        config.rules.insert(
            "alpha".to_string(),
            ModuleRules {
                default_mode: MountMode::Overlay,
                ..Default::default()
            },
        );

        let mut state = RuntimeState::default();
        state.overlay_modules = vec!["alpha".to_string()];

        let modules = build_runtime_modules_payload(&config, &state);
        assert_eq!(modules.len(), 1);

        let module = &modules[0];
        assert_eq!(module.id, "alpha");
        assert_eq!(module.mode, MountMode::Overlay);
        assert!(module.is_mounted);
        assert!(module.enabled);
        assert_eq!(module.source_path, PathBuf::from("/modules/alpha"));
        assert_eq!(module.rules.default_mode, MountMode::Overlay);
    }

    #[test]
    fn runtime_modules_payload_includes_mount_error_marker_modules() {
        let temp = tempfile::tempdir().unwrap();
        let module_dir = temp.path().join("broken");
        fs::create_dir_all(&module_dir).unwrap();
        fs::write(module_dir.join("MOUNT_ERROR"), b"").unwrap();

        let config = Config {
            moduledir: temp.path().to_path_buf(),
            default_mode: DefaultMode::Overlay,
            ..Default::default()
        };
        let state = RuntimeState::default();

        let modules = build_runtime_modules_payload(&config, &state);
        assert_eq!(modules.len(), 1);

        let module = &modules[0];
        assert_eq!(module.id, "broken");
        assert!(!module.is_mounted);
        assert!(!module.enabled);
        assert_eq!(
            module.mount_error.as_deref(),
            Some("mount_error marker present")
        );
    }

    #[test]
    fn apply_modules_payload_handles_case_insensitive_disable_marker() {
        let temp = tempfile::tempdir().unwrap();
        let config_path = temp.path().join("config.toml");
        let module_dir = temp.path().join("modules").join("broken");
        fs::create_dir_all(&module_dir).unwrap();
        fs::write(module_dir.join("DISABLE"), b"").unwrap();

        let config = Config {
            moduledir: temp.path().join("modules"),
            ..Default::default()
        };
        config.save_to_file(&config_path).unwrap();

        let payload = apply_modules_payload(
            &config_path,
            &[ModuleApplyEntry {
                id: "broken".to_string(),
                enabled: Some(false),
                source_path: Some(module_dir.clone()),
                rules: ModuleRules::default(),
            }],
        )
        .unwrap();

        assert_eq!(payload.updated, 1);
        assert!(module_dir.join(defs::DISABLE_FILE_NAME).exists());
        assert!(crate::utils::dir_contains_entry_case_insensitive(
            &module_dir,
            defs::DISABLE_FILE_NAME
        ));
    }
}
