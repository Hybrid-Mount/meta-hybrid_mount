// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    fs,
    os::unix::fs::{FileTypeExt, MetadataExt},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use serde::de::DeserializeOwned;
use serde_json::json;

use crate::{
    conf::{
        cli::Cli,
        config::{self, Config},
        loader,
        schema::{HymoKstatRuleConfig, HymoMapsRuleConfig},
    },
    core::{api, inventory::listing as modules, runtime_state::RuntimeState, user_hide_rules},
    defs,
    mount::hymofs as hymofs_mount,
    sys::{hymofs, lkm},
    utils,
};

fn decode_hex_json<T: DeserializeOwned>(payload: &str, type_name: &str) -> Result<T> {
    if !payload.len().is_multiple_of(2) {
        bail!("Invalid hex payload length for {}", type_name);
    }

    let json_bytes = (0..payload.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&payload[i..i + 2], 16))
        .collect::<Result<Vec<u8>, _>>()
        .with_context(|| format!("Failed to decode hex payload for {}", type_name))?;

    serde_json::from_slice(&json_bytes)
        .with_context(|| format!("Failed to parse {} JSON payload", type_name))
}

fn load_effective_config(cli: &Cli) -> Result<Config> {
    let mut config = loader::load_config(cli)?;
    config.merge_with_cli(
        cli.moduledir.clone(),
        cli.mountsource.clone(),
        cli.partitions.clone(),
    );
    Ok(config)
}

fn config_output_path(cli: &Cli) -> PathBuf {
    cli.config
        .clone()
        .unwrap_or_else(|| PathBuf::from(defs::CONFIG_FILE))
}

fn save_hymofs_config_for_cli(cli: &Cli, config: &Config) -> Result<PathBuf> {
    let main_path = config_output_path(cli);
    config
        .save_to_file(&main_path)
        .with_context(|| format!("Failed to save config file to {}", main_path.display()))?;
    Ok(main_path)
}

fn apply_live_if_possible<F>(config: &Config, description: &str, operation: F) -> Result<bool>
where
    F: FnOnce() -> Result<()>,
{
    if !hymofs_mount::can_operate(config) {
        crate::scoped_log!(
            warn,
            "cli:hymofs",
            "live apply skipped: operation={}, status={}",
            description,
            hymofs::status_name(hymofs::check_status())
        );
        return Ok(false);
    }

    operation()?;
    Ok(true)
}

fn require_live_hymofs(config: &Config, description: &str) -> Result<()> {
    hymofs_mount::require_live(config, description)
}

fn print_config_apply_result(path: &Path, what: &str, applied: bool) {
    if applied {
        println!("{what} saved to {} and applied to HymoFS.", path.display());
    } else {
        println!(
            "{what} saved to {}. HymoFS is not currently available, so only the config was updated.",
            path.display()
        );
    }
}

fn clear_pathbuf(path: &mut PathBuf) {
    *path = PathBuf::new();
}

fn apply_live_runtime_sync(config: &Config, description: &str) -> Result<bool> {
    apply_live_if_possible(config, description, || {
        hymofs_mount::sync_runtime_config(config)
    })
}

fn detect_rule_file_type(path: &Path) -> Result<i32> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to read source metadata for {}", path.display()))?;
    let file_type = metadata.file_type();

    if file_type.is_char_device() && metadata.rdev() == 0 {
        bail!(
            "source {} is a whiteout node; use `hymofs rule hide` instead",
            path.display()
        );
    }

    if file_type.is_file() {
        Ok(libc::DT_REG as i32)
    } else if file_type.is_symlink() {
        Ok(libc::DT_LNK as i32)
    } else {
        bail!(
            "unsupported source type for rule add: {} (use `merge` or `add-dir` for directories)",
            path.display()
        )
    }
}

pub fn handle_gen_config(output: &Path, force: bool) -> Result<()> {
    if output.exists() && !force {
        bail!(
            "Config already exists at {}. Use --force to overwrite.",
            output.display()
        );
    }

    Config::default()
        .save_to_file(output)
        .with_context(|| format!("Failed to save generated config to {}", output.display()))
}

pub fn handle_show_config(cli: &Cli) -> Result<()> {
    let config = load_effective_config(cli)?;

    let json = serde_json::to_string(&config).context("Failed to serialize config to JSON")?;

    println!("{}", json);

    Ok(())
}

pub fn handle_save_config(payload: &str) -> Result<()> {
    let config: Config = decode_hex_json(payload, "config")?;

    config
        .save_to_file(defs::CONFIG_FILE)
        .context("Failed to save config file")?;

    println!("Configuration saved successfully.");

    Ok(())
}

pub fn handle_save_module_rules(module_id: &str, payload: &str) -> Result<()> {
    utils::validate_module_id(module_id)?;
    let new_rules: config::ModuleRules = decode_hex_json(payload, "module rules")?;
    let mut config = Config::load_default().unwrap_or_default();

    config.rules.insert(module_id.to_string(), new_rules);

    config
        .save_to_file(defs::CONFIG_FILE)
        .context("Failed to update config file with new rules")?;

    println!("Module rules saved for {} into config.toml", module_id);

    Ok(())
}

pub fn handle_modules(cli: &Cli) -> Result<()> {
    let config = load_effective_config(cli)?;

    modules::print_list(&config).context("Failed to list modules")
}

pub fn handle_state() -> Result<()> {
    let state = RuntimeState::load().context("Failed to load runtime state")?;
    println!(
        "{}",
        serde_json::to_string_pretty(&state).context("Failed to serialize runtime state")?
    );
    Ok(())
}

pub fn handle_logs(lines: usize) -> Result<()> {
    if !Path::new(defs::DAEMON_LOG_FILE).exists() {
        println!("No daemon log has been written yet.");
        return Ok(());
    }

    let content = fs::read_to_string(defs::DAEMON_LOG_FILE)
        .with_context(|| format!("Failed to read daemon log file {}", defs::DAEMON_LOG_FILE))?;
    let mut selected: Vec<&str> = content.lines().rev().take(lines).collect();
    selected.reverse();

    for line in selected {
        println!("{line}");
    }

    Ok(())
}

pub fn handle_api_system(cli: &Cli) -> Result<()> {
    let config = load_effective_config(cli)?;
    let state = RuntimeState::load().unwrap_or_default();
    let payload = api::build_system_payload(&config, &state);
    println!(
        "{}",
        serde_json::to_string_pretty(&payload).context("Failed to serialize API system payload")?
    );
    Ok(())
}

pub fn handle_api_storage() -> Result<()> {
    let state = RuntimeState::load().unwrap_or_default();
    let payload = api::build_storage_payload(&state);
    println!(
        "{}",
        serde_json::to_string_pretty(&payload)
            .context("Failed to serialize API storage payload")?
    );
    Ok(())
}

pub fn handle_api_mount_stats() -> Result<()> {
    let state = RuntimeState::load().unwrap_or_default();
    let payload = api::build_mount_stats_payload(&state);
    println!(
        "{}",
        serde_json::to_string_pretty(&payload).context("Failed to serialize API mount stats")?
    );
    Ok(())
}

pub fn handle_api_partitions(cli: &Cli) -> Result<()> {
    let config = load_effective_config(cli)?;
    let payload = api::build_partitions_payload(&config);
    println!(
        "{}",
        serde_json::to_string_pretty(&payload).context("Failed to serialize API partitions")?
    );
    Ok(())
}

pub fn handle_api_lkm(cli: &Cli) -> Result<()> {
    let config = load_effective_config(cli)?;
    let payload = api::build_lkm_payload(&config);
    println!(
        "{}",
        serde_json::to_string_pretty(&payload).context("Failed to serialize API lkm payload")?
    );
    Ok(())
}

pub fn handle_api_features() -> Result<()> {
    let payload = api::build_features_payload();
    println!(
        "{}",
        serde_json::to_string_pretty(&payload).context("Failed to serialize API features")?
    );
    Ok(())
}

pub fn handle_api_hooks(cli: &Cli) -> Result<()> {
    let config = load_effective_config(cli)?;
    require_live_hymofs(&config, "read HymoFS hooks")?;
    println!("{}", hymofs_mount::hook_lines()?.join("\n"));
    Ok(())
}

pub fn handle_hide_list() -> Result<()> {
    let rules = user_hide_rules::load_user_hide_rules()?;
    println!(
        "{}",
        serde_json::to_string_pretty(&rules).context("Failed to serialize user hide rules")?
    );
    Ok(())
}

pub fn handle_hide_add(cli: &Cli, path: &Path) -> Result<()> {
    let added = user_hide_rules::add_user_hide_rule(path)?;
    if added {
        let config = load_effective_config(cli)?;
        if hymofs_mount::can_operate(&config)
            && let Err(err) = hymofs::hide_path(path)
        {
            crate::scoped_log!(
                warn,
                "cli:hide",
                "live apply failed: path={}, error={:#}",
                path.display(),
                err
            );
        }
    }
    if added {
        println!("User hide rule added: {}", path.display());
    } else {
        println!("User hide rule already exists: {}", path.display());
    }
    Ok(())
}

pub fn handle_hide_remove(path: &Path) -> Result<()> {
    let removed = user_hide_rules::remove_user_hide_rule(path)?;
    if removed {
        println!(
            "User hide rule removed from persistent list: {}. Existing kernel hide state may persist until HymoFS rules are rebuilt.",
            path.display()
        );
    } else {
        println!("User hide rule was not present: {}", path.display());
    }
    Ok(())
}

pub fn handle_hide_apply(cli: &Cli) -> Result<()> {
    let config = load_effective_config(cli)?;
    require_live_hymofs(&config, "apply user hide rules")?;
    let (applied, failed) = user_hide_rules::apply_user_hide_rules()?;
    println!("User hide rules applied: {applied} succeeded, {failed} failed.");
    Ok(())
}

pub fn handle_hymofs_status(cli: &Cli) -> Result<()> {
    let config = load_effective_config(cli)?;
    let runtime_state = RuntimeState::load().unwrap_or_default();
    let hymofs_info = hymofs_mount::collect_runtime_info(&config);

    let output = json!({
        "status": hymofs_info.status,
        "available": hymofs_info.available,
        "protocol_version": hymofs_info.protocol_version,
        "feature_bits": hymofs_info.feature_bits,
        "feature_names": hymofs_info.feature_names,
        "hooks": hymofs_info.hooks,
        "rule_count": hymofs_info.rule_count,
        "user_hide_rule_count": hymofs_info.user_hide_rule_count,
        "mirror_path": hymofs_info.mirror_path,
        "lkm": api::build_lkm_payload(&config),
        "config": &config.hymofs,
        "runtime": {
            "snapshot": &runtime_state.hymofs,
            "hymofs_modules": &runtime_state.hymofs_modules,
            "active_mounts": &runtime_state.active_mounts,
            "log_file": &runtime_state.log_file,
        }
    });

    println!(
        "{}",
        serde_json::to_string_pretty(&output).context("Failed to serialize HymoFS status")?
    );

    Ok(())
}

pub fn handle_lkm_status(cli: &Cli) -> Result<()> {
    let config = load_effective_config(cli)?;
    let payload = api::build_lkm_payload(&config);
    println!(
        "{}",
        serde_json::to_string_pretty(&payload).context("Failed to serialize LKM status")?
    );
    Ok(())
}

pub fn handle_lkm_load(cli: &Cli) -> Result<()> {
    let config = load_effective_config(cli)?;
    lkm::load(&config.hymofs)?;
    hymofs::invalidate_status_cache();
    println!("HymoFS LKM loaded.");
    Ok(())
}

pub fn handle_lkm_unload(cli: &Cli) -> Result<()> {
    let config = load_effective_config(cli)?;
    lkm::unload(&config.hymofs)?;
    hymofs::invalidate_status_cache();
    println!("HymoFS LKM unloaded.");
    Ok(())
}

pub fn handle_lkm_set_autoload(cli: &Cli, enabled: bool) -> Result<()> {
    let mut config = load_effective_config(cli)?;
    config.hymofs.lkm_autoload = enabled;
    let path = save_hymofs_config_for_cli(cli, &config)?;
    println!(
        "HymoFS LKM autoload {} in {}.",
        if enabled { "enabled" } else { "disabled" },
        path.display()
    );
    Ok(())
}

pub fn handle_lkm_set_kmi(cli: &Cli, kmi: &str) -> Result<()> {
    let mut config = load_effective_config(cli)?;
    config.hymofs.lkm_kmi_override = kmi.to_string();
    let path = save_hymofs_config_for_cli(cli, &config)?;
    println!(
        "HymoFS LKM KMI override set to {} in {}.",
        kmi,
        path.display()
    );
    Ok(())
}

pub fn handle_lkm_clear_kmi(cli: &Cli) -> Result<()> {
    let mut config = load_effective_config(cli)?;
    config.hymofs.lkm_kmi_override.clear();
    let path = save_hymofs_config_for_cli(cli, &config)?;
    println!("HymoFS LKM KMI override cleared in {}.", path.display());
    Ok(())
}

pub fn handle_hymofs_list(cli: &Cli) -> Result<()> {
    let config = load_effective_config(cli)?;
    let payload = if config.hymofs.enabled && hymofs_mount::can_operate(&config) {
        api::parse_hymofs_rule_listing(&hymofs::get_active_rules()?)
    } else {
        Vec::new()
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&payload).context("Failed to serialize HymoFS rules")?
    );
    Ok(())
}

pub fn handle_hymofs_version(cli: &Cli) -> Result<()> {
    let config = load_effective_config(cli)?;
    let state = RuntimeState::load().unwrap_or_default();
    let payload = api::build_hymofs_version_payload(&config, &state);
    println!(
        "{}",
        serde_json::to_string_pretty(&payload).context("Failed to serialize HymoFS version")?
    );
    Ok(())
}

pub fn handle_hymofs_features() -> Result<()> {
    let output = api::build_features_payload();
    println!(
        "{}",
        serde_json::to_string_pretty(&output).context("Failed to serialize HymoFS features")?
    );
    Ok(())
}

pub fn handle_hymofs_hooks() -> Result<()> {
    println!("{}", hymofs_mount::hook_lines()?.join("\n"));
    Ok(())
}

pub fn handle_hymofs_clear() -> Result<()> {
    hymofs::clear_rules()?;
    println!("HymoFS rules cleared.");
    Ok(())
}

pub fn handle_hymofs_release_connection() -> Result<()> {
    hymofs::release_connection();
    println!("Released cached HymoFS client connection.");
    Ok(())
}

pub fn handle_hymofs_invalidate_cache() -> Result<()> {
    hymofs::invalidate_status_cache();
    println!("Invalidated cached HymoFS status.");
    Ok(())
}

pub fn handle_hymofs_fix_mounts() -> Result<()> {
    hymofs::fix_mounts()?;
    println!("HymoFS mount ordering fixed.");
    Ok(())
}

pub fn handle_hymofs_set_enabled(cli: &Cli, enabled: bool) -> Result<()> {
    let mut config = load_effective_config(cli)?;
    config.hymofs.enabled = enabled;
    let path = save_hymofs_config_for_cli(cli, &config)?;
    let applied = apply_live_if_possible(&config, "set_enabled", || {
        if enabled {
            hymofs_mount::sync_runtime_config(&config)?;
            hymofs::set_enabled(true)?;
        } else {
            hymofs_mount::clear_runtime_best_effort();
        }
        Ok(())
    })?;
    hymofs::invalidate_status_cache();
    print_config_apply_result(
        &path,
        if enabled {
            "HymoFS enabled state"
        } else {
            "HymoFS disabled state"
        },
        applied,
    );
    Ok(())
}

pub fn handle_hymofs_set_hidexattr(cli: &Cli, enabled: bool) -> Result<()> {
    let mut config = load_effective_config(cli)?;
    config.hymofs.enable_hidexattr = enabled;
    let path = save_hymofs_config_for_cli(cli, &config)?;
    let applied = apply_live_runtime_sync(&config, "set_hidexattr")?;
    print_config_apply_result(&path, "HymoFS hidexattr setting", applied);
    Ok(())
}

pub fn handle_hymofs_set_mirror(cli: &Cli, path_value: &Path) -> Result<()> {
    let mut config = load_effective_config(cli)?;
    config.hymofs.mirror_path = path_value.to_path_buf();
    let path = save_hymofs_config_for_cli(cli, &config)?;
    let applied = apply_live_runtime_sync(&config, "set_mirror_path")?;
    print_config_apply_result(&path, "HymoFS mirror path", applied);
    Ok(())
}

pub fn handle_hymofs_set_debug(cli: &Cli, enabled: bool) -> Result<()> {
    let mut config = load_effective_config(cli)?;
    config.hymofs.enable_kernel_debug = enabled;
    let path = save_hymofs_config_for_cli(cli, &config)?;
    let applied = apply_live_runtime_sync(&config, "set_debug")?;
    print_config_apply_result(&path, "HymoFS kernel debug setting", applied);
    Ok(())
}

pub fn handle_hymofs_set_stealth(cli: &Cli, enabled: bool) -> Result<()> {
    let mut config = load_effective_config(cli)?;
    config.hymofs.enable_stealth = enabled;
    let path = save_hymofs_config_for_cli(cli, &config)?;
    let applied = apply_live_runtime_sync(&config, "set_stealth")?;
    print_config_apply_result(&path, "HymoFS stealth setting", applied);
    Ok(())
}

pub fn handle_hymofs_set_ignore_protocol_mismatch(cli: &Cli, enabled: bool) -> Result<()> {
    let mut config = load_effective_config(cli)?;
    config.hymofs.ignore_protocol_mismatch = enabled;
    let path = save_hymofs_config_for_cli(cli, &config)?;
    hymofs::invalidate_status_cache();
    println!(
        "HymoFS protocol mismatch policy saved to {}. Ignore mismatch is now {}.",
        path.display(),
        if enabled { "enabled" } else { "disabled" }
    );
    Ok(())
}

pub fn handle_hymofs_set_mount_hide(
    cli: &Cli,
    enabled: bool,
    path_pattern: Option<&Path>,
) -> Result<()> {
    let mut config = load_effective_config(cli)?;
    config.hymofs.enable_mount_hide = enabled;
    config.hymofs.mount_hide.enabled = enabled;
    match (enabled, path_pattern) {
        (_, Some(path_pattern)) => {
            config.hymofs.mount_hide.path_pattern = path_pattern.to_path_buf()
        }
        (false, None) => clear_pathbuf(&mut config.hymofs.mount_hide.path_pattern),
        (true, None) => {}
    }

    let save_path = save_hymofs_config_for_cli(cli, &config)?;
    let applied = apply_live_runtime_sync(&config, "set_mount_hide")?;
    print_config_apply_result(&save_path, "HymoFS mount_hide setting", applied);
    Ok(())
}

pub fn handle_hymofs_set_maps_spoof(cli: &Cli, enabled: bool) -> Result<()> {
    let mut config = load_effective_config(cli)?;
    config.hymofs.enable_maps_spoof = enabled;
    let path = save_hymofs_config_for_cli(cli, &config)?;
    let applied = apply_live_runtime_sync(&config, "set_maps_spoof")?;
    print_config_apply_result(&path, "HymoFS maps_spoof setting", applied);
    Ok(())
}

pub fn handle_hymofs_set_statfs_spoof(
    cli: &Cli,
    enabled: bool,
    path_value: Option<&Path>,
    spoof_f_type: Option<u64>,
) -> Result<()> {
    let mut config = load_effective_config(cli)?;
    config.hymofs.enable_statfs_spoof = enabled;
    config.hymofs.statfs_spoof.enabled = enabled;
    match path_value {
        Some(path) => config.hymofs.statfs_spoof.path = path.to_path_buf(),
        None if !enabled => clear_pathbuf(&mut config.hymofs.statfs_spoof.path),
        None => {}
    }
    if let Some(spoof_f_type) = spoof_f_type {
        config.hymofs.statfs_spoof.spoof_f_type = spoof_f_type;
    } else if !enabled {
        config.hymofs.statfs_spoof.spoof_f_type = 0;
    }

    let save_path = save_hymofs_config_for_cli(cli, &config)?;
    let applied = apply_live_runtime_sync(&config, "set_statfs_spoof")?;
    print_config_apply_result(&save_path, "HymoFS statfs_spoof setting", applied);
    Ok(())
}

pub fn handle_hymofs_set_uname(
    cli: &Cli,
    sysname: Option<&str>,
    nodename: Option<&str>,
    release: Option<&str>,
    version: Option<&str>,
    machine: Option<&str>,
    domainname: Option<&str>,
) -> Result<()> {
    if sysname.is_none()
        && nodename.is_none()
        && release.is_none()
        && version.is_none()
        && machine.is_none()
        && domainname.is_none()
    {
        bail!("No uname fields were provided. Use `hymofs uname clear` to clear spoofing.");
    }

    let mut config = load_effective_config(cli)?;
    if let Some(value) = sysname {
        config.hymofs.uname.sysname = value.to_string();
    }
    if let Some(value) = nodename {
        config.hymofs.uname.nodename = value.to_string();
    }
    if let Some(value) = release {
        config.hymofs.uname.release = value.to_string();
        config.hymofs.uname_release = value.to_string();
    }
    if let Some(value) = version {
        config.hymofs.uname.version = value.to_string();
        config.hymofs.uname_version = value.to_string();
    }
    if let Some(value) = machine {
        config.hymofs.uname.machine = value.to_string();
    }
    if let Some(value) = domainname {
        config.hymofs.uname.domainname = value.to_string();
    }

    let path = save_hymofs_config_for_cli(cli, &config)?;
    let applied = apply_live_runtime_sync(&config, "set_uname")?;
    print_config_apply_result(&path, "HymoFS uname spoof setting", applied);
    Ok(())
}

pub fn handle_hymofs_clear_uname(cli: &Cli) -> Result<()> {
    let mut config = load_effective_config(cli)?;
    config.hymofs.uname = Default::default();
    config.hymofs.uname_release.clear();
    config.hymofs.uname_version.clear();

    let path = save_hymofs_config_for_cli(cli, &config)?;
    let applied = apply_live_runtime_sync(&config, "clear_uname")?;
    print_config_apply_result(&path, "HymoFS uname spoof setting", applied);
    Ok(())
}

pub fn handle_hymofs_set_cmdline(cli: &Cli, value: &str) -> Result<()> {
    let mut config = load_effective_config(cli)?;
    config.hymofs.cmdline_value = value.to_string();

    let path = save_hymofs_config_for_cli(cli, &config)?;
    let applied = apply_live_runtime_sync(&config, "set_cmdline")?;
    print_config_apply_result(&path, "HymoFS cmdline spoof setting", applied);
    Ok(())
}

pub fn handle_hymofs_clear_cmdline(cli: &Cli) -> Result<()> {
    let mut config = load_effective_config(cli)?;
    config.hymofs.cmdline_value.clear();

    let path = save_hymofs_config_for_cli(cli, &config)?;
    let applied = apply_live_runtime_sync(&config, "clear_cmdline")?;
    print_config_apply_result(&path, "HymoFS cmdline spoof setting", applied);
    Ok(())
}

pub fn handle_hymofs_set_hide_uids(cli: &Cli, uids: &[u32]) -> Result<()> {
    let mut config = load_effective_config(cli)?;
    config.hymofs.hide_uids = uids.to_vec();

    let path = save_hymofs_config_for_cli(cli, &config)?;
    let applied = apply_live_runtime_sync(&config, "set_hide_uids")?;
    print_config_apply_result(&path, "HymoFS hide_uids setting", applied);
    Ok(())
}

pub fn handle_hymofs_clear_hide_uids(cli: &Cli) -> Result<()> {
    let mut config = load_effective_config(cli)?;
    config.hymofs.hide_uids.clear();

    let path = save_hymofs_config_for_cli(cli, &config)?;
    let applied = apply_live_runtime_sync(&config, "clear_hide_uids")?;
    print_config_apply_result(&path, "HymoFS hide_uids setting", applied);
    Ok(())
}

pub fn handle_hymofs_add_maps_rule(
    cli: &Cli,
    target_ino: u64,
    target_dev: u64,
    spoofed_ino: u64,
    spoofed_dev: u64,
    path: &Path,
) -> Result<()> {
    let mut config = load_effective_config(cli)?;
    let new_rule = HymoMapsRuleConfig {
        target_ino,
        target_dev,
        spoofed_ino,
        spoofed_dev,
        spoofed_pathname: path.to_path_buf(),
    };

    if let Some(existing) = config
        .hymofs
        .maps_rules
        .iter_mut()
        .find(|rule| rule.target_ino == target_ino && rule.target_dev == target_dev)
    {
        *existing = new_rule.clone();
    } else {
        config.hymofs.maps_rules.push(new_rule);
    }

    let path_out = save_hymofs_config_for_cli(cli, &config)?;
    let applied = apply_live_runtime_sync(&config, "add_maps_rule")?;
    print_config_apply_result(&path_out, "HymoFS maps rule", applied);
    Ok(())
}

pub fn handle_hymofs_clear_maps_rules(cli: &Cli) -> Result<()> {
    let mut config = load_effective_config(cli)?;
    config.hymofs.maps_rules.clear();

    let path = save_hymofs_config_for_cli(cli, &config)?;
    let applied = apply_live_runtime_sync(&config, "clear_maps_rules")?;
    print_config_apply_result(&path, "HymoFS maps rules", applied);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn handle_hymofs_upsert_kstat_rule(
    cli: &Cli,
    target_ino: u64,
    target_path: &Path,
    spoofed_ino: u64,
    spoofed_dev: u64,
    spoofed_nlink: u32,
    spoofed_size: i64,
    spoofed_atime_sec: i64,
    spoofed_atime_nsec: i64,
    spoofed_mtime_sec: i64,
    spoofed_mtime_nsec: i64,
    spoofed_ctime_sec: i64,
    spoofed_ctime_nsec: i64,
    spoofed_blksize: u64,
    spoofed_blocks: u64,
    is_static: bool,
) -> Result<()> {
    let mut config = load_effective_config(cli)?;
    let new_rule = HymoKstatRuleConfig {
        target_ino,
        target_pathname: target_path.to_path_buf(),
        spoofed_ino,
        spoofed_dev,
        spoofed_nlink,
        spoofed_size,
        spoofed_atime_sec,
        spoofed_atime_nsec,
        spoofed_mtime_sec,
        spoofed_mtime_nsec,
        spoofed_ctime_sec,
        spoofed_ctime_nsec,
        spoofed_blksize,
        spoofed_blocks,
        is_static,
    };

    if let Some(existing) = config
        .hymofs
        .kstat_rules
        .iter_mut()
        .find(|rule| rule.target_ino == target_ino && rule.target_pathname == target_path)
    {
        *existing = new_rule.clone();
    } else {
        config.hymofs.kstat_rules.push(new_rule.clone());
    }

    let path = save_hymofs_config_for_cli(cli, &config)?;
    let applied = apply_live_runtime_sync(&config, "upsert_kstat_rule")?;
    print_config_apply_result(&path, "HymoFS kstat rule", applied);
    Ok(())
}

pub fn handle_hymofs_clear_kstat_rules_config(cli: &Cli) -> Result<()> {
    let mut config = load_effective_config(cli)?;
    config.hymofs.kstat_rules.clear();

    let path = save_hymofs_config_for_cli(cli, &config)?;
    println!(
        "HymoFS kstat rules were removed from {}. Existing kernel kstat spoof rules may persist until the LKM is reloaded.",
        path.display()
    );
    Ok(())
}

pub fn handle_hymofs_rule_add(
    cli: &Cli,
    target: &Path,
    source: &Path,
    file_type: Option<i32>,
) -> Result<()> {
    let config = load_effective_config(cli)?;
    require_live_hymofs(&config, "add HymoFS rule")?;
    let file_type = match file_type {
        Some(value) => value,
        None => detect_rule_file_type(source)?,
    };
    hymofs::add_rule(target, source, file_type)?;
    println!(
        "HymoFS ADD rule applied: target={}, source={}, file_type={}",
        target.display(),
        source.display(),
        file_type
    );
    Ok(())
}

pub fn handle_hymofs_rule_merge(cli: &Cli, target: &Path, source: &Path) -> Result<()> {
    let config = load_effective_config(cli)?;
    require_live_hymofs(&config, "add HymoFS merge rule")?;
    hymofs::add_merge_rule(target, source)?;
    println!(
        "HymoFS MERGE rule applied: target={}, source={}",
        target.display(),
        source.display()
    );
    Ok(())
}

pub fn handle_hymofs_rule_hide(cli: &Cli, path: &Path) -> Result<()> {
    let config = load_effective_config(cli)?;
    require_live_hymofs(&config, "add HymoFS hide rule")?;
    hymofs::hide_path(path)?;
    println!("HymoFS HIDE rule applied: {}", path.display());
    Ok(())
}

pub fn handle_hymofs_rule_delete(cli: &Cli, path: &Path) -> Result<()> {
    let config = load_effective_config(cli)?;
    require_live_hymofs(&config, "delete HymoFS rule")?;
    hymofs::delete_rule(path)?;
    println!("HymoFS rule deleted: {}", path.display());
    Ok(())
}

pub fn handle_hymofs_rule_add_dir(cli: &Cli, target_base: &Path, source_dir: &Path) -> Result<()> {
    let config = load_effective_config(cli)?;
    require_live_hymofs(&config, "add HymoFS rules from directory")?;
    hymofs::add_rules_from_directory(target_base, source_dir)?;
    println!(
        "HymoFS directory rules applied: target_base={}, source_dir={}",
        target_base.display(),
        source_dir.display()
    );
    Ok(())
}

pub fn handle_hymofs_rule_remove_dir(
    cli: &Cli,
    target_base: &Path,
    source_dir: &Path,
) -> Result<()> {
    let config = load_effective_config(cli)?;
    require_live_hymofs(&config, "remove HymoFS rules from directory")?;
    hymofs::remove_rules_from_directory(target_base, source_dir)?;
    println!(
        "HymoFS directory rules removed: target_base={}, source_dir={}",
        target_base.display(),
        source_dir.display()
    );
    Ok(())
}
