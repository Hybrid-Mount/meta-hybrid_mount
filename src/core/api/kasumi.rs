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
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use serde::Serialize;

use crate::{
    conf::config::Config,
    core::runtime_state::RuntimeState,
    defs,
    sys::{
        kasumi::{self, KasumiStatus},
        lkm::{self, LkmStatus},
    },
};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct KasumiRuleEntry {
    #[serde(rename = "type")]
    pub rule_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_type: Option<i32>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FeatureInfo {
    pub bitmask: i32,
    pub names: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct LkmPayload {
    pub loaded: bool,
    pub module_name: Option<String>,
    pub autoload: bool,
    pub kmi_override: String,
    pub current_kmi: String,
    pub search_dir: PathBuf,
    pub module_file: Option<PathBuf>,
    pub last_error: Option<String>,
}

impl From<LkmStatus> for LkmPayload {
    fn from(status: LkmStatus) -> Self {
        Self {
            loaded: status.loaded,
            module_name: status.module_name,
            autoload: status.autoload,
            kmi_override: status.kmi_override,
            current_kmi: status.current_kmi,
            search_dir: status.search_dir,
            module_file: status.module_file,
            last_error: lkm::last_error(),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct KasumiVersionPayload {
    pub protocol_version: i32,
    pub kernel_version: i32,
    pub kasumi_available: bool,
    pub protocol_mismatch: bool,
    pub mismatch_message: Option<String>,
    pub active_modules: Vec<String>,
    pub mount_base: PathBuf,
    pub mirror_path: PathBuf,
}

pub fn parse_kasumi_rule_listing(listing: &str) -> Vec<KasumiRuleEntry> {
    let mut rules = Vec::new();

    for raw_line in listing.lines() {
        let line = raw_line.trim();
        if line.is_empty()
            || line.starts_with("Kasumi Protocol:")
            || line.starts_with("Kasumi Enabled:")
        {
            continue;
        }

        let mut parts = line.split_whitespace();
        let Some(kind_raw) = parts.next() else {
            continue;
        };
        let rule_type = kind_raw.to_uppercase();

        match rule_type.as_str() {
            "ADD" => {
                let target = parts.next().map(ToString::to_string);
                let source = parts.next().map(ToString::to_string);
                let file_type = parts.next().and_then(|value| value.parse::<i32>().ok());
                rules.push(KasumiRuleEntry {
                    rule_type,
                    target,
                    source,
                    path: None,
                    args: None,
                    file_type,
                });
            }
            "MERGE" => {
                let target = parts.next().map(ToString::to_string);
                let source = parts.next().map(ToString::to_string);
                rules.push(KasumiRuleEntry {
                    rule_type,
                    target,
                    source,
                    path: None,
                    args: None,
                    file_type: None,
                });
            }
            "HIDE" | "INJECT" => {
                rules.push(KasumiRuleEntry {
                    rule_type,
                    target: None,
                    source: None,
                    path: parts.next().map(ToString::to_string),
                    args: None,
                    file_type: None,
                });
            }
            _ => {
                let args = parts.collect::<Vec<_>>().join(" ");
                rules.push(KasumiRuleEntry {
                    rule_type,
                    target: None,
                    source: None,
                    path: None,
                    args: (!args.is_empty()).then_some(args),
                    file_type: None,
                });
            }
        }
    }

    rules
}

pub fn build_features_payload() -> FeatureInfo {
    let bits = kasumi::get_features().unwrap_or_default();
    FeatureInfo {
        bitmask: bits,
        names: kasumi::feature_names(bits),
    }
}

pub fn build_lkm_payload(config: &Config) -> LkmPayload {
    let status = lkm::status(&config.kasumi);
    LkmPayload::from(status)
}

pub fn build_kasumi_version_payload(config: &Config, state: &RuntimeState) -> KasumiVersionPayload {
    if !config.kasumi.enabled {
        return KasumiVersionPayload {
            protocol_version: kasumi::KSM_PROTOCOL_VERSION,
            kernel_version: 0,
            kasumi_available: false,
            protocol_mismatch: false,
            mismatch_message: None,
            active_modules: Vec::new(),
            mount_base: state.mount_point.clone(),
            mirror_path: config.kasumi.mirror_path.clone(),
        };
    }

    let status = kasumi::check_status();
    let kernel_version = kasumi::get_protocol_version().ok();
    let active_rules = kasumi::list_rules().unwrap_or_default();
    let parsed_rules = parse_kasumi_rule_listing(&active_rules);
    let active_modules = if !state.kasumi_modules.is_empty() {
        let mut modules = state.kasumi_modules.clone();
        modules.sort();
        modules.dedup();
        modules
    } else {
        extract_active_module_ids(&parsed_rules, &config.kasumi.mirror_path)
    };

    let mismatch = kernel_version.is_some_and(|version| version != kasumi::KSM_PROTOCOL_VERSION);

    KasumiVersionPayload {
        protocol_version: kasumi::KSM_PROTOCOL_VERSION,
        kernel_version: kernel_version.unwrap_or_default(),
        kasumi_available: status == KasumiStatus::Available,
        protocol_mismatch: mismatch,
        mismatch_message: mismatch_message(status, kernel_version),
        active_modules,
        mount_base: state.mount_point.clone(),
        mirror_path: config.kasumi.mirror_path.clone(),
    }
}

fn mismatch_message(status: KasumiStatus, kernel_version: Option<i32>) -> Option<String> {
    match status {
        KasumiStatus::KernelNotSupported => Some(format!(
            "kernel protocol {} is not compatible with userspace api{}",
            kernel_version.unwrap_or_default(),
            kasumi::KSM_PROTOCOL_VERSION
        )),
        KasumiStatus::ModuleTooOld => Some(format!(
            "kernel protocol {} is newer than userspace api{}",
            kernel_version.unwrap_or_default(),
            kasumi::KSM_PROTOCOL_VERSION
        )),
        KasumiStatus::Available => kernel_version
            .filter(|version| *version != kasumi::KSM_PROTOCOL_VERSION)
            .map(|version| {
                format!(
                    "protocol mismatch: userspace api{}, kernel api{}",
                    kasumi::KSM_PROTOCOL_VERSION,
                    version
                )
            }),
        KasumiStatus::NotPresent => None,
    }
}

fn extract_active_module_ids(rules: &[KasumiRuleEntry], mirror_path: &Path) -> Vec<String> {
    let mut modules = BTreeSet::new();

    for rule in rules {
        let Some(source) = rule.source.as_deref() else {
            continue;
        };

        if let Some(module_id) = extract_module_id_from_source(source, mirror_path) {
            modules.insert(module_id);
        }
    }

    modules.into_iter().collect()
}

fn extract_module_id_from_source(source: &str, mirror_path: &Path) -> Option<String> {
    let module_root = format!("{}/", defs::MODULES_DIR.trim_end_matches('/'));
    if let Some(rest) = source.strip_prefix(&module_root) {
        return rest
            .split('/')
            .next()
            .filter(|value| !value.is_empty())
            .map(ToString::to_string);
    }

    let mirror_prefix = format!(
        "{}/",
        mirror_path.display().to_string().trim_end_matches('/')
    );
    if let Some(rest) = source.strip_prefix(&mirror_prefix) {
        return rest
            .split('/')
            .next()
            .filter(|value| !value.is_empty())
            .map(ToString::to_string);
    }

    None
}
