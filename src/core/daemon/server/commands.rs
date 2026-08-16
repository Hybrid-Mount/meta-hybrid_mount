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

use super::{
    super::protocol::{ConfigCommand, DaemonCommand, ModulesCommand, SystemCommand},
    http::{self, WebuiHttpSession},
};
use crate::{
    conf::config::Config,
    core::{api, runtime_state::RuntimeState},
    defs,
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

    fn refresh<T: Serialize>(&self, _config: &Config, payload: T) -> Result<Value> {
        self.refresh_runtime_snapshot()?;
        to_value(&payload)
    }

    fn refresh_runtime_snapshot(&self) -> Result<()> {
        refresh_runtime_snapshot(self.state, self.sse_clients)
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
    }
}

fn command_writes_config(command: &DaemonCommand) -> bool {
    match command {
        DaemonCommand::Config(ConfigCommand::Get) => false,
        DaemonCommand::Config(_) | DaemonCommand::Modules(ModulesCommand::Apply { .. }) => true,
        DaemonCommand::Modules(ModulesCommand::List) | DaemonCommand::System(_) => false,
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
            ctx.refresh(&config, config_update_payload(&config, false))
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
            if apply_runtime && !applied {
                crate::scoped_log!(
                    warn,
                    "daemon:config",
                    "runtime apply requested but unsupported; changes require a reboot"
                );
            }
            ctx.refresh(&config, config_update_payload(&config, applied))
        }
        ConfigCommand::Reset => {
            let config = Config::default();
            save_and_apply_runtime_config(&config, config_path)?;
            ctx.refresh(&config, config_update_payload(&config, false))
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

fn save_and_apply_runtime_config(config: &Config, config_path: &Path) -> Result<bool> {
    config.save_to_file(config_path)?;
    apply_runtime_config(config)
}

/// Runtime configuration application was removed together with the Kasumi
/// backend. OverlayFS and Magic Mount plans are built during boot, so config
/// changes are persisted for the next boot and cannot be applied live.
/// The `apply_runtime` protocol flag is retained for API compatibility and
/// always reports `false` here; callers should use `reboot_required`.
fn apply_runtime_config(_config: &Config) -> Result<bool> {
    Ok(false)
}

fn config_update_payload(config: &Config, applied: bool) -> Value {
    json!({
        "saved": true,
        "applied": applied,
        "reboot_required": !applied,
        "config": &config,
    })
}

fn refresh_runtime_snapshot(
    state: &Arc<Mutex<RuntimeState>>,
    sse_clients: &http::SharedSseClients,
) -> Result<()> {
    let mut guard = state
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime state lock is poisoned"))?;
    let daemon_changed = !guard.daemon.alive || guard.daemon.socket_path != defs::SOCKET_FILE;
    guard.set_daemon_state(true, defs::SOCKET_FILE)?;
    guard
        .status_value()
        .map_err(|e| anyhow::anyhow!("Failed to cache status value: {e}"))?;
    if daemon_changed {
        guard.save()?;
    }
    drop(guard);
    http::broadcast_sse_event(state, sse_clients, "runtime_changed")?;
    Ok(())
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
    fn config_patch_folds_legacy_kasumi_mode_into_magic() {
        let temp = tempfile::tempdir().unwrap();
        let config_path = temp.path().join("config.toml");
        Config::default().save_to_file(&config_path).unwrap();

        let config = patch_config_file(
            &config_path,
            json!({
                "default_mode": "kasumi",
                "kasumi": { "enabled": true },
            }),
        )
        .unwrap();

        assert_eq!(config.default_mode, crate::domain::DefaultMode::Magic);
        assert!(
            !serde_json::to_value(config)
                .unwrap()
                .as_object()
                .unwrap()
                .contains_key("kasumi")
        );
    }

    #[test]
    fn config_update_payload_reports_reboot_requirement() {
        let config = Config::default();

        let unsupported = config_update_payload(&config, false);
        assert_eq!(unsupported["saved"], json!(true));
        assert_eq!(unsupported["applied"], json!(false));
        assert_eq!(unsupported["reboot_required"], json!(true));
        assert_eq!(unsupported["config"]["default_mode"], json!("overlay"));

        let applied = config_update_payload(&config, true);
        assert_eq!(applied["applied"], json!(true));
        assert_eq!(applied["reboot_required"], json!(false));
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
