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

use std::path::Path;

use anyhow::{Context, Result, bail};

use super::{
    common::{
        effective_maps_spoof_enabled, effective_mount_hide_enabled, effective_selinux_fix_enabled,
        effective_statfs_spoof_enabled, effective_stealth_enabled, feature_supported,
        has_uname_spoof_config, to_c_long, to_c_ulong,
    },
    compile::{CompiledRules, compile_rules, log_compiled_rule_summary},
    status::{can_operate, hook_lines},
};
use crate::{
    conf::{
        config,
        schema::{self, KasumiUnameMode},
    },
    core::{inventory::Module, ops::plan::MountPlan, user_hide_rules},
    defs,
    sys::{
        kasumi::{
            self, KSM_FEATURE_CMDLINE_SPOOF, KSM_FEATURE_KSTAT_SPOOF, KSM_FEATURE_MAPS_SPOOF,
            KSM_FEATURE_MOUNT_HIDE, KSM_FEATURE_SELINUX_FIX, KSM_FEATURE_STATFS_SPOOF,
            KSM_FEATURE_UNAME_SPOOF, KasumiMapsRule, KasumiMountHideArg, KasumiSpoofKstat,
            KasumiSpoofUname, KasumiStatfsSpoofArg,
        },
        lkm,
    },
};

fn mount_mapping_requested(plan: &MountPlan) -> bool {
    !plan.kasumi_module_ids.is_empty()
}

fn auxiliary_features_requested(config: &config::Config) -> bool {
    config.kasumi.enable_kernel_debug
        || effective_stealth_enabled(config)
        || effective_mount_hide_enabled(config)
        || effective_maps_spoof_enabled(config)
        || effective_statfs_spoof_enabled(config)
        || effective_selinux_fix_enabled(config)
        || has_uname_spoof_config(config)
        || !config.kasumi.cmdline_value.is_empty()
        || !config.kasumi.hide_uids.is_empty()
        || !config.kasumi.kstat_rules.is_empty()
        || user_hide_rules::user_hide_rule_count() > 0
}

fn kasumi_runtime_requested(plan: &MountPlan, config: &config::Config) -> bool {
    config.kasumi.enabled && (mount_mapping_requested(plan) || auxiliary_features_requested(config))
}

fn apply_feature_toggle<F>(
    feature_name: &str,
    enabled: bool,
    features: Option<i32>,
    required_feature: i32,
    operation: F,
) where
    F: FnOnce(bool) -> Result<()>,
{
    let supported = feature_supported(features, required_feature);

    if !supported {
        crate::scoped_log!(
            warn,
            "mount:kasumi",
            "feature skip: name={}, enabled={}, reason=unsupported",
            feature_name,
            enabled
        );
        return;
    }

    if let Err(err) = operation(enabled) {
        crate::scoped_log!(
            warn,
            "mount:kasumi",
            "feature apply failed: name={}, enabled={}, error={:#}",
            feature_name,
            enabled,
            err
        );
    }
}

fn get_features() -> Option<i32> {
    match kasumi::get_features() {
        Ok(bits) => Some(bits),
        Err(err) => {
            crate::scoped_log!(
                debug,
                "mount:kasumi",
                "feature query failed: error={:#}",
                err
            );
            None
        }
    }
}

fn log_feature_summary(features: Option<i32>) {
    if let Some(bits) = features {
        let names = kasumi::feature_names(bits);
        crate::scoped_log!(
            info,
            "mount:kasumi",
            "features: bits={}, names={}",
            bits,
            if names.is_empty() {
                "none".to_string()
            } else {
                names.join(",")
            }
        );
    }
}

fn apply_runtime_switches(
    config: &config::Config,
    runtime_requested: bool,
    features: Option<i32>,
) -> Result<()> {
    if !runtime_requested {
        return Ok(());
    }

    if config.kasumi.enable_kernel_debug {
        kasumi::set_debug(true)?;
    }

    if effective_stealth_enabled(config) {
        kasumi::set_stealth(true)?;
    }

    let mount_hide_enabled = effective_mount_hide_enabled(config);
    if mount_hide_enabled {
        if feature_supported(features, KSM_FEATURE_MOUNT_HIDE) {
            if let Err(err) = apply_mount_hide_from_config(config) {
                crate::scoped_log!(
                    warn,
                    "mount:kasumi",
                    "feature apply failed: name=mount_hide, enabled=true, error={:#}",
                    err
                );
            }
        } else {
            crate::scoped_log!(
                warn,
                "mount:kasumi",
                "feature skip: name=mount_hide, enabled=true, reason=unsupported"
            );
        }
    }

    let maps_spoof_enabled = effective_maps_spoof_enabled(config);
    if maps_spoof_enabled {
        apply_feature_toggle(
            "maps_spoof",
            true,
            features,
            KSM_FEATURE_MAPS_SPOOF,
            kasumi::set_maps_spoof,
        );
    }

    let statfs_spoof_enabled = effective_statfs_spoof_enabled(config);
    if statfs_spoof_enabled {
        if feature_supported(features, KSM_FEATURE_STATFS_SPOOF) {
            if let Err(err) = apply_statfs_spoof_from_config(config) {
                crate::scoped_log!(
                    warn,
                    "mount:kasumi",
                    "feature apply failed: name=statfs_spoof, enabled=true, error={:#}",
                    err
                );
            }
        } else {
            crate::scoped_log!(
                warn,
                "mount:kasumi",
                "feature skip: name=statfs_spoof, enabled=true, reason=unsupported"
            );
        }
    }

    let selinux_fix_enabled = effective_selinux_fix_enabled(config);
    if feature_supported(features, KSM_FEATURE_SELINUX_FIX) {
        if let Err(err) = kasumi::set_selinux_fix(selinux_fix_enabled) {
            crate::scoped_log!(
                warn,
                "mount:kasumi",
                "feature apply failed: name=selinux_fix, enabled={}, error={:#}",
                selinux_fix_enabled,
                err
            );
        }
    } else if selinux_fix_enabled {
        crate::scoped_log!(
            warn,
            "mount:kasumi",
            "feature skip: name=selinux_fix, enabled=true, reason=unsupported"
        );
    }

    Ok(())
}

pub fn apply_mount_hide_from_config(config: &config::Config) -> Result<()> {
    let enabled = effective_mount_hide_enabled(config);

    if enabled && !config.kasumi.mount_hide.path_pattern.as_os_str().is_empty() {
        let arg =
            KasumiMountHideArg::new(true, Some(config.kasumi.mount_hide.path_pattern.as_path()))?;
        kasumi::set_mount_hide_config(&arg)
    } else {
        kasumi::set_mount_hide(enabled)
    }
}

pub fn apply_statfs_spoof_from_config(config: &config::Config) -> Result<()> {
    let enabled = effective_statfs_spoof_enabled(config);

    if enabled
        && (!config.kasumi.statfs_spoof.path.as_os_str().is_empty()
            || config.kasumi.statfs_spoof.spoof_f_type != 0)
    {
        let arg = KasumiStatfsSpoofArg::with_path_and_f_type(
            true,
            config.kasumi.statfs_spoof.path.as_path(),
            to_c_ulong(
                config.kasumi.statfs_spoof.spoof_f_type,
                "statfs_spoof.spoof_f_type",
            )?,
        )?;
        kasumi::set_statfs_spoof_config(&arg)
    } else {
        kasumi::set_statfs_spoof(enabled)
    }
}

pub fn apply_uname_from_config(config: &config::Config) -> Result<()> {
    let mut uname = KasumiSpoofUname::default();
    if !config.kasumi.uname.sysname.is_empty() {
        uname.set_sysname(&config.kasumi.uname.sysname)?;
    }
    if !config.kasumi.uname.nodename.is_empty() {
        uname.set_nodename(&config.kasumi.uname.nodename)?;
    }
    if !config.kasumi.uname.release.is_empty() {
        uname.set_release(&config.kasumi.uname.release)?;
    }
    if !config.kasumi.uname.version.is_empty() {
        uname.set_version(&config.kasumi.uname.version)?;
    }
    if !config.kasumi.uname.machine.is_empty() {
        uname.set_machine(&config.kasumi.uname.machine)?;
    }
    if !config.kasumi.uname.domainname.is_empty() {
        uname.set_domainname(&config.kasumi.uname.domainname)?;
    }
    match config.kasumi.uname_mode {
        KasumiUnameMode::Scoped => kasumi::set_uname(&uname),
        KasumiUnameMode::Global => kasumi::set_uname_global(&uname),
    }
}

pub fn apply_kstat_rule(rule: &schema::KasumiKstatRuleConfig) -> Result<()> {
    let mut native_rule = KasumiSpoofKstat::new(
        to_c_ulong(rule.target_ino, "target_ino")?,
        &rule.target_pathname,
    )?;
    native_rule.spoofed_ino = to_c_ulong(rule.spoofed_ino, "spoofed_ino")?;
    native_rule.spoofed_dev = to_c_ulong(rule.spoofed_dev, "spoofed_dev")?;
    native_rule.spoofed_nlink = rule.spoofed_nlink;
    native_rule.spoofed_size = rule.spoofed_size;
    native_rule.spoofed_atime_sec = to_c_long(rule.spoofed_atime_sec, "spoofed_atime_sec")?;
    native_rule.spoofed_atime_nsec = to_c_long(rule.spoofed_atime_nsec, "spoofed_atime_nsec")?;
    native_rule.spoofed_mtime_sec = to_c_long(rule.spoofed_mtime_sec, "spoofed_mtime_sec")?;
    native_rule.spoofed_mtime_nsec = to_c_long(rule.spoofed_mtime_nsec, "spoofed_mtime_nsec")?;
    native_rule.spoofed_ctime_sec = to_c_long(rule.spoofed_ctime_sec, "spoofed_ctime_sec")?;
    native_rule.spoofed_ctime_nsec = to_c_long(rule.spoofed_ctime_nsec, "spoofed_ctime_nsec")?;
    native_rule.spoofed_blksize = to_c_ulong(rule.spoofed_blksize, "spoofed_blksize")?;
    native_rule.spoofed_blocks = rule.spoofed_blocks;
    native_rule.is_static = if rule.is_static { 1 } else { 0 };

    match kasumi::update_spoof_kstat(&native_rule) {
        Ok(()) => Ok(()),
        Err(update_err) => {
            crate::scoped_log!(
                debug,
                "mount:kasumi",
                "kstat update fallback to add: target={}, error={:#}",
                rule.target_pathname.display(),
                update_err
            );
            kasumi::add_spoof_kstat(&native_rule).with_context(|| {
                format!(
                    "failed to apply kstat rule for {}",
                    rule.target_pathname.display()
                )
            })
        }
    }
}

fn apply_spoof_settings(config: &config::Config, features: Option<i32>) -> Result<()> {
    let has_uname_config = has_uname_spoof_config(config);
    let should_apply_uname =
        has_uname_config || matches!(config.kasumi.uname_mode, KasumiUnameMode::Global);
    if feature_supported(features, KSM_FEATURE_UNAME_SPOOF) && should_apply_uname {
        apply_uname_from_config(config)?;
    } else if should_apply_uname {
        crate::scoped_log!(
            warn,
            "mount:kasumi",
            "feature skip: name=uname_spoof, reason=unsupported"
        );
    }

    if feature_supported(features, KSM_FEATURE_CMDLINE_SPOOF)
        && !config.kasumi.cmdline_value.is_empty()
    {
        kasumi::set_cmdline_str(&config.kasumi.cmdline_value)?;
    } else if !config.kasumi.cmdline_value.is_empty() {
        crate::scoped_log!(
            warn,
            "mount:kasumi",
            "feature skip: name=cmdline_spoof, reason=unsupported"
        );
    }

    if !config.kasumi.hide_uids.is_empty()
        && let Err(err) = kasumi::set_hide_uids(&config.kasumi.hide_uids)
    {
        crate::scoped_log!(
            warn,
            "mount:kasumi",
            "hide_uids apply failed: count={}, error={:#}",
            config.kasumi.hide_uids.len(),
            err
        );
    }

    if !config.kasumi.kstat_rules.is_empty() {
        if !feature_supported(features, KSM_FEATURE_KSTAT_SPOOF) {
            crate::scoped_log!(
                warn,
                "mount:kasumi",
                "feature skip: name=kstat_rules, count={}, reason=unsupported",
                config.kasumi.kstat_rules.len()
            );
        } else {
            for rule in &config.kasumi.kstat_rules {
                apply_kstat_rule(rule)?;
            }
        }
    }

    if !config.kasumi.maps_rules.is_empty() {
        if !feature_supported(features, KSM_FEATURE_MAPS_SPOOF) {
            crate::scoped_log!(
                warn,
                "mount:kasumi",
                "feature skip: name=maps_rules, count={}, reason=unsupported",
                config.kasumi.maps_rules.len()
            );
        } else {
            for rule in &config.kasumi.maps_rules {
                let native_rule = KasumiMapsRule::new(
                    to_c_ulong(rule.target_ino, "target_ino")?,
                    to_c_ulong(rule.target_dev, "target_dev")?,
                    to_c_ulong(rule.spoofed_ino, "spoofed_ino")?,
                    to_c_ulong(rule.spoofed_dev, "spoofed_dev")?,
                    &rule.spoofed_pathname,
                )?;
                kasumi::add_maps_rule(&native_rule)?;
            }
        }
    }

    Ok(())
}

pub fn reset_runtime(config: &config::Config) -> Result<bool> {
    if !config.kasumi.enabled {
        return Ok(false);
    }

    let available = can_operate(config);
    if !available {
        return Ok(false);
    }

    crate::scoped_log!(
        info,
        "mount:kasumi",
        "reset: mirror_path={}",
        config.kasumi.mirror_path.display()
    );

    kasumi::set_mirror_path(&config.kasumi.mirror_path)?;
    kasumi::set_enabled(false)?;
    kasumi::clear_rules()?;
    if let Err(err) = kasumi::clear_maps_rules() {
        crate::scoped_log!(
            debug,
            "mount:kasumi",
            "maps rule clear skipped: error={:#}",
            err
        );
    }

    let features = get_features();
    log_feature_summary(features);

    if config.kasumi.mirror_path != Path::new(defs::KASUMI_MIRROR_DIR) {
        crate::scoped_log!(
            info,
            "mount:kasumi",
            "custom mirror active: path={}",
            config.kasumi.mirror_path.display()
        );
    }

    Ok(true)
}

pub fn apply_runtime_config(config: &config::Config) -> Result<bool> {
    // When the user enables Kasumi but the LKM hasn't been loaded yet, attempt
    // autoload so the deadlock ("switch hidden because LKM not loaded, LKM not
    // loaded because switch hidden") is broken.
    if config.kasumi.enabled && !can_operate(config) {
        let _ = lkm::autoload_if_needed(&config.kasumi);
    }

    if !can_operate(config) {
        return Ok(false);
    }

    let runtime_requested = config.kasumi.enabled && auxiliary_features_requested(config);
    let reset = reset_runtime(config)?;
    let features = get_features();
    log_feature_summary(features);

    if !runtime_requested {
        kasumi::set_enabled(false)?;
        return Ok(reset);
    }

    apply_runtime_switches(config, true, features)?;
    apply_spoof_settings(config, features)?;
    kasumi::set_enabled(true)?;
    if let Err(err) = kasumi::fix_mounts() {
        crate::scoped_log!(debug, "mount:kasumi", "fix_mounts skipped: error={:#}", err);
    }
    Ok(true)
}

pub fn apply(plan: &mut MountPlan, modules: &[Module], config: &config::Config) -> Result<bool> {
    if !config.kasumi.enabled {
        return Ok(false);
    }

    let runtime_requested = kasumi_runtime_requested(plan, config);
    let available = can_operate(config);
    if !available {
        if mount_mapping_requested(plan) {
            bail!("Kasumi became unavailable before rule application");
        }
        return Ok(false);
    }

    crate::scoped_log!(
        info,
        "mount:kasumi",
        "apply: mirror_path={}, kasumi_modules={}, runtime_requested={}",
        config.kasumi.mirror_path.display(),
        plan.kasumi_module_ids.len(),
        runtime_requested
    );

    let compiled = if mount_mapping_requested(plan) {
        compile_rules(modules, plan, config)?
    } else {
        CompiledRules::default()
    };
    let user_hide_paths = user_hide_rules::load_user_hide_rules()?;
    log_compiled_rule_summary(&compiled, &user_hide_paths);

    plan.kasumi_add_rules = compiled.add_rules;
    plan.kasumi_merge_rules = compiled.merge_rules;
    plan.kasumi_hide_rules = compiled.hide_rules;

    kasumi::set_mirror_path(&config.kasumi.mirror_path)?;
    kasumi::clear_rules()?;
    if let Err(err) = kasumi::clear_maps_rules() {
        crate::scoped_log!(
            debug,
            "mount:kasumi",
            "maps rule clear skipped: error={:#}",
            err
        );
    }

    let features = get_features();
    log_feature_summary(features);
    if !runtime_requested {
        kasumi::set_enabled(false)?;
        crate::scoped_log!(
            info,
            "mount:kasumi",
            "apply skipped: reason=no_runtime_request"
        );
        return Ok(false);
    }

    apply_runtime_switches(config, true, features)?;
    apply_spoof_settings(config, features)?;

    for rule in &plan.kasumi_add_rules {
        kasumi::add_rule(Path::new(&rule.target), &rule.source, rule.file_type)?;
    }
    for rule in &plan.kasumi_merge_rules {
        kasumi::add_merge_rule(Path::new(&rule.target), &rule.source)?;
    }
    for path in &plan.kasumi_hide_rules {
        kasumi::hide_path(Path::new(path))?;
    }

    let (user_hide_applied, user_hide_failed) =
        user_hide_rules::apply_user_hide_rules_from_paths(&user_hide_paths)?;

    kasumi::set_enabled(runtime_requested)?;
    if runtime_requested && let Err(err) = kasumi::fix_mounts() {
        crate::scoped_log!(debug, "mount:kasumi", "fix_mounts skipped: error={:#}", err);
    }

    crate::scoped_log!(
        info,
        "mount:kasumi",
        "apply complete: enabled={}, add_rules={}, merge_rules={}, hide_rules={}, maps_rules={}, kstat_rules={}",
        runtime_requested,
        plan.kasumi_add_rules.len(),
        plan.kasumi_merge_rules.len(),
        plan.kasumi_hide_rules.len(),
        config.kasumi.maps_rules.len(),
        config.kasumi.kstat_rules.len()
    );

    if user_hide_applied > 0 || user_hide_failed > 0 {
        crate::scoped_log!(
            info,
            "mount:kasumi",
            "user hide rules: applied={}, failed={}",
            user_hide_applied,
            user_hide_failed
        );
    }

    if runtime_requested {
        match hook_lines() {
            Ok(hooks) => crate::scoped_log!(debug, "mount:kasumi", "hooks: {}", hooks.join(",")),
            Err(err) => {
                crate::scoped_log!(debug, "mount:kasumi", "hook query skipped: error={:#}", err)
            }
        }
    }

    Ok(runtime_requested)
}
