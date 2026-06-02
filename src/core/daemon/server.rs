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
    os::{
        fd::AsRawFd,
        unix::{
            fs::PermissionsExt,
            net::{UnixListener, UnixStream},
        },
    },
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

use anyhow::{Context, Result, bail};
use signal_hook::{
    consts::signal::{SIGHUP, SIGINT, SIGTERM},
    flag,
};

use self::http::{ActiveWebuiConnectionGuard, WebuiHttpState};
use super::protocol::{DaemonRequest, DaemonResponse};
use crate::{core::runtime_state::RuntimeState, defs, sys::fs::atomic_write};

mod commands;
mod http;

pub fn serve(config: crate::conf::config::Config) -> Result<()> {
    if config.daemon_startup_mode == crate::conf::schema::DaemonStartupMode::Persistent {
        crate::scoped_log!(
            warn,
            "daemon",
            "daemon_startup_mode=persistent is not supported under the KSU module lifecycle — \
             the service exits once idle regardless of this setting"
        );
    }
    crate::utils::check_ksu();

    fs::create_dir_all(defs::RUN_DIR)
        .with_context(|| format!("Failed to create daemon run directory {}", defs::RUN_DIR))?;
    cleanup_stale_runtime_files()?;
    let listener = UnixListener::bind(defs::SOCKET_FILE)
        .with_context(|| format!("Failed to bind daemon socket {}", defs::SOCKET_FILE))?;
    fs::set_permissions(defs::SOCKET_FILE, fs::Permissions::from_mode(0o600))
        .with_context(|| format!("Failed to set permissions on {}", defs::SOCKET_FILE))?;
    listener
        .set_nonblocking(true)
        .with_context(|| format!("Failed to set {} nonblocking", defs::SOCKET_FILE))?;
    let webui = WebuiHttpState::bind()?;
    let webui_session = webui.session();

    write_pid_file()?;
    let state = Arc::new(Mutex::new(RuntimeState::load().unwrap_or_default()));
    {
        let mut guard = state.lock().expect("daemon state poisoned");
        guard.set_daemon_state(true, defs::SOCKET_FILE);
        guard.save()?;
    }
    let _guard = DaemonRuntimeGuard::new(state.clone());
    let shutdown = install_shutdown_flag()?;
    let config_cache = Arc::new(commands::RuntimeConfigCache::new());

    let active_webui_connections = Arc::new(AtomicUsize::new(0));
    let sse_clients: Arc<Mutex<Vec<std::net::TcpStream>>> = Arc::new(Mutex::new(Vec::new()));

    crate::scoped_log!(
        info,
        "daemon",
        "listening: socket={}, webui={}",
        defs::SOCKET_FILE,
        webui.base_url()
    );

    let unix_fd = listener.as_raw_fd();
    let tcp_fd = webui.listener.as_raw_fd();
    let mut fds = [
        libc::pollfd {
            fd: unix_fd,
            events: libc::POLLIN,
            revents: 0,
        },
        libc::pollfd {
            fd: tcp_fd,
            events: libc::POLLIN,
            revents: 0,
        },
    ];

    while !shutdown.load(Ordering::Relaxed) {
        fds[0].revents = 0;
        fds[1].revents = 0;
        let ret = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, 1000) };
        if ret < 0 {
            let err = IoError::last_os_error();
            if err.kind() == ErrorKind::Interrupted {
                continue;
            }
            return Err(err).context("poll failed in daemon event loop");
        }
        if ret == 0 {
            // timeout – loop back to check shutdown flag
            continue;
        }
        if fds[0].revents & libc::POLLIN != 0 {
            match listener.accept() {
                Ok((mut stream, _addr)) => {
                    if let Err(err) = handle_stream(
                        &config_cache,
                        &state,
                        &shutdown,
                        &webui_session,
                        &sse_clients,
                        &mut stream,
                    ) {
                        crate::scoped_log!(warn, "daemon", "request failed: error={:#}", err);
                        let payload = DaemonResponse::error(format!("{err:#}"));
                        if let Err(e) = write_response(&mut stream, &payload) {
                            crate::scoped_log!(
                                debug,
                                "daemon",
                                "failed to write error response: {:#}",
                                e
                            );
                        }
                    }
                }
                Err(err) if err.kind() == ErrorKind::WouldBlock => {}
                Err(err) => {
                    crate::scoped_log!(warn, "daemon", "accept failed: error={:#}", err);
                }
            }
        }
        if fds[1].revents & libc::POLLIN != 0 {
            match webui.listener.accept() {
                Ok((mut stream, _addr)) => {
                    let Some(connection_guard) =
                        ActiveWebuiConnectionGuard::try_acquire(&active_webui_connections)
                    else {
                        let _ = http::write_http_json(
                            &mut stream,
                            503,
                            "Service Unavailable",
                            &DaemonResponse::error("too many active WebUI daemon connections"),
                            http::ConnectionAction::Close,
                        );
                        continue;
                    };

                    let state = state.clone();
                    let shutdown = shutdown.clone();
                    let session = webui_session.clone();
                    let thread_sse = sse_clients.clone();
                    let thread_config_cache = config_cache.clone();
                    let _ = std::thread::Builder::new()
                        .name("hybrid-mount-webui-rpc".to_string())
                        .spawn(move || {
                            let _connection_guard = connection_guard;
                            if let Err(err) = http::handle_http_connection(
                                &thread_config_cache,
                                &state,
                                &shutdown,
                                &session,
                                thread_sse,
                                stream,
                            ) {
                                crate::scoped_log!(
                                    warn,
                                    "daemon:http",
                                    "request failed: error={:#}",
                                    err
                                );
                            }
                        });
                }
                Err(err) if err.kind() == ErrorKind::WouldBlock => {}
                Err(err) => {
                    crate::scoped_log!(warn, "daemon:http", "accept failed: error={:#}", err);
                }
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

fn handle_stream(
    config_cache: &commands::RuntimeConfigCache,
    state: &Arc<Mutex<RuntimeState>>,
    shutdown: &Arc<AtomicBool>,
    webui: &http::WebuiHttpSession,
    sse_clients: &Arc<Mutex<Vec<std::net::TcpStream>>>,
    stream: &mut UnixStream,
) -> Result<()> {
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
    let effective_config = commands::load_runtime_config(config_cache, &config_path)?;
    let ctx = commands::CommandContext::new(
        &effective_config,
        &config_path,
        config_cache,
        state,
        shutdown,
        webui,
        sse_clients,
    );
    let payload = commands::dispatch_command(&ctx, request.command)?;
    write_response(stream, &DaemonResponse::success(payload))
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

    if !is_pid_process_alive(pid) {
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

fn is_pid_process_alive(pid: i32) -> bool {
    let alive = unsafe { libc::kill(pid, 0) == 0 }
        || IoError::last_os_error().raw_os_error() == Some(libc::EPERM);
    if !alive {
        return false;
    }
    let cmdline_path = format!("/proc/{pid}/cmdline");
    match fs::read_to_string(&cmdline_path) {
        Ok(cmdline) => cmdline.contains("hybrid-mount"),
        Err(_) => true,
    }
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

fn install_shutdown_flag() -> Result<Arc<AtomicBool>> {
    let shutdown = Arc::new(AtomicBool::new(false));
    flag::register(SIGTERM, shutdown.clone()).context("Failed to register SIGTERM handler")?;
    flag::register(SIGINT, shutdown.clone()).context("Failed to register SIGINT handler")?;
    flag::register(SIGHUP, shutdown.clone()).context("Failed to register SIGHUP handler")?;
    Ok(shutdown)
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
