// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::{
    conf::config::Config,
    core::runtime_state::RuntimeState,
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
    pub module_file: PathBuf,
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

pub fn parse_kasumi_rule_listing(listing: &str) -> Result<Vec<KasumiRuleEntry>> {
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
                let target = parts.next().context("Kasumi ADD rule is missing target")?;
                let source = parts.next().context("Kasumi ADD rule is missing source")?;
                let file_type = parts
                    .next()
                    .context("Kasumi ADD rule is missing file type")?
                    .parse::<i32>()
                    .context("Kasumi ADD rule has invalid file type")?;
                if parts.next().is_some() {
                    bail!("Kasumi ADD rule has unexpected trailing fields: {line}");
                }
                rules.push(KasumiRuleEntry {
                    rule_type,
                    target: Some(target.to_string()),
                    source: Some(source.to_string()),
                    path: None,
                    args: None,
                    file_type: Some(file_type),
                });
            }
            "MERGE" => {
                let target = parts
                    .next()
                    .context("Kasumi MERGE rule is missing target")?;
                let source = parts
                    .next()
                    .context("Kasumi MERGE rule is missing source")?;
                if parts.next().is_some() {
                    bail!("Kasumi MERGE rule has unexpected trailing fields: {line}");
                }
                rules.push(KasumiRuleEntry {
                    rule_type,
                    target: Some(target.to_string()),
                    source: Some(source.to_string()),
                    path: None,
                    args: None,
                    file_type: None,
                });
            }
            "HIDE" | "INJECT" => {
                let path = parts.next().context("Kasumi path rule is missing path")?;
                if parts.next().is_some() {
                    bail!("Kasumi path rule has unexpected trailing fields: {line}");
                }
                rules.push(KasumiRuleEntry {
                    rule_type,
                    target: None,
                    source: None,
                    path: Some(path.to_string()),
                    args: None,
                    file_type: None,
                });
            }
            _ => bail!("unknown Kasumi rule type in listing: {line}"),
        }
    }

    Ok(rules)
}

pub fn build_features_payload() -> anyhow::Result<FeatureInfo> {
    let bits = kasumi::get_features()?;
    Ok(FeatureInfo {
        bitmask: bits,
        names: kasumi::feature_names(bits),
    })
}

pub fn build_lkm_payload(config: &Config) -> Result<LkmPayload> {
    Ok(LkmPayload::from(lkm::status(&config.kasumi)?))
}

pub fn build_kasumi_version_payload(
    config: &Config,
    state: &RuntimeState,
) -> anyhow::Result<KasumiVersionPayload> {
    if !config.kasumi.enabled {
        anyhow::bail!("Kasumi is disabled");
    }

    let status = kasumi::check_status()?;
    let kernel_version = kasumi::get_protocol_version()?;
    let mut active_modules = state.kasumi_modules.clone();
    active_modules.sort();
    active_modules.dedup();

    let mismatch = kernel_version != kasumi::KSM_PROTOCOL_VERSION;

    Ok(KasumiVersionPayload {
        protocol_version: kasumi::KSM_PROTOCOL_VERSION,
        kernel_version,
        kasumi_available: status == KasumiStatus::Available,
        protocol_mismatch: mismatch,
        mismatch_message: mismatch_message(status, kernel_version),
        active_modules,
        mount_base: state.mount_point.clone(),
        mirror_path: config.kasumi.mirror_path.clone(),
    })
}

fn mismatch_message(status: KasumiStatus, kernel_version: i32) -> Option<String> {
    match status {
        KasumiStatus::KernelNotSupported => Some(format!(
            "kernel protocol {} is not compatible with userspace api{}",
            kernel_version,
            kasumi::KSM_PROTOCOL_VERSION
        )),
        KasumiStatus::ModuleTooOld => Some(format!(
            "kernel protocol {} is newer than userspace api{}",
            kernel_version,
            kasumi::KSM_PROTOCOL_VERSION
        )),
        KasumiStatus::Available if kernel_version != kasumi::KSM_PROTOCOL_VERSION => Some(format!(
            "protocol mismatch: userspace api{}, kernel api{}",
            kasumi::KSM_PROTOCOL_VERSION,
            kernel_version
        )),
        KasumiStatus::Available => None,
        KasumiStatus::NotPresent => None,
    }
}
