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

use anyhow::{Result, bail};

use crate::{
    conf::config,
    core::{api, runtime_state::KasumiRuntimeInfo, user_hide_rules},
    sys::{
        kasumi::{self, KasumiStatus},
        lkm,
    },
};

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
    let live_status = kasumi::check_status();
    let lkm_loaded = lkm::is_loaded();
    let protocol_version = kasumi::get_protocol_version().ok();
    let feature_bits = kasumi::get_features().ok();
    let feature_names = feature_bits.map(kasumi::feature_names).unwrap_or_default();
    let hooks = hook_lines().unwrap_or_default();
    let rule_count = kasumi::list_rules()
        .map(|value| api::parse_kasumi_rule_listing(&value).len())
        .unwrap_or(0);
    let available = live_status == KasumiStatus::Available;
    let status = if config.kasumi.enabled {
        kasumi::status_name(live_status).to_string()
    } else if available || lkm_loaded || rule_count > 0 {
        "disabled_runtime_present".to_string()
    } else {
        "disabled".to_string()
    };

    KasumiRuntimeInfo {
        status,
        available,
        kernel_supported: kasumi::kernel_is_supported(),
        lkm_loaded,
        lkm_autoload: config.kasumi.lkm_autoload,
        lkm_kmi_override: config.kasumi.lkm_kmi_override.clone(),
        lkm_current_kmi: lkm::current_kmi(),
        lkm_dir: config.kasumi.lkm_dir.clone(),
        protocol_version,
        feature_bits,
        feature_names,
        hooks,
        rule_count,
        user_hide_rule_count: user_hide_rules::user_hide_rule_count(),
        mirror_path: config.kasumi.mirror_path.clone(),
    }
}
