// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::{
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};

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

fn runtime_probe() -> Result<RuntimeProbe> {
    let cache = RUNTIME_PROBE_CACHE.get_or_init(|| Mutex::new(None));
    let cached = cache
        .lock()
        .map_err(|_| anyhow::anyhow!("Kasumi runtime probe cache is poisoned"))?;
    if let Some(probe) = cached.as_ref()
        && probe.checked_at.elapsed() < RUNTIME_PROBE_TTL
    {
        return Ok(probe.clone());
    }
    drop(cached);

    let live_status = kasumi::check_status()?;
    let lkm_loaded = lkm::is_loaded()?;
    let protocol_version = if lkm_loaded {
        Some(kasumi::get_protocol_version()?)
    } else {
        None
    };
    let (feature_bits, hooks, rule_count) = if live_status == KasumiStatus::Available {
        (
            Some(kasumi::get_features()?),
            hook_lines()?,
            api::parse_kasumi_rule_listing(&kasumi::list_rules()?)?.len(),
        )
    } else {
        (None, Vec::new(), 0)
    };

    let probe = RuntimeProbe {
        checked_at: Instant::now(),
        live_status,
        lkm_loaded,
        protocol_version,
        feature_bits,
        hooks,
        rule_count,
        kernel_supported: kasumi::kernel_is_supported()?,
        current_kmi: lkm::current_kmi()?,
    };
    *cache
        .lock()
        .map_err(|_| anyhow::anyhow!("Kasumi runtime probe cache is poisoned"))? =
        Some(probe.clone());
    Ok(probe)
}

pub fn invalidate_runtime_caches() -> Result<()> {
    kasumi::invalidate_status_cache()?;
    if let Some(cache) = RUNTIME_PROBE_CACHE.get() {
        *cache
            .lock()
            .map_err(|_| anyhow::anyhow!("Kasumi runtime probe cache is poisoned"))? = None;
    }
    Ok(())
}

pub fn can_operate(_config: &config::Config) -> Result<bool> {
    kasumi::can_operate()
}

pub fn require_live(config: &config::Config, description: &str) -> Result<()> {
    if can_operate(config)? {
        return Ok(());
    }

    bail!(
        "Kasumi is not available for {} (status={})",
        description,
        kasumi::status_name(kasumi::check_status()?)
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

pub fn collect_runtime_info(config: &config::Config) -> Result<KasumiRuntimeInfo> {
    let probe = runtime_probe().context("Failed to probe Kasumi runtime")?;
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

    Ok(KasumiRuntimeInfo {
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
        user_hide_rule_count: user_hide_rules::user_hide_rule_count()?,
        mirror_path: config.kasumi.mirror_path.clone(),
    })
}
