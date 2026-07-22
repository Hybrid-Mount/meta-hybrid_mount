// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::{
    fs,
    path::Path,
    process::Command,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use anyhow::{Context, Result, bail};
use serde::Serialize;
use serde_json::{Value, json};

#[cfg(feature = "kasumi")]
use super::super::protocol::KasumiCommand;
use super::{
    super::protocol::{ConfigCommand, DaemonCommand, ModulesCommand, SystemCommand},
    http::{self, WebuiHttpSession},
};
use crate::{
    conf::config::Config,
    core::{api, runtime_state::RuntimeState},
    defs,
};
#[cfg(feature = "kasumi")]
use crate::{
    conf::schema,
    core::user_hide_rules,
    mount::kasumi as kasumi_mount,
    sys::{kasumi, lkm},
};

pub(super) struct RuntimeConfigAccess {
    write_lock: Mutex<()>,
}

impl RuntimeConfigAccess {
    pub(super) fn new() -> Self {
        Self {
            write_lock: Mutex::new(()),
        }
    }

    fn lock_writes(&self) -> Result<std::sync::MutexGuard<'_, ()>> {
        self.write_lock
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime config write lock is poisoned"))
    }

    pub(super) fn load(&self, config_path: &Path) -> Result<Arc<Config>> {
        Ok(Arc::new(load_runtime_config_uncached(config_path)?))
    }
}

pub(super) fn load_runtime_config(
    config_access: &RuntimeConfigAccess,
    config_path: &Path,
) -> Result<Arc<Config>> {
    config_access.load(config_path)
}

fn load_runtime_config_uncached(config_path: &Path) -> Result<Config> {
    let config = Config::load_from_file(config_path)
        .with_context(|| format!("Failed to load config from path: {}", config_path.display()))?;
    crate::conf::loader::load_module_blacklist(config)
}

pub(super) struct CommandContext<'a> {
    config: &'a Config,
    config_path: &'a Path,
    config_access: &'a RuntimeConfigAccess,
    state: &'a Arc<Mutex<RuntimeState>>,
    shutdown: &'a Arc<AtomicBool>,
    webui: &'a WebuiHttpSession,
    sse_clients: &'a http::SharedSseClients,
}

impl<'a> CommandContext<'a> {
    pub(super) fn new(
        config: &'a Config,
        config_path: &'a Path,
        config_access: &'a RuntimeConfigAccess,
        state: &'a Arc<Mutex<RuntimeState>>,
        shutdown: &'a Arc<AtomicBool>,
        webui: &'a WebuiHttpSession,
        sse_clients: &'a http::SharedSseClients,
    ) -> Self {
        Self {
            config,
            config_path,
            config_access,
            state,
            shutdown,
            webui,
            sse_clients,
        }
    }

    fn refresh<T: Serialize>(&self, config: &Config, payload: T) -> Result<Value> {
        self.refresh_runtime_snapshot(config)?;
        to_value(&payload)
    }

    #[cfg(feature = "kasumi")]
    fn refresh_current<T: Serialize>(&self, payload: T) -> Result<Value> {
        self.refresh(self.config, payload)
    }

    #[cfg(feature = "kasumi")]
    fn refresh_message(&self, message: &'static str) -> Result<Value> {
        self.refresh_current(json!({ "message": message }))
    }

    #[cfg(feature = "kasumi")]
    fn invalidate_and_refresh_message(&self, message: &'static str) -> Result<Value> {
        kasumi_mount::invalidate_runtime_caches()?;
        self.refresh_message(message)
    }

    fn refresh_runtime_snapshot(&self, config: &Config) -> Result<()> {
        refresh_runtime_snapshot(config, self.state, self.sse_clients)
    }
}

fn runtime_snapshot(state: &Arc<Mutex<RuntimeState>>) -> Result<RuntimeState> {
    Ok(state
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime state lock is poisoned"))?
        .clone())
}

fn cached_status_value(state: &Arc<Mutex<RuntimeState>>) -> Result<Value> {
    let mut guard = state
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime state lock is poisoned"))?;
    Ok(guard.status_value()?.clone())
}

fn cached_status_and_snapshot(state: &Arc<Mutex<RuntimeState>>) -> Result<(Value, RuntimeState)> {
    let mut guard = state
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime state lock is poisoned"))?;
    let status_value = guard.status_value()?.clone();
    Ok((status_value, guard.clone()))
}

// ── Top-level dispatch ──────────────────────────────────────────────────

pub(super) fn dispatch_command(ctx: &CommandContext<'_>, command: DaemonCommand) -> Result<Value> {
    let _write_guard = if command_writes_config(&command) {
        Some(ctx.config_access.lock_writes()?)
    } else {
        None
    };
    dispatch_command_unlocked(ctx, command)
}

fn dispatch_command_unlocked(ctx: &CommandContext<'_>, command: DaemonCommand) -> Result<Value> {
    match command {
        DaemonCommand::System(cmd) => dispatch_system(ctx, cmd),
        DaemonCommand::Config(cmd) => dispatch_config(ctx, cmd),
        DaemonCommand::Modules(cmd) => dispatch_modules(ctx, cmd),
        #[cfg(feature = "kasumi")]
        DaemonCommand::Kasumi(cmd) => dispatch_kasumi(ctx, cmd),
    }
}

fn command_writes_config(command: &DaemonCommand) -> bool {
    match command {
        DaemonCommand::Config(ConfigCommand::Get) => false,
        DaemonCommand::Config(_) | DaemonCommand::Modules(ModulesCommand::Apply { .. }) => true,
        DaemonCommand::Modules(ModulesCommand::List) | DaemonCommand::System(_) => false,
        #[cfg(feature = "kasumi")]
        DaemonCommand::Kasumi(KasumiCommand::MapsAdd { .. } | KasumiCommand::MapsClear) => true,
        #[cfg(feature = "kasumi")]
        DaemonCommand::Kasumi(_) => false,
    }
}

// ── System commands ─────────────────────────────────────────────────────

fn dispatch_system(ctx: &CommandContext<'_>, cmd: SystemCommand) -> Result<Value> {
    let config = ctx.config;
    let state = ctx.state;
    let shutdown = ctx.shutdown;
    let webui = ctx.webui;

    match cmd {
        SystemCommand::Ping => Ok(json!({ "status": "ok" })),
        SystemCommand::WebuiStart => Ok(webui.session_payload()),
        SystemCommand::Shutdown => {
            shutdown.store(true, Ordering::Relaxed);
            Ok(json!({ "shutdown": true }))
        }
        SystemCommand::Init => dispatch_init(ctx),
        SystemCommand::Status => cached_status_value(state),
        SystemCommand::ApiStorage => {
            let snapshot = runtime_snapshot(state)?;
            to_value(&api::build_storage_payload(&snapshot)?)
        }
        SystemCommand::ApiMountStats => {
            let snapshot = runtime_snapshot(state)?;
            to_value(&api::build_mount_stats_payload(&snapshot))
        }
        SystemCommand::ApiMountTopology => {
            let snapshot = runtime_snapshot(state)?;
            to_value(&api::build_mount_topology_payload(config, &snapshot)?)
        }
        SystemCommand::ApiPartitions => to_value(&api::build_partitions_payload(config)?),
        SystemCommand::ApiSystemInfo => {
            let snapshot = runtime_snapshot(state)?;
            to_value(&api::build_system_info_payload(&snapshot)?)
        }
        SystemCommand::ApiVersion => to_value(&api::build_version_payload()),
        SystemCommand::ApiKernelUname => to_value(&read_kernel_uname_payload()?),
        SystemCommand::ApiOpenUrl { url } => {
            open_url(&url)?;
            Ok(json!({ "opened": true }))
        }
        SystemCommand::ApiReboot => {
            reboot_device()?;
            Ok(json!({ "reboot": true }))
        }
    }
}

fn dispatch_init(ctx: &CommandContext<'_>) -> Result<Value> {
    let config = ctx.config;
    let state = ctx.state;

    let (status_value, snapshot) = cached_status_and_snapshot(state)?;
    let config_value = to_value(config)?;
    let version_value = to_value(&api::build_version_payload())?;
    let system_info_value = to_value(&api::build_system_info_payload(&snapshot)?)?;
    #[cfg(feature = "kasumi")]
    {
        let kasumi_status_value = build_kasumi_runtime_json(config, &snapshot)?;
        Ok(json!({
            "status": status_value,
            "config": config_value,
            "version": version_value,
            "kasumi_status": kasumi_status_value,
            "system_info": system_info_value,
        }))
    }
    #[cfg(not(feature = "kasumi"))]
    Ok(json!({
        "status": status_value,
        "config": config_value,
        "version": version_value,
        "system_info": system_info_value,
    }))
}

// ── Config commands ─────────────────────────────────────────────────────

fn dispatch_config(ctx: &CommandContext<'_>, cmd: ConfigCommand) -> Result<Value> {
    let config = ctx.config;
    let config_path = ctx.config_path;

    match cmd {
        ConfigCommand::Get => to_value(config),
        ConfigCommand::Set { config: payload } => {
            let config: Config =
                serde_json::from_value(payload).context("Failed to decode config payload")?;
            config.save_to_file(config_path)?;
            ctx.refresh(&config, json!({ "saved": true, "config": &config }))
        }
        ConfigCommand::Patch {
            patch,
            apply_runtime,
        } => {
            let config = patch_config_file(config_path, patch)?;
            let applied = if apply_runtime {
                apply_runtime_config(&config)?
            } else {
                false
            };
            ctx.refresh(
                &config,
                json!({
                    "saved": true,
                    "applied": applied,
                    "config": &config,
                }),
            )
        }
        ConfigCommand::Reset => {
            let config = Config::default();
            save_and_apply_runtime_config(&config, config_path)?;
            ctx.refresh(&config, json!({ "saved": true, "config": &config }))
        }
    }
}

// ── Modules commands ────────────────────────────────────────────────────

fn dispatch_modules(ctx: &CommandContext<'_>, cmd: ModulesCommand) -> Result<Value> {
    let config = ctx.config;
    let config_path = ctx.config_path;
    let state = ctx.state;

    match cmd {
        ModulesCommand::List => {
            let snapshot = runtime_snapshot(state)?;
            to_value(&api::build_modules_payload(config, &snapshot)?)
        }
        ModulesCommand::Apply { modules } => {
            let payload = api::apply_modules_payload(config_path, &modules)?;
            let config = load_runtime_config_uncached(config_path)?;
            ctx.refresh(&config, payload)
        }
    }
}

// ── Kasumi commands ─────────────────────────────────────────────────────

#[cfg(feature = "kasumi")]
fn dispatch_kasumi(ctx: &CommandContext<'_>, cmd: KasumiCommand) -> Result<Value> {
    let config = ctx.config;
    let config_path = ctx.config_path;
    let config_access = ctx.config_access;
    let state = ctx.state;

    match cmd {
        KasumiCommand::Status => {
            let snapshot = runtime_snapshot(state)?;
            build_kasumi_runtime_json(config, &snapshot)
        }
        KasumiCommand::List => {
            kasumi_mount::require_live(config, "list rules")?;
            let payload = api::parse_kasumi_rule_listing(&kasumi::list_rules()?)?;
            to_value(&payload)
        }
        KasumiCommand::Version => {
            let snapshot = runtime_snapshot(state)?;
            to_value(&api::build_kasumi_version_payload(config, &snapshot)?)
        }
        KasumiCommand::Features => to_value(&api::build_features_payload()?),
        KasumiCommand::Hooks => to_value(&kasumi_mount::hook_lines()?),
        KasumiCommand::ApplyConfigRuntime => {
            let applied = apply_runtime_config(config)?;
            ctx.refresh_current(json!({ "applied": applied }))
        }
        KasumiCommand::Clear => {
            kasumi::clear_rules()?;
            ctx.refresh_message("Kasumi rules cleared.")
        }
        KasumiCommand::ReleaseConnection => {
            kasumi::release_connection()?;
            ctx.refresh_message("Released cached Kasumi client connection.")
        }
        KasumiCommand::InvalidateCache => {
            kasumi_mount::invalidate_runtime_caches()?;
            ctx.refresh_message("Invalidated cached Kasumi status.")
        }
        KasumiCommand::FixMounts => {
            kasumi::fix_mounts()?;
            ctx.refresh_message("Kasumi mount ordering fixed.")
        }
        KasumiCommand::RestoreUnameGlobal => {
            kasumi::restore_uname_global()?;
            ctx.refresh_message("Kasumi global uname restored.")
        }
        KasumiCommand::SetUname {
            mode,
            release,
            version,
        } => {
            let mode = parse_uname_mode(&mode)?;
            apply_uname(mode, &release, &version)?;
            ctx.refresh_current(json!({
                "message": "Kasumi uname applied.",
                "mode": display_uname_mode(mode),
                "release": release,
                "version": version,
            }))
        }
        KasumiCommand::ClearUname { mode } => {
            let mode = parse_uname_mode(&mode)?;
            match mode {
                schema::KasumiUnameMode::Scoped => {
                    apply_uname(schema::KasumiUnameMode::Scoped, "", "")?
                }
                schema::KasumiUnameMode::Global => kasumi::restore_uname_global()?,
            }
            ctx.refresh_current(json!({
                "message": "Kasumi uname cleared.",
                "mode": display_uname_mode(mode),
            }))
        }
        KasumiCommand::RuleAdd {
            target,
            source,
            file_type,
        } => {
            kasumi::add_rule(&target, &source, file_type)?;
            ctx.refresh_current(json!({
                "message": "Kasumi ADD rule applied.",
                "target": target,
                "source": source,
                "file_type": file_type,
            }))
        }
        KasumiCommand::RuleMerge { target, source } => {
            kasumi::add_merge_rule(&target, &source)?;
            ctx.refresh_current(json!({
                "message": "Kasumi MERGE rule applied.",
                "target": target,
                "source": source,
            }))
        }
        KasumiCommand::RuleHide { path } => {
            kasumi::hide_path(&path)?;
            ctx.refresh_current(json!({
                "message": "Kasumi HIDE rule applied.",
                "path": path,
            }))
        }
        KasumiCommand::RuleDelete { path } => {
            kasumi::delete_rule(&path)?;
            ctx.refresh_current(json!({
                "message": "Kasumi rule deleted.",
                "path": path,
            }))
        }
        KasumiCommand::RuleAddDir {
            target_base,
            source_dir,
        } => {
            kasumi::add_rules_from_directory(&target_base, &source_dir)?;
            ctx.refresh_current(json!({
                "message": "Kasumi directory rules applied.",
                "target_base": target_base,
                "source_dir": source_dir,
            }))
        }
        KasumiCommand::RuleRemoveDir {
            target_base,
            source_dir,
        } => {
            kasumi::remove_rules_from_directory(&target_base, &source_dir)?;
            ctx.refresh_current(json!({
                "message": "Kasumi directory rules removed.",
                "target_base": target_base,
                "source_dir": source_dir,
            }))
        }
        KasumiCommand::HideList => to_value(&user_hide_rules::load_user_hide_rules()?),
        KasumiCommand::HideAdd { path } => {
            let added = user_hide_rules::add_user_hide_rule(&path)?;
            if added && kasumi_mount::can_operate(config)? {
                kasumi::hide_path(&path)?;
            }
            ctx.refresh_current(json!({ "added": added, "path": path }))
        }
        KasumiCommand::HideRemove { path } => {
            let removed = user_hide_rules::remove_user_hide_rule(&path)?;
            ctx.refresh_current(json!({ "removed": removed, "path": path }))
        }
        KasumiCommand::HideApply => {
            kasumi_mount::require_live(config, "apply user hide rules")?;
            let applied = user_hide_rules::apply_user_hide_rules()?;
            ctx.refresh_current(json!({ "applied": applied }))
        }
        KasumiCommand::LkmStatus => to_value(&api::build_lkm_payload(config)?),
        KasumiCommand::LkmLoad => {
            lkm::load(&config.kasumi)?;
            ctx.invalidate_and_refresh_message("Kasumi LKM loaded.")
        }
        KasumiCommand::LkmUnload => {
            lkm::unload(&config.kasumi)?;
            ctx.invalidate_and_refresh_message("Kasumi LKM unloaded.")
        }
        KasumiCommand::MapsAdd { rule } => {
            let updated = add_kasumi_maps_config_rule(config_path, rule)?;
            apply_runtime_config(&updated)?;
            let count = updated.kasumi.maps_rules.len();
            ctx.refresh(
                &updated,
                json!({
                    "saved": true,
                    "config": &updated,
                    "count": count,
                }),
            )
        }
        KasumiCommand::MapsClear => {
            let mut updated = load_runtime_config(config_access, config_path)?
                .as_ref()
                .clone();
            updated.kasumi.maps_rules.clear();
            updated.save_to_file(config_path)?;
            apply_runtime_config(&updated)?;
            ctx.refresh(
                &updated,
                json!({
                    "saved": true,
                    "config": &updated,
                    "count": 0,
                }),
            )
        }
    }
}

fn patch_config_file(config_path: &Path, patch: Value) -> Result<Config> {
    let config = Config::load_from_file(config_path)
        .with_context(|| format!("Failed to load config from path: {}", config_path.display()))?;
    let mut payload = serde_json::to_value(config).context("Failed to encode current config")?;
    merge_json(&mut payload, patch, 0)
        .context("Failed to merge config patch (nesting too deep)")?;

    let config: Config =
        serde_json::from_value(payload).context("Failed to decode patched config")?;
    config.save_to_file(config_path)?;
    Ok(config)
}

fn merge_json(target: &mut Value, patch: Value, depth: usize) -> Result<()> {
    if depth > crate::defs::MAX_MERGE_JSON_DEPTH {
        bail!(
            "JSON patch nesting exceeds max depth of {}",
            crate::defs::MAX_MERGE_JSON_DEPTH
        );
    }
    match (target, patch) {
        (Value::Object(target), Value::Object(patch)) => {
            for (key, value) in patch {
                match target.get_mut(&key) {
                    Some(existing) => merge_json(existing, value, depth + 1)?,
                    None => {
                        target.insert(key, value);
                    }
                }
            }
        }
        (target, patch) => {
            *target = patch;
        }
    }
    Ok(())
}

fn read_kernel_uname_payload() -> Result<Value> {
    let release = fs::read_to_string("/proc/sys/kernel/osrelease")
        .context("failed to read /proc/sys/kernel/osrelease")?
        .trim()
        .to_string();
    let version = fs::read_to_string("/proc/sys/kernel/version")
        .context("failed to read /proc/sys/kernel/version")?
        .trim()
        .to_string();
    Ok(json!({ "release": release, "version": version }))
}

fn open_url(url: &str) -> Result<()> {
    validate_url(url)?;
    let status = Command::new("am")
        .arg("start")
        .arg("-a")
        .arg("android.intent.action.VIEW")
        .arg("-d")
        .arg(url)
        .status()
        .context("Failed to start Android VIEW intent")?;
    if !status.success() {
        bail!("am start exited with status {status}");
    }
    Ok(())
}

fn validate_url(url: &str) -> Result<()> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        bail!("URL must start with http:// or https://");
    }
    if url.contains('\0') || url.contains('\n') || url.contains('\r') {
        bail!("URL contains invalid control characters");
    }
    // Reject URLs that could be misinterpreted as am(1) flags
    if url.contains(" --") {
        bail!("URL contains suspicious argument-like patterns");
    }
    Ok(())
}

fn reboot_device() -> Result<()> {
    let status = Command::new("reboot")
        .status()
        .context("Failed to execute reboot")?;
    if !status.success() {
        bail!("reboot exited with status {status}");
    }
    Ok(())
}

#[cfg(feature = "kasumi")]
fn add_kasumi_maps_config_rule(config_path: &Path, rule: Value) -> Result<Config> {
    let mut config = load_runtime_config_uncached(config_path)?;
    let rule: crate::conf::schema::KasumiMapsRuleConfig =
        serde_json::from_value(rule).context("Failed to decode Kasumi maps rule")?;
    config
        .kasumi
        .maps_rules
        .retain(|item| item.target_ino != rule.target_ino || item.target_dev != rule.target_dev);
    config.kasumi.maps_rules.push(rule);
    config.save_to_file(config_path)?;
    Ok(config)
}

fn save_and_apply_runtime_config(config: &Config, config_path: &Path) -> Result<bool> {
    config.save_to_file(config_path)?;
    apply_runtime_config(config)
}

#[cfg(feature = "kasumi")]
fn apply_runtime_config(config: &Config) -> Result<bool> {
    let applied = kasumi_mount::apply_runtime_config(config)?;
    kasumi_mount::invalidate_runtime_caches()?;
    Ok(applied)
}

#[cfg(not(feature = "kasumi"))]
fn apply_runtime_config(_config: &Config) -> Result<bool> {
    Ok(false)
}

fn refresh_runtime_snapshot(
    config: &Config,
    state: &Arc<Mutex<RuntimeState>>,
    sse_clients: &http::SharedSseClients,
) -> Result<()> {
    let mut guard = state
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime state lock is poisoned"))?;
    #[cfg(feature = "kasumi")]
    let runtime_changed = {
        kasumi_mount::invalidate_runtime_caches()?;
        let next = kasumi_mount::collect_runtime_info(config)?;
        let changed = guard.kasumi != next;
        guard.kasumi = next;
        changed
    };
    #[cfg(not(feature = "kasumi"))]
    let runtime_changed = {
        let _ = config;
        false
    };
    let daemon_changed = !guard.daemon.alive || guard.daemon.socket_path != defs::SOCKET_FILE;
    guard.set_daemon_state(true, defs::SOCKET_FILE)?;
    guard
        .status_value()
        .map_err(|e| anyhow::anyhow!("Failed to cache status value: {e}"))?;
    if runtime_changed || daemon_changed {
        guard.save()?;
    }
    drop(guard);
    http::broadcast_sse_event(state, sse_clients, "runtime_changed")?;
    Ok(())
}

#[cfg(feature = "kasumi")]
fn parse_uname_mode(mode: &str) -> Result<schema::KasumiUnameMode> {
    match mode {
        "scoped" => Ok(schema::KasumiUnameMode::Scoped),
        "global" => Ok(schema::KasumiUnameMode::Global),
        _ => bail!("invalid uname mode: {mode} (expected scoped or global)"),
    }
}

#[cfg(feature = "kasumi")]
fn apply_uname(mode: schema::KasumiUnameMode, release: &str, version: &str) -> Result<()> {
    let mut uname = kasumi::KasumiSpoofUname::default();
    if !release.is_empty() {
        uname.set_release(release)?;
    }
    if !version.is_empty() {
        uname.set_version(version)?;
    }

    match mode {
        schema::KasumiUnameMode::Scoped => kasumi::set_uname(&uname),
        schema::KasumiUnameMode::Global => kasumi::set_uname_global(&uname),
    }
}

#[cfg(feature = "kasumi")]
fn display_uname_mode(mode: schema::KasumiUnameMode) -> &'static str {
    match mode {
        schema::KasumiUnameMode::Scoped => "scoped",
        schema::KasumiUnameMode::Global => "global",
    }
}

#[cfg(feature = "kasumi")]
fn build_kasumi_runtime_json(config: &Config, runtime_state: &RuntimeState) -> Result<Value> {
    let kasumi_info = kasumi_mount::collect_runtime_info(config)?;
    Ok(json!({
        "status": kasumi_info.status,
        "available": kasumi_info.available,
        "kernel_supported": kasumi_info.kernel_supported,
        "protocol_version": kasumi_info.protocol_version,
        "feature_bits": kasumi_info.feature_bits,
        "feature_names": kasumi_info.feature_names,
        "hooks": kasumi_info.hooks,
        "rule_count": kasumi_info.rule_count,
        "user_hide_rule_count": kasumi_info.user_hide_rule_count,
        "mirror_path": kasumi_info.mirror_path,
        "lkm": api::build_lkm_payload(config)?,
        "config": config.kasumi.clone(),
        "runtime": {
            "snapshot": &runtime_state.kasumi,
            "kasumi_modules": &runtime_state.kasumi_modules,
            "active_mounts": &runtime_state.active_mounts,
        }
    }))
}

fn to_value<T: Serialize>(payload: &T) -> Result<Value> {
    serde_json::to_value(payload).context("Failed to encode daemon payload")
}

#[cfg(test)]
mod tests {
    use std::sync::Barrier;

    use super::*;

    #[test]
    fn validate_url_accepts_valid_and_rejects_invalid() {
        // Accept http/https
        assert!(validate_url("https://example.com").is_ok());
        assert!(validate_url("http://localhost:8080/path?q=1").is_ok());

        // Reject non-http schemes
        assert!(validate_url("ftp://example.com").is_err());
        assert!(validate_url("javascript:alert(1)").is_err());
        assert!(validate_url("file:///etc/passwd").is_err());

        // Reject flag injection
        assert!(validate_url("https://example.com --es extra value").is_err());

        // Reject control characters
        assert!(validate_url("https://example.com\n").is_err());
        assert!(validate_url("https://example.com\r\n").is_err());
        assert!(validate_url("https://ex\0ample.com").is_err());
    }

    #[test]
    fn concurrent_config_patches_preserve_both_updates() {
        let temp = tempfile::tempdir().unwrap();
        let config_path = temp.path().join("config.toml");
        Config::default().save_to_file(&config_path).unwrap();

        let access = Arc::new(RuntimeConfigAccess::new());
        let barrier = Arc::new(Barrier::new(2));

        let patches = [
            json!({ "disable_umount": true }),
            json!({ "default_mode": "magic" }),
        ];
        let mut threads = Vec::new();
        for patch in patches {
            let access = access.clone();
            let barrier = barrier.clone();
            let config_path = config_path.clone();
            threads.push(std::thread::spawn(move || {
                barrier.wait();
                let _guard = access.lock_writes();
                patch_config_file(&config_path, patch).unwrap();
            }));
        }
        for thread in threads {
            thread.join().unwrap();
        }

        let saved = Config::load_from_file(&config_path).unwrap();
        assert!(saved.disable_umount);
        assert_eq!(saved.default_mode, crate::domain::DefaultMode::Magic);
        assert!(command_writes_config(&DaemonCommand::Config(
            ConfigCommand::Patch {
                patch: json!({}),
                apply_runtime: false,
            }
        )));
    }
}
