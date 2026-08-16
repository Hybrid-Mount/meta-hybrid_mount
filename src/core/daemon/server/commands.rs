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

use std::{
    collections::HashMap,
    fs,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
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
    super::protocol::{BatchCommand, ConfigCommand, DaemonCommand, ModulesCommand, SystemCommand},
    http::{self, WebuiHttpSession},
};
use crate::{
    conf::config::Config,
    core::{api, inventory, runtime_state::RuntimeState},
    defs,
    utils::{self, lock_or_recover},
};

#[derive(Clone, PartialEq, Eq)]
enum ConfigFileStamp {
    Missing,
    Present {
        dev: u64,
        ino: u64,
        len: u64,
        mtime_sec: i64,
        mtime_nsec: i64,
        ctime_sec: i64,
        ctime_nsec: i64,
    },
}

struct CachedRuntimeConfig {
    stamp: ConfigFileStamp,
    config: Arc<Config>,
}

pub(super) struct RuntimeConfigCache {
    entries: Mutex<HashMap<PathBuf, CachedRuntimeConfig>>,
    write_lock: Mutex<()>,
}

impl RuntimeConfigCache {
    pub(super) fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            write_lock: Mutex::new(()),
        }
    }

    fn lock_writes(&self) -> std::sync::MutexGuard<'_, ()> {
        lock_or_recover(&self.write_lock)
    }

    pub(super) fn load(&self, config_path: &Path) -> Result<Arc<Config>> {
        let stamp = config_file_stamp(config_path)?;
        let key = config_path.to_path_buf();
        let mut entries = lock_or_recover(&self.entries);

        if let Some(entry) = entries.get(&key)
            && entry.stamp == stamp
        {
            return Ok(entry.config.clone());
        }

        let config = Arc::new(load_runtime_config_uncached(config_path)?);
        entries.insert(
            key,
            CachedRuntimeConfig {
                stamp,
                config: config.clone(),
            },
        );
        Ok(config)
    }

    pub(super) fn store(&self, config_path: &Path, config: Config) -> Result<Arc<Config>> {
        let stamp = config_file_stamp(config_path)?;
        let config = Arc::new(config);
        lock_or_recover(&self.entries).insert(
            config_path.to_path_buf(),
            CachedRuntimeConfig {
                stamp,
                config: config.clone(),
            },
        );
        Ok(config)
    }
}

fn config_file_stamp(config_path: &Path) -> Result<ConfigFileStamp> {
    match fs::metadata(config_path) {
        Ok(metadata) => Ok(ConfigFileStamp::Present {
            dev: metadata.dev(),
            ino: metadata.ino(),
            len: metadata.len(),
            mtime_sec: metadata.mtime(),
            mtime_nsec: metadata.mtime_nsec(),
            ctime_sec: metadata.ctime(),
            ctime_nsec: metadata.ctime_nsec(),
        }),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(ConfigFileStamp::Missing),
        Err(err) => Err(err)
            .with_context(|| format!("Failed to inspect config file {}", config_path.display())),
    }
}

pub(super) fn load_runtime_config(
    config_cache: &RuntimeConfigCache,
    config_path: &Path,
) -> Result<Arc<Config>> {
    config_cache.load(config_path)
}

fn load_runtime_config_uncached(config_path: &Path) -> Result<Config> {
    Config::load_optional_from_file(config_path)
        .with_context(|| format!("Failed to load config from path: {}", config_path.display()))
}

pub(super) struct CommandContext<'a> {
    config: &'a Config,
    config_path: &'a Path,
    config_cache: &'a RuntimeConfigCache,
    state: &'a Arc<Mutex<RuntimeState>>,
    shutdown: &'a Arc<AtomicBool>,
    webui: &'a WebuiHttpSession,
    sse_clients: &'a http::SharedSseClients,
}

impl<'a> CommandContext<'a> {
    pub(super) fn new(
        config: &'a Config,
        config_path: &'a Path,
        config_cache: &'a RuntimeConfigCache,
        state: &'a Arc<Mutex<RuntimeState>>,
        shutdown: &'a Arc<AtomicBool>,
        webui: &'a WebuiHttpSession,
        sse_clients: &'a http::SharedSseClients,
    ) -> Self {
        Self {
            config,
            config_path,
            config_cache,
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

    fn refresh_runtime_snapshot(&self, config: &Config) -> Result<()> {
        refresh_runtime_snapshot(config, self.state, self.sse_clients)
    }

    fn cache_config(&self, config: Config) -> Result<Arc<Config>> {
        self.config_cache.store(self.config_path, config)
    }
}

fn runtime_snapshot(state: &Arc<Mutex<RuntimeState>>) -> RuntimeState {
    lock_or_recover(state).clone()
}

fn cached_status_value(state: &Arc<Mutex<RuntimeState>>) -> Result<Value> {
    let mut guard = lock_or_recover(state);
    Ok(guard.status_value()?.clone())
}

fn cached_status_and_snapshot(state: &Arc<Mutex<RuntimeState>>) -> Result<(Value, RuntimeState)> {
    let mut guard = lock_or_recover(state);
    let status_value = guard.status_value()?.clone();
    Ok((status_value, guard.clone()))
}

// ── Top-level dispatch ──────────────────────────────────────────────────

pub(super) fn dispatch_command(ctx: &CommandContext<'_>, command: DaemonCommand) -> Result<Value> {
    let _write_guard = command_writes_config(&command).then(|| ctx.config_cache.lock_writes());
    dispatch_command_unlocked(ctx, command)
}

fn dispatch_command_unlocked(ctx: &CommandContext<'_>, command: DaemonCommand) -> Result<Value> {
    match command {
        DaemonCommand::System(cmd) => dispatch_system(ctx, cmd),
        DaemonCommand::Config(cmd) => dispatch_config(ctx, cmd),
        DaemonCommand::Modules(cmd) => dispatch_modules(ctx, cmd),
        DaemonCommand::Batch(BatchCommand::Batch { commands }) => dispatch_batch(ctx, commands),
    }
}

fn command_writes_config(command: &DaemonCommand) -> bool {
    match command {
        DaemonCommand::Config(ConfigCommand::Get) => false,
        DaemonCommand::Config(_) | DaemonCommand::Modules(ModulesCommand::Apply { .. }) => true,
        DaemonCommand::Modules(ModulesCommand::List { .. }) | DaemonCommand::System(_) => false,
        DaemonCommand::Batch(BatchCommand::Batch { commands }) => {
            commands.iter().any(command_writes_config)
        }
    }
}

// ── System commands ─────────────────────────────────────────────────────

fn dispatch_system(ctx: &CommandContext<'_>, cmd: SystemCommand) -> Result<Value> {
    let config = ctx.config;
    let state = ctx.state;
    let shutdown = ctx.shutdown;
    let webui = ctx.webui;
    let sse_clients = ctx.sse_clients;

    match cmd {
        SystemCommand::Ping => to_value(&json!({ "status": "ok" })),
        SystemCommand::WebuiStart => Ok(webui.session_payload()),
        SystemCommand::Shutdown => {
            shutdown.store(true, Ordering::Relaxed);
            to_value(&json!({ "shutdown": true }))
        }
        SystemCommand::Init => dispatch_init(ctx),
        SystemCommand::Status => cached_status_value(state),
        SystemCommand::ApiStorage => {
            let snapshot = runtime_snapshot(state);
            to_value(&api::build_storage_payload(&snapshot))
        }
        SystemCommand::ApiMountStats => {
            let snapshot = runtime_snapshot(state);
            to_value(&api::build_mount_stats_payload(&snapshot))
        }
        SystemCommand::ApiMountTopology => {
            let snapshot = runtime_snapshot(state);
            to_value(&api::build_mount_topology_payload(config, &snapshot))
        }
        SystemCommand::ApiPartitions => to_value(&api::build_partitions_payload(config)),
        SystemCommand::ApiSystemInfo => {
            let snapshot = runtime_snapshot(state);
            to_value(&api::build_system_info_payload(&snapshot))
        }
        SystemCommand::ApiVersion => to_value(&api::build_version_payload()),
        SystemCommand::ApiKernelUname => to_value(&read_kernel_uname_payload()?),
        SystemCommand::ApiOpenUrl { url } => {
            open_url(&url)?;
            to_value(&json!({ "opened": true }))
        }
        SystemCommand::ApiReboot => {
            reboot_device()?;
            to_value(&json!({ "reboot": true }))
        }
        SystemCommand::ClearMountErrors => {
            let removed_markers = clear_mount_error_markers(config)?;
            let mut guard = lock_or_recover(state);
            let cleared = guard.mount_error_modules.len();
            guard.mount_error_modules.clear();
            guard.mount_error_reasons.clear();
            guard.save()?;
            drop(guard);
            http::broadcast_sse_event(state, sse_clients, "state_update");
            to_value(&json!({ "cleared": cleared, "removed_markers": removed_markers }))
        }
    }
}

fn dispatch_init(ctx: &CommandContext<'_>) -> Result<Value> {
    let config = ctx.config;
    let state = ctx.state;

    let (status_value, snapshot) = cached_status_and_snapshot(state)?;
    let config_value = to_value(config)?;
    let version_value = to_value(&api::build_version_payload())?;
    let system_info_value = to_value(&api::build_system_info_payload(&snapshot))?;

    to_value(&json!({
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
            ctx.cache_config(config.clone())?;
            ctx.refresh(&config, config_update_payload(&config, false))
        }
        ConfigCommand::Patch {
            patch,
            apply_runtime,
        } => {
            let config = patch_config_file(config_path, patch)?;
            ctx.cache_config(config.clone())?;
            let applied = apply_runtime
                .then(|| apply_runtime_config(&config))
                .transpose()?
                .unwrap_or(false);
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
            ctx.cache_config(config.clone())?;
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
        ModulesCommand::List { path } => {
            let snapshot = runtime_snapshot(state);
            to_value(&api::build_modules_payload(
                config,
                &snapshot,
                path.as_deref(),
            )?)
        }
        ModulesCommand::Apply { modules } => {
            let payload = api::apply_modules_payload(config_path, &modules)?;
            let config = load_runtime_config_uncached(config_path)?;
            ctx.cache_config(config.clone())?;
            ctx.refresh(&config, payload)
        }
    }
}

// ── Batch commands ──────────────────────────────────────────────────────

fn dispatch_batch(ctx: &CommandContext<'_>, commands: Vec<DaemonCommand>) -> Result<Value> {
    let noop_clients = http::SseClientRegistry::shared();
    let mut results: Vec<Value> = Vec::with_capacity(commands.len());
    for cmd in commands {
        // Reload between commands so a read following a write in the same
        // batch observes the configuration that was just persisted.
        let effective_config = load_runtime_config(ctx.config_cache, ctx.config_path)?;
        let batch_ctx = CommandContext::new(
            &effective_config,
            ctx.config_path,
            ctx.config_cache,
            ctx.state,
            ctx.shutdown,
            ctx.webui,
            &noop_clients,
        );
        // The outer batch holds the config write lock when any nested command
        // writes configuration, so recursive dispatch must not acquire it again.
        let result = match dispatch_command_unlocked(&batch_ctx, cmd) {
            Ok(value) => json!({ "ok": true, "data": value }),
            Err(err) => json!({ "ok": false, "error": format!("{err}") }),
        };
        results.push(result);
    }
    let effective_config = load_runtime_config(ctx.config_cache, ctx.config_path)?;
    ctx.refresh_runtime_snapshot(&effective_config)?;
    to_value(&json!({ "results": results }))
}

fn patch_config_file(config_path: &Path, patch: Value) -> Result<Config> {
    let config = load_runtime_config_uncached(config_path)?;
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
    to_value(&json!({ "release": release, "version": version }))
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

fn clear_mount_error_markers(config: &Config) -> Result<usize> {
    let entries = match fs::read_dir(&config.moduledir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(err) => {
            return Err(err)
                .with_context(|| format!("failed to read {}", config.moduledir.display()));
        }
    };

    let mut removed = 0usize;
    for entry in entries {
        let entry = entry.with_context(|| {
            format!(
                "failed to enumerate module directory {}",
                config.moduledir.display()
            )
        })?;
        if !entry
            .file_type()
            .with_context(|| format!("failed to inspect {}", entry.path().display()))?
            .is_dir()
        {
            continue;
        }

        let id = entry.file_name().to_string_lossy().into_owned();
        if inventory::is_reserved_module_dir(&id) {
            continue;
        }

        let marker_dir = entry.path();
        removed +=
            utils::remove_dir_entries_case_insensitive(&marker_dir, defs::MOUNT_ERROR_FILE_NAME)
                .with_context(|| {
                    format!("failed to remove marker under {}", marker_dir.display())
                })?;
    }

    Ok(removed)
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
    _config: &Config,
    state: &Arc<Mutex<RuntimeState>>,
    sse_clients: &http::SharedSseClients,
) -> Result<()> {
    let mut guard = lock_or_recover(state);
    guard.set_daemon_state(true, defs::SOCKET_FILE);
    guard
        .status_value()
        .map_err(|e| anyhow::anyhow!("Failed to cache status value: {e}"))?;
    guard.save()?;
    drop(guard);
    http::broadcast_sse_event(state, sse_clients, "state_update");
    Ok(())
}

fn to_value<T: Serialize>(payload: &T) -> Result<Value> {
    serde_json::to_value(payload).context("Failed to encode daemon payload")
}

#[cfg(test)]
mod tests {
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
    fn clear_mount_error_markers_removes_marker_files() {
        let temp = tempfile::tempdir().unwrap();
        let module_dir = temp.path().join("broken");
        fs::create_dir_all(&module_dir).unwrap();
        let marker = module_dir.join("MOUNT_ERROR");
        fs::write(&marker, b"").unwrap();

        let config = Config {
            moduledir: temp.path().to_path_buf(),
            ..Default::default()
        };

        assert_eq!(clear_mount_error_markers(&config).unwrap(), 1);
        assert!(!marker.exists());
        assert_eq!(clear_mount_error_markers(&config).unwrap(), 0);
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
}
