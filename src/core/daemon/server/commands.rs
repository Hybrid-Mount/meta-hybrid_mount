// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

#[cfg(feature = "kasumi")]
use std::os::unix::fs::FileTypeExt;
use std::{
    collections::HashMap,
    fs,
    net::TcpStream,
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

#[cfg(feature = "kasumi")]
use super::super::protocol::KasumiCommand;
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
#[cfg(feature = "kasumi")]
use crate::{
    conf::schema,
    core::user_hide_rules,
    mount::kasumi as kasumi_mount,
    sys::{kasumi, lkm},
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
}

impl RuntimeConfigCache {
    pub(super) fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
        }
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
    sse_clients: &'a Arc<Mutex<Vec<TcpStream>>>,
}

impl<'a> CommandContext<'a> {
    pub(super) fn new(
        config: &'a Config,
        config_path: &'a Path,
        config_cache: &'a RuntimeConfigCache,
        state: &'a Arc<Mutex<RuntimeState>>,
        shutdown: &'a Arc<AtomicBool>,
        webui: &'a WebuiHttpSession,
        sse_clients: &'a Arc<Mutex<Vec<TcpStream>>>,
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
        kasumi::invalidate_status_cache();
        self.refresh_message(message)
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
    match command {
        DaemonCommand::System(cmd) => dispatch_system(ctx, cmd),
        DaemonCommand::Config(cmd) => dispatch_config(ctx, cmd),
        DaemonCommand::Modules(cmd) => dispatch_modules(ctx, cmd),
        #[cfg(feature = "kasumi")]
        DaemonCommand::Kasumi(cmd) => dispatch_kasumi(ctx, cmd),
        DaemonCommand::Batch(BatchCommand::Batch { commands }) => dispatch_batch(ctx, commands),
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
            http::broadcast_sse_event(state, sse_clients, "mount_errors_cleared");
            to_value(&json!({ "cleared": cleared, "removed_markers": removed_markers }))
        }
    }
}

fn dispatch_init(ctx: &CommandContext<'_>) -> Result<Value> {
    let config = ctx.config;
    let state = ctx.state;

    let (status_value, snapshot) = cached_status_and_snapshot(state)?;
    let mut config_value = to_value(config)?;
    // When Kasumi is not usable (kernel too old, LKM not present, or
    // protocol mismatch) force kasumi.enabled = false.  When the Kasumi
    // feature is not even compiled in (lite/nano builds), strip the
    // entire [kasumi] section from the reported config.  Both paths
    // ensure the WebUI never shows a non-existent Kasumi mode — even
    // when switching from a full to lite build with a stale WebUI.
    #[cfg(feature = "kasumi")]
    {
        if (!config.kasumi.enabled || !kasumi::can_operate())
            && let Some(kasumi_val) = config_value.get_mut("kasumi")
            && let Some(enabled) = kasumi_val.get_mut("enabled")
        {
            *enabled = json!(false);
        }
    }
    #[cfg(not(feature = "kasumi"))]
    {
        // Strip the entire [kasumi] section from the reported config so
        // the WebUI never sees any kasumi data after switching from a
        // full to a lite build.  (Forcing just enabled=false leaves the
        // rest of the section intact, which can confuse the WebUI when
        // ENABLE_KASUMI is accidentally true in a stale WebUI bundle.)
        config_value.as_object_mut().map(|obj| obj.remove("kasumi"));
    }
    let version_value = to_value(&api::build_version_payload())?;
    let system_info_value = to_value(&api::build_system_info_payload(&snapshot))?;
    #[cfg(feature = "kasumi")]
    {
        let kasumi_status_value = build_kasumi_runtime_json(config, &snapshot)?;
        to_value(&json!({
            "status": status_value,
            "config": config_value,
            "version": version_value,
            "kasumi_status": kasumi_status_value,
            "system_info": system_info_value,
        }))
    }
    #[cfg(not(feature = "kasumi"))]
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
            ctx.refresh(&config, json!({ "saved": true, "config": &config }))
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
            ctx.cache_config(config.clone())?;
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

// ── Kasumi commands ─────────────────────────────────────────────────────

#[cfg(feature = "kasumi")]
fn dispatch_kasumi(ctx: &CommandContext<'_>, cmd: KasumiCommand) -> Result<Value> {
    let config = ctx.config;
    let config_path = ctx.config_path;
    let config_cache = ctx.config_cache;
    let state = ctx.state;

    match cmd {
        KasumiCommand::Status => {
            let snapshot = runtime_snapshot(state);
            build_kasumi_runtime_json(config, &snapshot)
        }
        KasumiCommand::List => {
            let payload = if kasumi_mount::can_operate(config) {
                api::parse_kasumi_rule_listing(&kasumi::list_rules()?)
            } else {
                Vec::new()
            };
            to_value(&payload)
        }
        KasumiCommand::Version => {
            let snapshot = runtime_snapshot(state);
            to_value(&api::build_kasumi_version_payload(config, &snapshot))
        }
        KasumiCommand::Features => to_value(&api::build_features_payload()),
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
            kasumi::release_connection();
            ctx.refresh_message("Released cached Kasumi client connection.")
        }
        KasumiCommand::InvalidateCache => {
            kasumi::invalidate_status_cache();
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
            let file_type = file_type.unwrap_or(detect_rule_file_type(&source)?);
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
            if added && kasumi_mount::can_operate(config) {
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
            let (applied, failed) = user_hide_rules::apply_user_hide_rules()?;
            ctx.refresh_current(json!({ "applied": applied, "failed": failed }))
        }
        KasumiCommand::LkmStatus => to_value(&api::build_lkm_payload(config)),
        KasumiCommand::LkmLoad => {
            lkm::load(&config.kasumi)?;
            ctx.invalidate_and_refresh_message("Kasumi LKM loaded.")
        }
        KasumiCommand::LkmUnload => {
            lkm::unload(&config.kasumi)?;
            ctx.invalidate_and_refresh_message("Kasumi LKM unloaded.")
        }
        KasumiCommand::ApiLkm => to_value(&api::build_lkm_payload(config)),
        KasumiCommand::ApiHooks => {
            kasumi_mount::require_live(config, "read Kasumi hooks")?;
            to_value(&kasumi_mount::hook_lines()?)
        }
        KasumiCommand::MapsAdd { rule } => {
            let updated = add_kasumi_maps_config_rule(config_path, rule)?;
            ctx.cache_config(updated.clone())?;
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
            let mut updated = load_runtime_config(config_cache, config_path)?
                .as_ref()
                .clone();
            updated.kasumi.maps_rules.clear();
            updated.save_to_file(config_path)?;
            ctx.cache_config(updated.clone())?;
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

// ── Batch commands ──────────────────────────────────────────────────────

fn dispatch_batch(ctx: &CommandContext<'_>, commands: Vec<DaemonCommand>) -> Result<Value> {
    let noop_clients = Arc::new(Mutex::new(Vec::new()));
    let batch_ctx = CommandContext::new(
        ctx.config,
        ctx.config_path,
        ctx.config_cache,
        ctx.state,
        ctx.shutdown,
        ctx.webui,
        &noop_clients,
    );
    let mut results: Vec<Value> = Vec::with_capacity(commands.len());
    for cmd in commands {
        let result = match dispatch_command(&batch_ctx, cmd) {
            Ok(value) => json!({ "ok": true, "data": value }),
            Err(err) => json!({ "ok": false, "error": format!("{err}") }),
        };
        results.push(result);
    }
    ctx.refresh_runtime_snapshot(ctx.config)?;
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
    kasumi::invalidate_status_cache();
    Ok(applied)
}

#[cfg(not(feature = "kasumi"))]
fn apply_runtime_config(_config: &Config) -> Result<bool> {
    Ok(false)
}

fn refresh_runtime_snapshot(
    config: &Config,
    state: &Arc<Mutex<RuntimeState>>,
    sse_clients: &Arc<Mutex<Vec<TcpStream>>>,
) -> Result<()> {
    let mut guard = lock_or_recover(state);
    #[cfg(feature = "kasumi")]
    {
        guard.kasumi = kasumi_mount::collect_runtime_info(config);
    }
    #[cfg(not(feature = "kasumi"))]
    {
        let _ = config;
    }
    guard.set_daemon_state(true, defs::SOCKET_FILE);
    guard
        .status_value()
        .map_err(|e| anyhow::anyhow!("Failed to cache status value: {e}"))?;
    guard.save()?;
    drop(guard);
    http::broadcast_sse_event(state, sse_clients, "runtime_changed");
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
fn detect_rule_file_type(path: &Path) -> Result<i32> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to read source metadata for {}", path.display()))?;
    let file_type = metadata.file_type();

    if file_type.is_char_device() && metadata.rdev() == 0 {
        bail!(
            "source {} is a whiteout node; use `kasumi rule hide` instead",
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

#[cfg(feature = "kasumi")]
fn build_kasumi_runtime_json(config: &Config, runtime_state: &RuntimeState) -> Result<Value> {
    let kasumi_info = kasumi_mount::collect_runtime_info(config);
    to_value(&json!({
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
        "lkm": api::build_lkm_payload(config),
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
}
