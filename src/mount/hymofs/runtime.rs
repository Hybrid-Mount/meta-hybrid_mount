// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::Path;

use anyhow::{Context, Result, bail};

use super::{
    common::{
        effective_maps_spoof_enabled, effective_mount_hide_enabled, effective_statfs_spoof_enabled,
        effective_stealth_enabled, feature_supported, has_uname_spoof_config, to_c_long, to_c_uint,
        to_c_ulong,
    },
    compile::{CompiledRules, compile_rules, log_compiled_rule_summary},
    status::{can_operate, hook_lines},
};
use crate::{
    conf::{config, schema},
    core::{inventory::Module, ops::planner::MountPlan, user_hide_rules},
    defs,
    sys::hymofs::{
        self, HYMO_FEATURE_CMDLINE_SPOOF, HYMO_FEATURE_KSTAT_SPOOF, HYMO_FEATURE_MAPS_SPOOF,
        HYMO_FEATURE_MOUNT_HIDE, HYMO_FEATURE_STATFS_SPOOF, HYMO_FEATURE_UNAME_SPOOF, HymoMapsRule,
        HymoMountHideArg, HymoSpoofKstat, HymoSpoofUname, HymoStatfsSpoofArg,
    },
};

pub(super) fn mount_mapping_requested(plan: &MountPlan) -> bool {
    !plan.hymofs_module_ids.is_empty()
}

pub(super) fn auxiliary_features_requested(config: &config::Config) -> bool {
    config.hymofs.enable_kernel_debug
        || effective_stealth_enabled(config)
        || effective_mount_hide_enabled(config)
        || effective_maps_spoof_enabled(config)
        || effective_statfs_spoof_enabled(config)
        || has_uname_spoof_config(config)
        || !config.hymofs.cmdline_value.is_empty()
        || !config.hymofs.hide_uids.is_empty()
        || !config.hymofs.kstat_rules.is_empty()
        || user_hide_rules::user_hide_rule_count() > 0
}

pub(super) fn hymofs_runtime_requested(plan: &MountPlan, config: &config::Config) -> bool {
    config.hymofs.enabled && (mount_mapping_requested(plan) || auxiliary_features_requested(config))
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
            "mount:hymofs",
            "feature skip: name={}, enabled={}, reason=unsupported",
            feature_name,
            enabled
        );
        return;
    }

    if let Err(err) = operation(enabled) {
        crate::scoped_log!(
            warn,
            "mount:hymofs",
            "feature apply failed: name={}, enabled={}, error={:#}",
            feature_name,
            enabled,
            err
        );
    }
}

fn get_features() -> Option<i32> {
    match hymofs::get_features() {
        Ok(bits) => Some(bits),
        Err(err) => {
            crate::scoped_log!(
                debug,
                "mount:hymofs",
                "feature query failed: error={:#}",
                err
            );
            None
        }
    }
}

fn log_feature_summary(features: Option<i32>) {
    if let Some(bits) = features {
        let names = hymofs::feature_names(bits);
        crate::scoped_log!(
            info,
            "mount:hymofs",
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

fn log_feature_skip(feature_name: &str, detail: &str) {
    crate::scoped_log!(
        warn,
        "mount:hymofs",
        "feature skip: name={}, reason=unsupported, detail={}",
        feature_name,
        detail
    );
}

fn sync_mount_hide_config(config: &config::Config, features: Option<i32>) -> Result<()> {
    let enabled = effective_mount_hide_enabled(config);
    if !feature_supported(features, HYMO_FEATURE_MOUNT_HIDE) {
        if enabled {
            log_feature_skip("mount_hide", "live sync requested");
        }
        return Ok(());
    }

    apply_mount_hide_from_config(config)
}

fn sync_maps_spoof_config(config: &config::Config, features: Option<i32>) -> Result<()> {
    let enabled = effective_maps_spoof_enabled(config);
    let has_rules = !config.hymofs.maps_rules.is_empty();
    if !feature_supported(features, HYMO_FEATURE_MAPS_SPOOF) {
        if enabled || has_rules {
            log_feature_skip("maps_spoof", "live sync requested");
        }
        return Ok(());
    }

    hymofs::set_maps_spoof(enabled)?;
    hymofs::clear_maps_rules()?;

    for rule in &config.hymofs.maps_rules {
        let native_rule = HymoMapsRule::new(
            to_c_ulong(rule.target_ino, "target_ino")?,
            to_c_ulong(rule.target_dev, "target_dev")?,
            to_c_ulong(rule.spoofed_ino, "spoofed_ino")?,
            to_c_ulong(rule.spoofed_dev, "spoofed_dev")?,
            &rule.spoofed_pathname,
        )?;
        hymofs::add_maps_rule(&native_rule)?;
    }

    Ok(())
}

fn sync_statfs_spoof_config(config: &config::Config, features: Option<i32>) -> Result<()> {
    let enabled = effective_statfs_spoof_enabled(config);
    if !feature_supported(features, HYMO_FEATURE_STATFS_SPOOF) {
        if enabled {
            log_feature_skip("statfs_spoof", "live sync requested");
        }
        return Ok(());
    }

    apply_statfs_spoof_from_config(config)
}

fn sync_uname_config(config: &config::Config, features: Option<i32>) -> Result<()> {
    let configured = has_uname_spoof_config(config);
    if !feature_supported(features, HYMO_FEATURE_UNAME_SPOOF) {
        if configured {
            log_feature_skip("uname_spoof", "live sync requested");
        }
        return Ok(());
    }

    if configured {
        apply_uname_from_config(config)
    } else {
        hymofs::set_uname(&HymoSpoofUname::default())
    }
}

fn sync_cmdline_config(config: &config::Config, features: Option<i32>) -> Result<()> {
    if !feature_supported(features, HYMO_FEATURE_CMDLINE_SPOOF) {
        if !config.hymofs.cmdline_value.is_empty() {
            log_feature_skip("cmdline_spoof", "live sync requested");
        }
        return Ok(());
    }

    if config.hymofs.cmdline_value.is_empty() {
        crate::scoped_log!(debug, "mount:hymofs", "cmdline sync: action=clear");
    }

    hymofs::set_cmdline_str(&config.hymofs.cmdline_value)
}

fn sync_kstat_rules(config: &config::Config, features: Option<i32>) -> Result<()> {
    if config.hymofs.kstat_rules.is_empty() {
        return Ok(());
    }

    if !feature_supported(features, HYMO_FEATURE_KSTAT_SPOOF) {
        log_feature_skip("kstat_rules", "live sync requested");
        return Ok(());
    }

    for rule in &config.hymofs.kstat_rules {
        apply_kstat_rule(rule)?;
    }

    Ok(())
}

fn apply_runtime_switches(
    config: &config::Config,
    runtime_requested: bool,
    features: Option<i32>,
) -> Result<()> {
    if !runtime_requested {
        return Ok(());
    }

    if config.hymofs.enable_kernel_debug {
        hymofs::set_debug(true)?;
    }

    if effective_stealth_enabled(config) {
        hymofs::set_stealth(true)?;
    }

    let mount_hide_enabled = effective_mount_hide_enabled(config);
    if mount_hide_enabled {
        if feature_supported(features, HYMO_FEATURE_MOUNT_HIDE) {
            if let Err(err) = apply_mount_hide_from_config(config) {
                crate::scoped_log!(
                    warn,
                    "mount:hymofs",
                    "feature apply failed: name=mount_hide, enabled=true, error={:#}",
                    err
                );
            }
        } else {
            crate::scoped_log!(
                warn,
                "mount:hymofs",
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
            HYMO_FEATURE_MAPS_SPOOF,
            hymofs::set_maps_spoof,
        );
    }

    let statfs_spoof_enabled = effective_statfs_spoof_enabled(config);
    if statfs_spoof_enabled {
        if feature_supported(features, HYMO_FEATURE_STATFS_SPOOF) {
            if let Err(err) = apply_statfs_spoof_from_config(config) {
                crate::scoped_log!(
                    warn,
                    "mount:hymofs",
                    "feature apply failed: name=statfs_spoof, enabled=true, error={:#}",
                    err
                );
            }
        } else {
            crate::scoped_log!(
                warn,
                "mount:hymofs",
                "feature skip: name=statfs_spoof, enabled=true, reason=unsupported"
            );
        }
    }

    Ok(())
}

pub fn apply_mount_hide_from_config(config: &config::Config) -> Result<()> {
    let enabled = effective_mount_hide_enabled(config);

    if enabled && !config.hymofs.mount_hide.path_pattern.as_os_str().is_empty() {
        let arg =
            HymoMountHideArg::new(true, Some(config.hymofs.mount_hide.path_pattern.as_path()))?;
        hymofs::set_mount_hide_config(&arg)
    } else {
        hymofs::set_mount_hide(enabled)
    }
}

pub fn apply_statfs_spoof_from_config(config: &config::Config) -> Result<()> {
    let enabled = effective_statfs_spoof_enabled(config);

    if enabled
        && (!config.hymofs.statfs_spoof.path.as_os_str().is_empty()
            || config.hymofs.statfs_spoof.spoof_f_type != 0)
    {
        let arg = HymoStatfsSpoofArg::with_path_and_f_type(
            true,
            config.hymofs.statfs_spoof.path.as_path(),
            to_c_ulong(
                config.hymofs.statfs_spoof.spoof_f_type,
                "statfs_spoof.spoof_f_type",
            )?,
        )?;
        hymofs::set_statfs_spoof_config(&arg)
    } else {
        hymofs::set_statfs_spoof(enabled)
    }
}

pub fn apply_uname_from_config(config: &config::Config) -> Result<()> {
    let mut uname = HymoSpoofUname::default();
    if !config.hymofs.uname.sysname.is_empty() {
        uname.set_sysname(&config.hymofs.uname.sysname)?;
    }
    if !config.hymofs.uname.nodename.is_empty() {
        uname.set_nodename(&config.hymofs.uname.nodename)?;
    }
    if !config.hymofs.uname.release.is_empty() {
        uname.set_release(&config.hymofs.uname.release)?;
    }
    if !config.hymofs.uname.version.is_empty() {
        uname.set_version(&config.hymofs.uname.version)?;
    }
    if !config.hymofs.uname.machine.is_empty() {
        uname.set_machine(&config.hymofs.uname.machine)?;
    }
    if !config.hymofs.uname.domainname.is_empty() {
        uname.set_domainname(&config.hymofs.uname.domainname)?;
    }
    if !config.hymofs.uname_release.is_empty() {
        uname.set_release(&config.hymofs.uname_release)?;
    }
    if !config.hymofs.uname_version.is_empty() {
        uname.set_version(&config.hymofs.uname_version)?;
    }
    hymofs::set_uname(&uname)
}

pub fn apply_kstat_rule(rule: &schema::HymoKstatRuleConfig) -> Result<()> {
    let mut native_rule = HymoSpoofKstat::new(
        to_c_ulong(rule.target_ino, "target_ino")?,
        &rule.target_pathname,
    )?;
    native_rule.spoofed_ino = to_c_ulong(rule.spoofed_ino, "spoofed_ino")?;
    native_rule.spoofed_dev = to_c_ulong(rule.spoofed_dev, "spoofed_dev")?;
    native_rule.spoofed_nlink = to_c_uint(rule.spoofed_nlink, "spoofed_nlink");
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

    match hymofs::update_spoof_kstat(&native_rule) {
        Ok(()) => Ok(()),
        Err(update_err) => {
            crate::scoped_log!(
                debug,
                "mount:hymofs",
                "kstat update fallback to add: target={}, error={:#}",
                rule.target_pathname.display(),
                update_err
            );
            hymofs::add_spoof_kstat(&native_rule).with_context(|| {
                format!(
                    "failed to apply kstat rule for {}",
                    rule.target_pathname.display()
                )
            })
        }
    }
}

pub fn clear_runtime_best_effort() {
    let empty_uname = HymoSpoofUname::default();

    for (name, result) in [
        ("set_enabled(false)", hymofs::set_enabled(false)),
        ("clear_rules", hymofs::clear_rules()),
        ("clear_maps_rules", hymofs::clear_maps_rules()),
        ("set_uname(clear)", hymofs::set_uname(&empty_uname)),
        ("set_cmdline(clear)", hymofs::set_cmdline_str("")),
        ("set_hide_uids(clear)", hymofs::set_hide_uids(&[])),
        ("set_mount_hide(false)", hymofs::set_mount_hide(false)),
        ("set_maps_spoof(false)", hymofs::set_maps_spoof(false)),
        ("set_statfs_spoof(false)", hymofs::set_statfs_spoof(false)),
        ("set_stealth(false)", hymofs::set_stealth(false)),
        ("set_debug(false)", hymofs::set_debug(false)),
    ] {
        if let Err(err) = result {
            crate::scoped_log!(
                debug,
                "mount:hymofs",
                "disable cleanup skipped: operation={}, error={:#}",
                name,
                err
            );
        }
    }

    hymofs::invalidate_status_cache();
}

pub fn sync_runtime_config(config: &config::Config) -> Result<()> {
    let features = get_features();
    log_feature_summary(features);

    hymofs::set_mirror_path(&config.hymofs.mirror_path)?;
    hymofs::set_debug(config.hymofs.enable_kernel_debug)?;
    hymofs::set_stealth(effective_stealth_enabled(config))?;
    sync_mount_hide_config(config, features)?;
    sync_maps_spoof_config(config, features)?;
    sync_statfs_spoof_config(config, features)?;
    sync_uname_config(config, features)?;
    sync_cmdline_config(config, features)?;

    hymofs::set_hide_uids(&config.hymofs.hide_uids)?;
    sync_kstat_rules(config, features)?;

    Ok(())
}

fn apply_spoof_settings(config: &config::Config, features: Option<i32>) -> Result<()> {
    let has_uname_config = has_uname_spoof_config(config);
    if feature_supported(features, HYMO_FEATURE_UNAME_SPOOF) && has_uname_config {
        apply_uname_from_config(config)?;
    } else if has_uname_config {
        crate::scoped_log!(
            warn,
            "mount:hymofs",
            "feature skip: name=uname_spoof, reason=unsupported"
        );
    }

    if feature_supported(features, HYMO_FEATURE_CMDLINE_SPOOF)
        && !config.hymofs.cmdline_value.is_empty()
    {
        hymofs::set_cmdline_str(&config.hymofs.cmdline_value)?;
    } else if !config.hymofs.cmdline_value.is_empty() {
        crate::scoped_log!(
            warn,
            "mount:hymofs",
            "feature skip: name=cmdline_spoof, reason=unsupported"
        );
    }

    if !config.hymofs.hide_uids.is_empty()
        && let Err(err) = hymofs::set_hide_uids(&config.hymofs.hide_uids)
    {
        crate::scoped_log!(
            warn,
            "mount:hymofs",
            "hide_uids apply failed: count={}, error={:#}",
            config.hymofs.hide_uids.len(),
            err
        );
    }

    if !config.hymofs.kstat_rules.is_empty() {
        if !feature_supported(features, HYMO_FEATURE_KSTAT_SPOOF) {
            crate::scoped_log!(
                warn,
                "mount:hymofs",
                "feature skip: name=kstat_rules, count={}, reason=unsupported",
                config.hymofs.kstat_rules.len()
            );
        } else {
            for rule in &config.hymofs.kstat_rules {
                apply_kstat_rule(rule)?;
            }
        }
    }

    if !config.hymofs.maps_rules.is_empty() {
        if !feature_supported(features, HYMO_FEATURE_MAPS_SPOOF) {
            crate::scoped_log!(
                warn,
                "mount:hymofs",
                "feature skip: name=maps_rules, count={}, reason=unsupported",
                config.hymofs.maps_rules.len()
            );
        } else {
            for rule in &config.hymofs.maps_rules {
                let native_rule = HymoMapsRule::new(
                    to_c_ulong(rule.target_ino, "target_ino")?,
                    to_c_ulong(rule.target_dev, "target_dev")?,
                    to_c_ulong(rule.spoofed_ino, "spoofed_ino")?,
                    to_c_ulong(rule.spoofed_dev, "spoofed_dev")?,
                    &rule.spoofed_pathname,
                )?;
                hymofs::add_maps_rule(&native_rule)?;
            }
        }
    }

    Ok(())
}

pub fn reset_runtime(config: &config::Config) -> Result<bool> {
    if !config.hymofs.enabled {
        return Ok(false);
    }

    let available = can_operate(config);
    if !available {
        return Ok(false);
    }

    crate::scoped_log!(
        info,
        "mount:hymofs",
        "reset: mirror_path={}",
        config.hymofs.mirror_path.display()
    );

    hymofs::set_mirror_path(&config.hymofs.mirror_path)?;
    hymofs::set_enabled(false)?;
    hymofs::clear_rules()?;
    if let Err(err) = hymofs::clear_maps_rules() {
        crate::scoped_log!(
            debug,
            "mount:hymofs",
            "maps rule clear skipped: error={:#}",
            err
        );
    }

    let features = get_features();
    log_feature_summary(features);

    if config.hymofs.mirror_path != Path::new(defs::HYMOFS_MIRROR_DIR) {
        crate::scoped_log!(
            info,
            "mount:hymofs",
            "custom mirror active: path={}",
            config.hymofs.mirror_path.display()
        );
    }

    Ok(true)
}

pub fn apply(plan: &mut MountPlan, modules: &[Module], config: &config::Config) -> Result<bool> {
    if !config.hymofs.enabled {
        return Ok(false);
    }

    let runtime_requested = hymofs_runtime_requested(plan, config);
    let available = can_operate(config);
    if !available {
        if mount_mapping_requested(plan) {
            bail!("HymoFS became unavailable before rule application");
        }
        return Ok(false);
    }

    crate::scoped_log!(
        info,
        "mount:hymofs",
        "apply: mirror_path={}, hymofs_modules={}, runtime_requested={}",
        config.hymofs.mirror_path.display(),
        plan.hymofs_module_ids.len(),
        runtime_requested
    );

    let compiled = if mount_mapping_requested(plan) {
        compile_rules(modules, plan, config)?
    } else {
        CompiledRules::default()
    };
    let user_hide_paths = user_hide_rules::load_user_hide_rules()?;
    log_compiled_rule_summary(&compiled, &user_hide_paths);

    plan.hymofs_add_rules = compiled.add_rules;
    plan.hymofs_merge_rules = compiled.merge_rules;
    plan.hymofs_hide_rules = compiled.hide_rules;

    hymofs::set_mirror_path(&config.hymofs.mirror_path)?;
    hymofs::clear_rules()?;
    if let Err(err) = hymofs::clear_maps_rules() {
        crate::scoped_log!(
            debug,
            "mount:hymofs",
            "maps rule clear skipped: error={:#}",
            err
        );
    }

    let features = get_features();
    log_feature_summary(features);
    if !runtime_requested {
        hymofs::set_enabled(false)?;
        crate::scoped_log!(
            info,
            "mount:hymofs",
            "apply skipped: reason=no_runtime_request"
        );
        return Ok(false);
    }

    apply_runtime_switches(config, true, features)?;
    apply_spoof_settings(config, features)?;

    for rule in &plan.hymofs_add_rules {
        hymofs::add_rule(Path::new(&rule.target), &rule.source, rule.file_type)?;
    }
    for rule in &plan.hymofs_merge_rules {
        hymofs::add_merge_rule(Path::new(&rule.target), &rule.source)?;
    }
    for path in &plan.hymofs_hide_rules {
        hymofs::hide_path(Path::new(path))?;
    }

    let (user_hide_applied, user_hide_failed) =
        user_hide_rules::apply_user_hide_rules_from_paths(&user_hide_paths)?;

    hymofs::set_enabled(runtime_requested)?;
    if runtime_requested && let Err(err) = hymofs::fix_mounts() {
        crate::scoped_log!(debug, "mount:hymofs", "fix_mounts skipped: error={:#}", err);
    }

    crate::scoped_log!(
        info,
        "mount:hymofs",
        "apply complete: enabled={}, add_rules={}, merge_rules={}, hide_rules={}, maps_rules={}, kstat_rules={}",
        runtime_requested,
        plan.hymofs_add_rules.len(),
        plan.hymofs_merge_rules.len(),
        plan.hymofs_hide_rules.len(),
        config.hymofs.maps_rules.len(),
        config.hymofs.kstat_rules.len()
    );

    if user_hide_applied > 0 || user_hide_failed > 0 {
        crate::scoped_log!(
            info,
            "mount:hymofs",
            "user hide rules: applied={}, failed={}",
            user_hide_applied,
            user_hide_failed
        );
    }

    if runtime_requested {
        match hook_lines() {
            Ok(hooks) => crate::scoped_log!(debug, "mount:hymofs", "hooks: {}", hooks.join(",")),
            Err(err) => {
                crate::scoped_log!(debug, "mount:hymofs", "hook query skipped: error={:#}", err)
            }
        }
    }

    Ok(runtime_requested)
}
