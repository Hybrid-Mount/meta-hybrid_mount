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
    fs,
    io::{BufRead, BufReader, Error as IoError, ErrorKind, Write},
    os::unix::{
        fs::{FileTypeExt, MetadataExt, PermissionsExt},
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use anyhow::{Context, Result, bail};
use serde::Serialize;
use serde_json::{Value, json};
use signal_hook::{
    consts::signal::{SIGHUP, SIGINT, SIGTERM},
    flag,
};

use super::protocol::{DaemonCommand, DaemonRequest, DaemonResponse};
use crate::{
    conf::config::Config,
    core::{api, runtime_state::RuntimeState, user_hide_rules},
    defs,
    mount::kasumi as kasumi_mount,
    sys::{fs::atomic_write, kasumi, lkm},
};

pub fn serve(_config: Config) -> Result<()> {
    cleanup_stale_runtime_files()?;
    let listener = UnixListener::bind(defs::SOCKET_FILE)
        .with_context(|| format!("Failed to bind daemon socket {}", defs::SOCKET_FILE))?;
    fs::set_permissions(defs::SOCKET_FILE, fs::Permissions::from_mode(0o600))
        .with_context(|| format!("Failed to set permissions on {}", defs::SOCKET_FILE))?;
    listener
        .set_nonblocking(true)
        .with_context(|| format!("Failed to set {} nonblocking", defs::SOCKET_FILE))?;

    write_pid_file()?;
    let state = Arc::new(Mutex::new(RuntimeState::load().unwrap_or_default()));
    {
        let mut guard = state.lock().expect("daemon state poisoned");
        guard.set_daemon_state(true, defs::SOCKET_FILE);
        guard.save()?;
    }
    let _guard = DaemonRuntimeGuard::new(state.clone());
    let shutdown = install_shutdown_flag()?;

    crate::scoped_log!(info, "daemon", "listening: socket={}", defs::SOCKET_FILE);

    while !shutdown.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((mut stream, _addr)) => {
                if let Err(err) = handle_stream(&state, &mut stream) {
                    crate::scoped_log!(warn, "daemon", "request failed: error={:#}", err);
                    let payload = DaemonResponse::error(format!("{err:#}"));
                    let _ = write_response(&mut stream, &payload);
                }
            }
            Err(err) if err.kind() == ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(err) => {
                crate::scoped_log!(warn, "daemon", "accept failed: error={:#}", err);
            }
        }
    }

    crate::scoped_log!(
        info,
        "daemon",
        "shutdown requested: socket={}",
        defs::SOCKET_FILE
    );
    Ok(())
}

fn handle_stream(state: &Arc<Mutex<RuntimeState>>, stream: &mut UnixStream) -> Result<()> {
    let mut reader = BufReader::new(
        stream
            .try_clone()
            .context("Failed to clone daemon stream")?,
    );
    let mut line = String::new();
    let bytes = reader
        .read_line(&mut line)
        .context("Failed to read daemon request")?;
    if bytes == 0 {
        bail!("daemon request was empty");
    }

    let request: DaemonRequest =
        serde_json::from_str(line.trim_end()).context("Failed to parse daemon request")?;
    let config_path = request
        .config_path
        .unwrap_or_else(|| PathBuf::from(defs::CONFIG_FILE));
    let effective_config = load_runtime_config(&config_path)?;
    let payload = dispatch_command(&effective_config, &config_path, state, request.command)?;
    write_response(stream, &DaemonResponse::success(payload))
}

fn load_runtime_config(config_path: &Path) -> Result<Config> {
    Config::load_optional_from_file(config_path)
        .with_context(|| format!("Failed to load config from path: {}", config_path.display()))
}

fn dispatch_command(
    config: &Config,
    config_path: &Path,
    state: &Arc<Mutex<RuntimeState>>,
    command: DaemonCommand,
) -> Result<Value> {
    match command {
        DaemonCommand::Ping => to_value(&json!({ "status": "ok" })),
        DaemonCommand::Status => {
            let guard = state.lock().expect("daemon state poisoned");
            to_value(&*guard)
        }
        DaemonCommand::ApiStorage => {
            let guard = state.lock().expect("daemon state poisoned");
            to_value(&api::build_storage_payload(&guard))
        }
        DaemonCommand::ApiMountStats => {
            let guard = state.lock().expect("daemon state poisoned");
            to_value(&api::build_mount_stats_payload(&guard))
        }
        DaemonCommand::ApiMountTopology => {
            let guard = state.lock().expect("daemon state poisoned");
            to_value(&api::build_mount_topology_payload(config, &guard))
        }
        DaemonCommand::ApiPartitions => to_value(&api::build_partitions_payload(config)),
        DaemonCommand::ApiSystemInfo => {
            let guard = state.lock().expect("daemon state poisoned");
            to_value(&api::build_system_info_payload(&guard))
        }
        DaemonCommand::ApiVersion => to_value(&api::build_version_payload()),
        DaemonCommand::ApiConfigGet => to_value(config),
        DaemonCommand::ApiConfigSet { config: payload } => {
            let config: Config =
                serde_json::from_value(payload).context("Failed to decode config payload")?;
            config.save_to_file(config_path)?;
            to_value(&json!({ "saved": true }))
        }
        DaemonCommand::ApiModulesList { path } => {
            let guard = state.lock().expect("daemon state poisoned");
            to_value(&api::build_modules_payload(
                config,
                &guard,
                path.as_deref(),
            )?)
        }
        DaemonCommand::ApiModulesApply { modules } => {
            to_value(&api::apply_modules_payload(config_path, &modules)?)
        }
        DaemonCommand::ApiLkm => to_value(&api::build_lkm_payload(config)),
        DaemonCommand::ApiHooks => {
            kasumi_mount::require_live(config, "read Kasumi hooks")?;
            to_value(&kasumi_mount::hook_lines()?)
        }
        DaemonCommand::KasumiStatus => {
            let runtime_state = state.lock().expect("daemon state poisoned").clone();
            let kasumi_info = kasumi_mount::collect_runtime_info(config);
            to_value(&json!({
                "status": kasumi_info.status,
                "available": kasumi_info.available,
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
                    "snapshot": runtime_state.kasumi,
                    "kasumi_modules": runtime_state.kasumi_modules,
                    "active_mounts": runtime_state.active_mounts,
                }
            }))
        }
        DaemonCommand::KasumiList => {
            let payload = if kasumi_mount::can_operate(config) {
                api::parse_kasumi_rule_listing(&kasumi::list_rules()?)
            } else {
                Vec::new()
            };
            to_value(&payload)
        }
        DaemonCommand::KasumiVersion => {
            let guard = state.lock().expect("daemon state poisoned");
            to_value(&api::build_kasumi_version_payload(config, &guard))
        }
        DaemonCommand::KasumiFeatures => to_value(&api::build_features_payload()),
        DaemonCommand::KasumiHooks => to_value(&kasumi_mount::hook_lines()?),
        DaemonCommand::KasumiApplyConfigRuntime => {
            let applied = kasumi_mount::apply_runtime_config(config)?;
            kasumi::invalidate_status_cache();
            refresh_runtime_snapshot(config, state)?;
            to_value(&json!({ "applied": applied }))
        }
        DaemonCommand::HideList => to_value(&user_hide_rules::load_user_hide_rules()?),
        DaemonCommand::HideAdd { path } => {
            let added = user_hide_rules::add_user_hide_rule(&path)?;
            if added && kasumi_mount::can_operate(config) {
                kasumi::hide_path(&path)?;
            }
            refresh_runtime_snapshot(config, state)?;
            to_value(&json!({ "added": added, "path": path }))
        }
        DaemonCommand::HideRemove { path } => {
            let removed = user_hide_rules::remove_user_hide_rule(&path)?;
            refresh_runtime_snapshot(config, state)?;
            to_value(&json!({ "removed": removed, "path": path }))
        }
        DaemonCommand::HideApply => {
            kasumi_mount::require_live(config, "apply user hide rules")?;
            let (applied, failed) = user_hide_rules::apply_user_hide_rules()?;
            refresh_runtime_snapshot(config, state)?;
            to_value(&json!({ "applied": applied, "failed": failed }))
        }
        DaemonCommand::LkmStatus => to_value(&api::build_lkm_payload(config)),
        DaemonCommand::LkmLoad => {
            lkm::load(&config.kasumi)?;
            kasumi::invalidate_status_cache();
            refresh_runtime_snapshot(config, state)?;
            to_value(&json!({ "message": "Kasumi LKM loaded." }))
        }
        DaemonCommand::LkmUnload => {
            lkm::unload(&config.kasumi)?;
            kasumi::invalidate_status_cache();
            refresh_runtime_snapshot(config, state)?;
            to_value(&json!({ "message": "Kasumi LKM unloaded." }))
        }
        DaemonCommand::KasumiClear => {
            kasumi::clear_rules()?;
            refresh_runtime_snapshot(config, state)?;
            to_value(&json!({ "message": "Kasumi rules cleared." }))
        }
        DaemonCommand::KasumiReleaseConnection => {
            kasumi::release_connection();
            refresh_runtime_snapshot(config, state)?;
            to_value(&json!({ "message": "Released cached Kasumi client connection." }))
        }
        DaemonCommand::KasumiInvalidateCache => {
            kasumi::invalidate_status_cache();
            refresh_runtime_snapshot(config, state)?;
            to_value(&json!({ "message": "Invalidated cached Kasumi status." }))
        }
        DaemonCommand::KasumiFixMounts => {
            kasumi::fix_mounts()?;
            refresh_runtime_snapshot(config, state)?;
            to_value(&json!({ "message": "Kasumi mount ordering fixed." }))
        }
        DaemonCommand::KasumiRestoreUnameGlobal => {
            kasumi::restore_uname_global()?;
            refresh_runtime_snapshot(config, state)?;
            to_value(&json!({ "message": "Kasumi global uname restored." }))
        }
        DaemonCommand::KasumiSetUname {
            mode,
            release,
            version,
        } => {
            let mode = parse_uname_mode(&mode)?;
            apply_uname(mode, &release, &version)?;
            refresh_runtime_snapshot(config, state)?;
            to_value(&json!({
                "message": "Kasumi uname applied.",
                "mode": display_uname_mode(mode),
                "release": release,
                "version": version,
            }))
        }
        DaemonCommand::KasumiClearUname { mode } => {
            let mode = parse_uname_mode(&mode)?;
            match mode {
                KasumiUnameMode::Scoped => apply_uname(KasumiUnameMode::Scoped, "", "")?,
                KasumiUnameMode::Global => kasumi::restore_uname_global()?,
            }
            refresh_runtime_snapshot(config, state)?;
            to_value(&json!({
                "message": "Kasumi uname cleared.",
                "mode": display_uname_mode(mode),
            }))
        }
        DaemonCommand::KasumiRuleAdd {
            target,
            source,
            file_type,
        } => {
            let file_type = file_type.unwrap_or(detect_rule_file_type(&source)?);
            kasumi::add_rule(&target, &source, file_type)?;
            refresh_runtime_snapshot(config, state)?;
            to_value(&json!({
                "message": "Kasumi ADD rule applied.",
                "target": target,
                "source": source,
                "file_type": file_type,
            }))
        }
        DaemonCommand::KasumiRuleMerge { target, source } => {
            kasumi::add_merge_rule(&target, &source)?;
            refresh_runtime_snapshot(config, state)?;
            to_value(&json!({
                "message": "Kasumi MERGE rule applied.",
                "target": target,
                "source": source,
            }))
        }
        DaemonCommand::KasumiRuleHide { path } => {
            kasumi::hide_path(&path)?;
            refresh_runtime_snapshot(config, state)?;
            to_value(&json!({
                "message": "Kasumi HIDE rule applied.",
                "path": path,
            }))
        }
        DaemonCommand::KasumiRuleDelete { path } => {
            kasumi::delete_rule(&path)?;
            refresh_runtime_snapshot(config, state)?;
            to_value(&json!({
                "message": "Kasumi rule deleted.",
                "path": path,
            }))
        }
        DaemonCommand::KasumiRuleAddDir {
            target_base,
            source_dir,
        } => {
            kasumi::add_rules_from_directory(&target_base, &source_dir)?;
            refresh_runtime_snapshot(config, state)?;
            to_value(&json!({
                "message": "Kasumi directory rules applied.",
                "target_base": target_base,
                "source_dir": source_dir,
            }))
        }
        DaemonCommand::KasumiRuleRemoveDir {
            target_base,
            source_dir,
        } => {
            kasumi::remove_rules_from_directory(&target_base, &source_dir)?;
            refresh_runtime_snapshot(config, state)?;
            to_value(&json!({
                "message": "Kasumi directory rules removed.",
                "target_base": target_base,
                "source_dir": source_dir,
            }))
        }
    }
}

fn write_response(stream: &mut UnixStream, response: &DaemonResponse) -> Result<()> {
    let serialized =
        serde_json::to_string(response).context("Failed to serialize daemon response")?;
    stream
        .write_all(serialized.as_bytes())
        .context("Failed to write daemon response")?;
    stream
        .write_all(b"\n")
        .context("Failed to terminate daemon response")?;
    stream.flush().context("Failed to flush daemon response")
}

fn cleanup_stale_runtime_files() -> Result<()> {
    cleanup_stale_pid_file()?;
    cleanup_stale_socket(Path::new(defs::SOCKET_FILE))?;
    mark_state_stopped_if_offline()?;
    Ok(())
}

fn cleanup_stale_socket(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    match UnixStream::connect(path) {
        Ok(_) => bail!("daemon socket already active at {}", path.display()),
        Err(_) => {
            fs::remove_file(path)
                .with_context(|| format!("Failed to remove stale socket {}", path.display()))?;
            Ok(())
        }
    }
}

fn cleanup_stale_pid_file() -> Result<()> {
    let Ok(raw) = fs::read_to_string(defs::PID_FILE) else {
        return Ok(());
    };
    let pid = raw.trim().parse::<i32>().ok();
    let Some(pid) = pid else {
        fs::remove_file(defs::PID_FILE)
            .with_context(|| format!("Failed to remove invalid pid file {}", defs::PID_FILE))?;
        return Ok(());
    };

    let alive = unsafe { libc::kill(pid, 0) == 0 }
        || IoError::last_os_error().raw_os_error() == Some(libc::EPERM);
    if !alive {
        match fs::remove_file(defs::PID_FILE) {
            Ok(()) => {}
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            Err(err) => {
                return Err(err).with_context(|| {
                    format!("Failed to remove stale pid file {}", defs::PID_FILE)
                });
            }
        }
    }
    Ok(())
}

fn mark_state_stopped_if_offline() -> Result<()> {
    let mut state = RuntimeState::load().unwrap_or_default();
    if !state.daemon.alive {
        return Ok(());
    }
    state.set_daemon_state(false, "");
    state.save()
}

fn write_pid_file() -> Result<()> {
    atomic_write(
        defs::PID_FILE,
        format!("{}\n", std::process::id()).as_bytes(),
    )
    .with_context(|| format!("Failed to write pid file {}", defs::PID_FILE))
}

fn refresh_runtime_snapshot(config: &Config, state: &Arc<Mutex<RuntimeState>>) -> Result<()> {
    let mut guard = state.lock().expect("daemon state poisoned");
    guard.kasumi = kasumi_mount::collect_runtime_info(config);
    guard.set_daemon_state(true, defs::SOCKET_FILE);
    guard.save()
}

fn to_value<T: Serialize>(payload: &T) -> Result<Value> {
    serde_json::to_value(payload).context("Failed to encode daemon payload")
}

fn install_shutdown_flag() -> Result<Arc<AtomicBool>> {
    let shutdown = Arc::new(AtomicBool::new(false));
    flag::register(SIGTERM, shutdown.clone()).context("Failed to register SIGTERM handler")?;
    flag::register(SIGINT, shutdown.clone()).context("Failed to register SIGINT handler")?;
    flag::register(SIGHUP, shutdown.clone()).context("Failed to register SIGHUP handler")?;
    Ok(shutdown)
}

fn parse_uname_mode(mode: &str) -> Result<KasumiUnameMode> {
    match mode {
        "scoped" => Ok(KasumiUnameMode::Scoped),
        "global" => Ok(KasumiUnameMode::Global),
        _ => bail!("invalid uname mode: {mode} (expected scoped or global)"),
    }
}

fn apply_uname(mode: KasumiUnameMode, release: &str, version: &str) -> Result<()> {
    let mut uname = kasumi::KasumiSpoofUname::default();
    if !release.is_empty() {
        uname.set_release(release)?;
    }
    if !version.is_empty() {
        uname.set_version(version)?;
    }

    match mode {
        KasumiUnameMode::Scoped => kasumi::set_uname(&uname),
        KasumiUnameMode::Global => kasumi::set_uname_global(&uname),
    }
}

fn display_uname_mode(mode: KasumiUnameMode) -> &'static str {
    match mode {
        KasumiUnameMode::Scoped => "scoped",
        KasumiUnameMode::Global => "global",
    }
}

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

struct DaemonRuntimeGuard {
    state: Arc<Mutex<RuntimeState>>,
}

impl DaemonRuntimeGuard {
    fn new(state: Arc<Mutex<RuntimeState>>) -> Self {
        Self { state }
    }
}

impl Drop for DaemonRuntimeGuard {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.state.lock() {
            guard.set_daemon_state(false, "");
            let _ = guard.save();
        }
        let _ = fs::remove_file(defs::PID_FILE);
        let _ = fs::remove_file(defs::SOCKET_FILE);
    }
}

#[derive(Clone, Copy)]
enum KasumiUnameMode {
    Scoped,
    Global,
}
