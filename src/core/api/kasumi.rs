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
    pub current_kmi: Option<String>,
    pub search_dir: PathBuf,
    pub module_file: Option<PathBuf>,
    pub available_kmis: Vec<String>,
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
            available_kmis: status.available_kmis,
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
    const STATUS_KEYS: &[&str] = &[
        "maps_spoof",
        "mount_hide",
        "selinux_fix",
        "statfs_spoof",
        "stealth",
    ];
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
            _ if STATUS_KEYS.contains(&kind_raw)
                && matches!(parts.next(), Some("enabled" | "disabled"))
                && parts.next().is_none() =>
            {
                continue;
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

#[cfg(test)]
mod tests {
    use super::parse_kasumi_rule_listing;

    #[test]
    fn rule_listing_skips_feature_status_lines() {
        let listing = "Kasumi Protocol: 16\nstealth enabled\nselinux_fix disabled\nADD /system/app /mirror/app 1\n";

        let rules = parse_kasumi_rule_listing(listing).unwrap();

        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].rule_type, "ADD");
    }

    #[test]
    fn rule_listing_rejects_unknown_non_status_lines() {
        let result = parse_kasumi_rule_listing("unknown_feature enabled\n");

        assert!(result.is_err());
    }

    #[test]
    fn rule_listing_keeps_lowercase_path_rules_named_like_status_values() {
        let rules = parse_kasumi_rule_listing("hide enabled\ninject disabled\n").unwrap();

        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].path.as_deref(), Some("enabled"));
        assert_eq!(rules[1].path.as_deref(), Some("disabled"));
    }
}
