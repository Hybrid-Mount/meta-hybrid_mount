// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::{
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use anyhow::{Result, bail};

use crate::{
    conf::config,
    core::{api, runtime_state::KasumiRuntimeInfo, user_hide_rules},
    sys::{
        kasumi::{self, KasumiStatus},
        lkm,
    },
};

const RUNTIME_PROBE_TTL: Duration = Duration::from_secs(3);

#[derive(Clone)]
struct RuntimeProbe {
    checked_at: Instant,
    live_status: KasumiStatus,
    lkm_loaded: bool,
    protocol_version: Option<i32>,
    feature_bits: Option<i32>,
    hooks: Vec<String>,
    rule_count: usize,
    kernel_supported: bool,
    current_kmi: String,
}

static RUNTIME_PROBE_CACHE: OnceLock<Mutex<Option<RuntimeProbe>>> = OnceLock::new();

fn runtime_probe() -> RuntimeProbe {
    let cache = RUNTIME_PROBE_CACHE.get_or_init(|| Mutex::new(None));
    if let Some(probe) = crate::utils::lock_or_recover(cache).as_ref()
        && probe.checked_at.elapsed() < RUNTIME_PROBE_TTL
    {
        return probe.clone();
    }

    let probe = RuntimeProbe {
        checked_at: Instant::now(),
        live_status: kasumi::check_status(),
        lkm_loaded: lkm::is_loaded(),
        protocol_version: kasumi::get_protocol_version().ok(),
        feature_bits: kasumi::get_features().ok(),
        hooks: hook_lines().unwrap_or_default(),
        rule_count: kasumi::list_rules()
            .map(|value| api::parse_kasumi_rule_listing(&value).len())
            .unwrap_or(0),
        kernel_supported: kasumi::kernel_is_supported(),
        current_kmi: lkm::current_kmi(),
    };
    *crate::utils::lock_or_recover(cache) = Some(probe.clone());
    probe
}

pub fn invalidate_runtime_info_cache() {
    if let Some(cache) = RUNTIME_PROBE_CACHE.get() {
        *crate::utils::lock_or_recover(cache) = None;
    }
}

pub fn can_operate(config: &config::Config) -> bool {
    let _ = config;
    kasumi::can_operate()
}

pub fn require_live(config: &config::Config, description: &str) -> Result<()> {
    if can_operate(config) {
        return Ok(());
    }

    bail!(
        "Kasumi is not available for {} (status={})",
        description,
        kasumi::status_name(kasumi::check_status())
    );
}

pub fn hook_lines() -> Result<Vec<String>> {
    Ok(kasumi::get_hooks()?
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect())
}

pub fn collect_runtime_info(config: &config::Config) -> KasumiRuntimeInfo {
    let probe = runtime_probe();
    let live_status = probe.live_status;
    let feature_bits = probe.feature_bits;
    let feature_names = feature_bits.map(kasumi::feature_names).unwrap_or_default();
    let available = live_status == KasumiStatus::Available;
    let status = if config.kasumi.enabled {
        kasumi::status_name(live_status).to_string()
    } else if available || probe.lkm_loaded || probe.rule_count > 0 {
        "disabled_runtime_present".to_string()
    } else {
        "disabled".to_string()
    };

    KasumiRuntimeInfo {
        status,
        available,
        kernel_supported: probe.kernel_supported,
        lkm_loaded: probe.lkm_loaded,
        lkm_autoload: config.kasumi.lkm_autoload,
        lkm_kmi_override: config.kasumi.lkm_kmi_override.clone(),
        lkm_current_kmi: probe.current_kmi,
        lkm_dir: config.kasumi.lkm_dir.clone(),
        protocol_version: probe.protocol_version,
        feature_bits,
        feature_names,
        hooks: probe.hooks,
        rule_count: probe.rule_count,
        user_hide_rule_count: user_hide_rules::user_hide_rule_count(),
        mirror_path: config.kasumi.mirror_path.clone(),
    }
}
