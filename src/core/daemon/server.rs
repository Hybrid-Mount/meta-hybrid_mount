// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

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
    time::Duration,
};

use anyhow::{Context, Result, bail};
use signal_hook::{
    consts::signal::{SIGHUP, SIGINT, SIGTERM},
    flag,
};

use self::http::{ActiveWebuiConnectionGuard, WebuiHttpState};
use super::protocol::{DaemonRequest, DaemonResponse};
use crate::{conf::config::Config, core::runtime_state::RuntimeState, defs, sys::fs::atomic_write};

mod commands;
mod http;

const MAX_DAEMON_REQUEST_BYTES: usize = 1024 * 1024;
const DAEMON_STREAM_TIMEOUT: Duration = Duration::from_secs(5);

pub fn serve() -> Result<()> {
    crate::utils::check_ksu();

    fs::create_dir_all(defs::RUN_DIR)
        .with_context(|| format!("Failed to create daemon run directory {}", defs::RUN_DIR))?;
    cleanup_stale_runtime_files()?;
    let config = Config::load_from_file(defs::CONFIG_FILE).unwrap_or_else(|error| {
        crate::scoped_log!(
            warn,
            "daemon",
            "config load failed, using defaults for idle state: error={:#}",
            error
        );
        Config::default()
    });
    let mut runtime_state = match RuntimeState::load() {
        Ok(state) => normalize_runtime_state(state, &config),
        Err(err) if error_is_not_found(&err) => {
            crate::scoped_log!(
                warn,
                "daemon",
                "runtime state missing, starting idle: path={}",
                defs::STATE_FILE
            );
            idle_runtime_state(&config)
        }
        Err(err) => {
            return Err(err).with_context(|| {
                format!(
                    "Failed to load daemon runtime state: path={}",
                    defs::STATE_FILE
                )
            });
        }
    };
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
    runtime_state.set_daemon_state(true, defs::SOCKET_FILE)?;
    runtime_state.save()?;
    let state = Arc::new(Mutex::new(runtime_state));
    let mut runtime_guard = DaemonRuntimeGuard::new(state.clone());
    let shutdown = install_shutdown_flag()?;
    let config_access = Arc::new(commands::RuntimeConfigAccess::new());

    let active_webui_connections = Arc::new(AtomicUsize::new(0));
    let sse_clients = http::SseClientRegistry::shared();

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
                        &config_access,
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
                Err(err) => return Err(err).context("daemon socket accept failed"),
            }
        }
        if fds[1].revents & libc::POLLIN != 0 {
            match webui.listener.accept() {
                Ok((mut stream, _addr)) => {
                    let Some(connection_guard) =
                        ActiveWebuiConnectionGuard::try_acquire(&active_webui_connections)
                    else {
                        http::write_http_json(
                            &mut stream,
                            503,
                            "Service Unavailable",
                            &DaemonResponse::error("too many active WebUI daemon connections"),
                            http::ConnectionAction::Close,
                        )?;
                        continue;
                    };

                    let state = state.clone();
                    let shutdown = shutdown.clone();
                    let session = webui_session.clone();
                    let thread_sse = sse_clients.clone();
                    let thread_config_access = config_access.clone();
                    std::thread::Builder::new()
                        .name("hybrid-mount-webui-rpc".to_string())
                        .spawn(move || {
                            let _connection_guard = connection_guard;
                            if let Err(err) = http::handle_http_connection(
                                &thread_config_access,
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
                        })
                        .context("Failed to spawn WebUI RPC worker")?;
                }
                Err(err) if err.kind() == ErrorKind::WouldBlock => {}
                Err(err) => return Err(err).context("WebUI socket accept failed"),
            }
        }
    }

    crate::scoped_log!(
        info,
        "daemon",
        "shutdown requested: socket={}",
        defs::SOCKET_FILE
    );
    runtime_guard.cleanup()?;
    Ok(())
}

fn idle_runtime_state(config: &Config) -> RuntimeState {
    RuntimeState::idle(
        config.overlay_mode.clone().as_str(),
        PathBuf::from(defs::HYBRID_MOUNT_DIR),
    )
}

fn normalize_runtime_state(state: RuntimeState, config: &Config) -> RuntimeState {
    if state.has_valid_mount_identity() {
        return state;
    }

    crate::scoped_log!(
        warn,
        "daemon",
        "runtime state is invalid, starting idle: storage_mode={}, mount_point={}",
        state.storage_mode,
        state.mount_point.display()
    );
    idle_runtime_state(config)
}

fn handle_stream(
    config_access: &commands::RuntimeConfigAccess,
    state: &Arc<Mutex<RuntimeState>>,
    shutdown: &Arc<AtomicBool>,
    webui: &http::WebuiHttpSession,
    sse_clients: &http::SharedSseClients,
    stream: &mut UnixStream,
) -> Result<()> {
    stream
        .set_read_timeout(Some(DAEMON_STREAM_TIMEOUT))
        .context("Failed to set daemon request read timeout")?;
    stream
        .set_write_timeout(Some(DAEMON_STREAM_TIMEOUT))
        .context("Failed to set daemon response write timeout")?;
    let mut reader = BufReader::new(
        stream
            .try_clone()
            .context("Failed to clone daemon stream")?,
    );
    let Some(mut line) = read_limited_request_line(&mut reader)? else {
        bail!("daemon request was empty");
    };
    while matches!(line.last(), Some(b'\r' | b'\n')) {
        line.pop();
    }

    let request: DaemonRequest =
        serde_json::from_slice(&line).context("Failed to parse daemon request")?;
    let config_path = request.config_path;
    let effective_config = commands::load_runtime_config(config_access, &config_path)?;
    let ctx = commands::CommandContext::new(
        &effective_config,
        &config_path,
        config_access,
        state,
        shutdown,
        webui,
        sse_clients,
    );
    let payload = commands::dispatch_command(&ctx, request.command)?;
    write_response(stream, &DaemonResponse::success(payload))
}

fn read_limited_request_line<R: BufRead>(reader: &mut R) -> Result<Option<Vec<u8>>> {
    let mut line = Vec::new();
    loop {
        let available = reader.fill_buf().context("Failed to read daemon request")?;
        if available.is_empty() {
            return if line.is_empty() {
                Ok(None)
            } else {
                Ok(Some(line))
            };
        }

        let newline = available.iter().position(|byte| *byte == b'\n');
        let take_len = newline.map_or(available.len(), |index| index + 1);
        if line.len() + take_len > MAX_DAEMON_REQUEST_BYTES {
            bail!(
                "daemon request exceeds maximum size of {} bytes",
                MAX_DAEMON_REQUEST_BYTES
            );
        }

        line.extend_from_slice(&available[..take_len]);
        reader.consume(take_len);
        if newline.is_some() {
            return Ok(Some(line));
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
    Ok(())
}

fn error_is_not_found(error: &anyhow::Error) -> bool {
    error
        .chain()
        .filter_map(|cause| cause.downcast_ref::<IoError>())
        .any(|error| error.kind() == ErrorKind::NotFound)
}

fn cleanup_stale_socket(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    match UnixStream::connect(path) {
        Ok(_) => bail!("daemon socket already active at {}", path.display()),
        Err(err) if err.raw_os_error() == Some(libc::ECONNREFUSED) => {
            crate::scoped_log!(
                debug,
                "daemon:server",
                "removing stale socket: {}",
                path.display()
            );
            fs::remove_file(path)
                .with_context(|| format!("Failed to remove stale socket {}", path.display()))?;
            Ok(())
        }
        Err(err) => {
            Err(err).with_context(|| format!("Failed to inspect daemon socket {}", path.display()))
        }
    }
}

fn cleanup_stale_pid_file() -> Result<()> {
    let raw = match fs::read_to_string(defs::PID_FILE) {
        Ok(raw) => raw,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
        Err(err) => {
            return Err(err).with_context(|| format!("Failed to read pid file {}", defs::PID_FILE));
        }
    };
    let pid = raw
        .trim()
        .parse::<i32>()
        .with_context(|| format!("Invalid pid file {}", defs::PID_FILE))?;

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
        Err(err) => {
            crate::scoped_log!(
                debug,
                "daemon:server",
                "cannot read cmdline for pid {} (expected if process ended): {}",
                pid,
                err
            );
            true
        }
    }
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
    active: bool,
}

impl DaemonRuntimeGuard {
    fn new(state: Arc<Mutex<RuntimeState>>) -> Self {
        Self {
            state,
            active: true,
        }
    }

    fn cleanup(&mut self) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime state lock is poisoned during daemon cleanup"))?;
        state.set_daemon_state(false, "")?;
        state.save()?;
        drop(state);
        remove_runtime_file(defs::PID_FILE)?;
        remove_runtime_file(defs::SOCKET_FILE)?;
        self.active = false;
        Ok(())
    }
}

fn remove_runtime_file(path: &str) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("Failed to remove runtime file {path}")),
    }
}

impl Drop for DaemonRuntimeGuard {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        if let Err(error) = self.cleanup() {
            crate::scoped_log!(error, "daemon", "daemon cleanup failed: {:#}", error);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_runtime_state_is_normalized_for_webui() {
        let mut config = Config::default();
        config.overlay_mode = crate::conf::config::OverlayMode::Tmpfs;

        let state = normalize_runtime_state(RuntimeState::default(), &config);

        assert!(state.has_valid_mount_identity());
        assert_eq!(state.storage_mode, "tmpfs");
        assert_eq!(state.mount_point, PathBuf::from(defs::HYBRID_MOUNT_DIR));
        assert!(!state.mounted);
    }

    #[test]
    fn valid_fallback_state_preserves_actual_storage_mode() {
        let mut config = Config::default();
        config.overlay_mode = crate::conf::config::OverlayMode::Tmpfs;
        let persisted = RuntimeState::idle("ext4", PathBuf::from("/actual-mount"));

        let state = normalize_runtime_state(persisted, &config);

        assert_eq!(state.storage_mode, "ext4");
        assert_eq!(state.mount_point, PathBuf::from("/actual-mount"));
    }

    #[test]
    fn missing_runtime_state_error_is_recoverable() {
        let error = anyhow::Error::from(IoError::from(ErrorKind::NotFound));

        assert!(error_is_not_found(&error));
    }

    #[test]
    fn malformed_runtime_state_error_is_not_recoverable() {
        let error = anyhow::anyhow!("invalid runtime state");

        assert!(!error_is_not_found(&error));
    }

    #[test]
    fn limited_daemon_request_reader_accepts_one_line() {
        let mut reader = std::io::BufReader::new(std::io::Cursor::new(b"{\"type\":\"ping\"}\n"));
        let line = read_limited_request_line(&mut reader).unwrap().unwrap();
        assert_eq!(line, b"{\"type\":\"ping\"}\n");
    }

    #[test]
    fn limited_daemon_request_reader_rejects_oversized_input() {
        let input = vec![b'x'; MAX_DAEMON_REQUEST_BYTES + 1];
        let mut reader = std::io::BufReader::new(std::io::Cursor::new(input));
        assert!(read_limited_request_line(&mut reader).is_err());
    }
}
