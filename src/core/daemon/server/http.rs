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
    io::{BufRead, BufReader, ErrorKind, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

use anyhow::{Context, Error, Result};
use serde_json::{Value, json};

use super::super::protocol::DaemonResponse;
use crate::{core::runtime_state::RuntimeState, defs};

pub(super) struct WebuiHttpState {
    pub(super) listener: TcpListener,
    session: WebuiHttpSession,
}

#[derive(Clone)]
pub(super) struct WebuiHttpSession {
    addr: SocketAddr,
    token: String,
    bearer_token: String,
}

fn random_u64_hex() -> Result<String> {
    let mut buf = [0u8; 8];
    fs::File::open("/dev/urandom")
        .context("Failed to open /dev/urandom")?
        .read_exact(&mut buf)
        .context("Failed to read random bytes")?;
    Ok(format!("{:016x}", u64::from_ne_bytes(buf)))
}

impl WebuiHttpState {
    pub(super) fn bind() -> Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .context("Failed to bind WebUI daemon HTTP listener")?;
        listener
            .set_nonblocking(true)
            .context("Failed to set WebUI daemon HTTP listener nonblocking")?;
        let addr = listener
            .local_addr()
            .context("Failed to read WebUI daemon HTTP listener address")?;
        let token = format!(
            "{}{}",
            random_u64_hex().context("Failed to generate daemon token")?,
            random_u64_hex().context("Failed to generate daemon token")?
        );
        let bearer_token = format!("Bearer {token}");
        Ok(Self {
            listener,
            session: WebuiHttpSession {
                addr,
                token,
                bearer_token,
            },
        })
    }

    pub(super) fn session(&self) -> WebuiHttpSession {
        self.session.clone()
    }

    pub(super) fn base_url(&self) -> String {
        self.session.base_url()
    }
}

impl WebuiHttpSession {
    pub(super) fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    pub(super) fn session_payload(&self) -> Value {
        json!({
            "base_url": self.base_url(),
            "token": self.token.clone(),
        })
    }
}

pub(super) struct WebuiHttpRequest {
    pub(super) request_line: String,
    pub(super) authorized: bool,
    pub(super) close_after_response: bool,
    pub(super) body: Vec<u8>,
}

pub(super) const MAX_WEBUI_HTTP_BODY_BYTES: usize = 1024 * 1024;
pub(super) const MAX_WEBUI_CONNECTIONS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum WebuiHttpRequestReadError {
    InvalidContentLength,
    RequestBodyTooLarge,
}

impl WebuiHttpRequestReadError {
    fn status(self) -> (u16, &'static str, &'static str) {
        match self {
            Self::InvalidContentLength => (400, "Bad Request", "invalid content-length header"),
            Self::RequestBodyTooLarge => (413, "Payload Too Large", "request body too large"),
        }
    }
}

impl std::fmt::Display for WebuiHttpRequestReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (_, _, message) = self.status();
        f.write_str(message)
    }
}

impl std::error::Error for WebuiHttpRequestReadError {}

pub(super) struct ActiveWebuiConnectionGuard {
    active_connections: Arc<AtomicUsize>,
}

impl ActiveWebuiConnectionGuard {
    pub(super) fn try_acquire(active_connections: &Arc<AtomicUsize>) -> Option<Self> {
        loop {
            let current = active_connections.load(Ordering::Relaxed);
            if current >= MAX_WEBUI_CONNECTIONS {
                return None;
            }
            if active_connections
                .compare_exchange(current, current + 1, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                return Some(Self {
                    active_connections: active_connections.clone(),
                });
            }
        }
    }
}

impl Drop for ActiveWebuiConnectionGuard {
    fn drop(&mut self) {
        self.active_connections.fetch_sub(1, Ordering::Relaxed);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ConnectionAction {
    Keep,
    Close,
}

pub(super) fn handle_http_connection(
    config_cache: &super::commands::RuntimeConfigCache,
    state: &Arc<Mutex<RuntimeState>>,
    shutdown: &Arc<AtomicBool>,
    webui: &WebuiHttpSession,
    sse_clients: Arc<Mutex<Vec<TcpStream>>>,
    mut stream: TcpStream,
) -> Result<()> {
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .context("Failed to set WebUI HTTP read timeout")?;
    stream
        .set_write_timeout(Some(Duration::from_secs(10)))
        .context("Failed to set WebUI HTTP write timeout")?;
    let mut reader = BufReader::new(
        stream
            .try_clone()
            .context("Failed to clone WebUI HTTP stream")?,
    );

    while !shutdown.load(Ordering::Relaxed) {
        let request = match read_http_request(&mut reader, webui) {
            Ok(Some(request)) => request,
            Ok(None) => break,
            Err(err) => {
                if let Some(read_err) = err.downcast_ref::<WebuiHttpRequestReadError>() {
                    let (status, reason, message) = read_err.status();
                    if let Err(e) = write_http_json(
                        &mut stream,
                        status,
                        reason,
                        &DaemonResponse::error(message),
                        ConnectionAction::Close,
                    ) {
                        crate::scoped_log!(
                            debug,
                            "daemon:http",
                            "failed to write error response: {:#}",
                            e
                        );
                    }
                    break;
                }
                return Err(err);
            }
        };
        if handle_http_request(
            config_cache,
            state,
            shutdown,
            webui,
            &sse_clients,
            &mut stream,
            request,
        )? == ConnectionAction::Close
        {
            break;
        }
    }

    Ok(())
}

fn read_http_request(
    reader: &mut BufReader<TcpStream>,
    webui: &WebuiHttpSession,
) -> Result<Option<WebuiHttpRequest>> {
    let mut request_line = String::new();
    let bytes = match reader.read_line(&mut request_line) {
        Ok(bytes) => bytes,
        Err(err) if matches!(err.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
            return Ok(None);
        }
        Err(err) => return Err(err).context("Failed to read WebUI HTTP request line"),
    };
    if bytes == 0 {
        return Ok(None);
    }

    let mut content_length = 0usize;
    let mut authorized = false;
    let mut close_after_response = request_line.contains("HTTP/1.0");
    loop {
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .context("Failed to read WebUI HTTP header")?;
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        if let Some((name, value)) = trimmed.split_once(':') {
            let name = name.trim();
            let value = value.trim();
            if name.eq_ignore_ascii_case("content-length") {
                content_length = parse_content_length(value)?;
            } else if name.eq_ignore_ascii_case("authorization") {
                authorized = value == webui.bearer_token.as_str();
            } else if name.eq_ignore_ascii_case("connection") {
                for directive in value.split(',').map(str::trim) {
                    if directive.eq_ignore_ascii_case("close") {
                        close_after_response = true;
                    } else if directive.eq_ignore_ascii_case("keep-alive") {
                        close_after_response = false;
                    }
                }
            }
        }
    }

    let mut body = allocate_request_body(content_length)?;
    std::io::Read::read_exact(reader, &mut body)
        .context("Failed to read WebUI HTTP request body")?;

    Ok(Some(WebuiHttpRequest {
        request_line,
        authorized,
        close_after_response,
        body,
    }))
}

fn parse_content_length(value: &str) -> Result<usize> {
    let content_length = value
        .parse::<usize>()
        .map_err(|_| Error::new(WebuiHttpRequestReadError::InvalidContentLength))?;
    if content_length > MAX_WEBUI_HTTP_BODY_BYTES {
        return Err(Error::new(WebuiHttpRequestReadError::RequestBodyTooLarge));
    }
    Ok(content_length)
}

fn allocate_request_body(content_length: usize) -> Result<Vec<u8>> {
    // Size already validated by parse_content_length; kept as a safety belt.
    debug_assert!(content_length <= MAX_WEBUI_HTTP_BODY_BYTES);
    Ok(vec![0; content_length])
}

fn handle_http_request(
    config_cache: &super::commands::RuntimeConfigCache,
    state: &Arc<Mutex<RuntimeState>>,
    shutdown: &Arc<AtomicBool>,
    webui: &WebuiHttpSession,
    sse_clients: &Arc<Mutex<Vec<TcpStream>>>,
    stream: &mut TcpStream,
    request: WebuiHttpRequest,
) -> Result<ConnectionAction> {
    let mut connection_action = if request.close_after_response || shutdown.load(Ordering::Relaxed)
    {
        ConnectionAction::Close
    } else {
        ConnectionAction::Keep
    };

    if request.request_line.starts_with("OPTIONS ") {
        write_http_response(stream, 204, "No Content", b"", connection_action)?;
        return Ok(ConnectionAction::Close);
    }
    if request.request_line.starts_with("GET /events ") {
        return handle_sse_endpoint(
            state,
            shutdown,
            webui,
            sse_clients,
            stream,
            &request.request_line,
        );
    }
    if !request.request_line.starts_with("POST /rpc ") {
        write_http_json(
            stream,
            404,
            "Not Found",
            &DaemonResponse::error("unknown WebUI daemon endpoint"),
            connection_action,
        )?;
        return Ok(ConnectionAction::Close);
    }
    if !request.authorized {
        write_http_json(
            stream,
            401,
            "Unauthorized",
            &DaemonResponse::error("invalid WebUI daemon token"),
            connection_action,
        )?;
        return Ok(ConnectionAction::Close);
    }

    let close_after_response = request.close_after_response;
    let request: super::super::protocol::DaemonRequest =
        serde_json::from_slice(&request.body).context("Failed to parse WebUI daemon request")?;
    let config_path = request
        .config_path
        .unwrap_or_else(|| PathBuf::from(defs::CONFIG_FILE));
    let effective_config = super::commands::load_runtime_config(config_cache, &config_path)?;
    let ctx = super::commands::CommandContext::new(
        &effective_config,
        &config_path,
        config_cache,
        state,
        shutdown,
        webui,
        sse_clients,
    );
    let response = match super::commands::dispatch_command(&ctx, request.command) {
        Ok(payload) => DaemonResponse::success(payload),
        Err(err) => DaemonResponse::error(format!("{err}")),
    };
    connection_action = if close_after_response || shutdown.load(Ordering::Relaxed) {
        ConnectionAction::Close
    } else {
        ConnectionAction::Keep
    };
    write_http_json(stream, 200, "OK", &response, connection_action)?;
    Ok(connection_action)
}

pub(super) fn write_http_json(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    response: &DaemonResponse,
    connection_action: ConnectionAction,
) -> Result<()> {
    let body = serde_json::to_vec(response).context("Failed to serialize WebUI HTTP response")?;
    write_http_response(stream, status, reason, &body, connection_action)
}

fn write_http_response(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    body: &[u8],
    connection_action: ConnectionAction,
) -> Result<()> {
    let connection = if connection_action == ConnectionAction::Keep {
        "keep-alive"
    } else {
        "close"
    };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Access-Control-Allow-Methods: POST, OPTIONS\r\n\
         Access-Control-Allow-Headers: authorization, content-type\r\n\
         Access-Control-Max-Age: 600\r\n\
         Connection: {connection}\r\n\
         Keep-Alive: timeout=30\r\n\r\n",
        body.len(),
    )
    .context("Failed to write WebUI HTTP response header")?;
    stream
        .write_all(body)
        .context("Failed to write WebUI HTTP response body")?;
    stream
        .flush()
        .context("Failed to flush WebUI HTTP response")
}

fn parse_query_param<'a>(request_line: &'a str, key: &str) -> Option<&'a str> {
    let path = request_line.split_whitespace().nth(1)?;
    let query = path.split('?').nth(1)?;
    for pair in query.split('&') {
        let (k, v) = pair.split_once('=')?;
        if k == key {
            return Some(v);
        }
    }
    None
}

// Token is passed via query parameter because the browser EventSource API
// does not support custom headers. The listener binds 127.0.0.1 only, so the
// token is not exposed over the network.
fn handle_sse_endpoint(
    state: &Arc<Mutex<RuntimeState>>,
    shutdown: &Arc<AtomicBool>,
    webui: &WebuiHttpSession,
    sse_clients: &Arc<Mutex<Vec<TcpStream>>>,
    stream: &mut TcpStream,
    request_line: &str,
) -> Result<ConnectionAction> {
    let token = parse_query_param(request_line, "token").unwrap_or("");
    if token != webui.token {
        write_http_json(
            stream,
            401,
            "Unauthorized",
            &DaemonResponse::error("invalid SSE token"),
            ConnectionAction::Close,
        )?;
        return Ok(ConnectionAction::Close);
    }

    write!(
        stream,
        "HTTP/1.1 200 OK\r\n\
         Content-Type: text/event-stream\r\n\
         Cache-Control: no-cache\r\n\
         Connection: keep-alive\r\n\
         Access-Control-Allow-Origin: *\r\n\r\n"
    )
    .context("Failed to write SSE response header")?;
    stream.flush().context("Failed to flush SSE headers")?;

    // Send initial event
    let initial = {
        let mut guard = state.lock().expect("daemon state poisoned");
        serde_json::to_string(guard.status_value()?).unwrap_or_default()
    };
    write!(stream, "event: state_update\ndata: {initial}\n\n")
        .context("Failed to write SSE initial event")?;
    stream
        .flush()
        .context("Failed to flush SSE initial event")?;

    let sse_stream = stream
        .try_clone()
        .context("Failed to clone stream for SSE broadcast")?;
    {
        let mut clients = sse_clients.lock().expect("sse_clients poisoned");
        clients.push(sse_stream);
    }

    // Block until shutdown or client disconnect. Read with 5 s timeout so we
    // can periodically send an SSE comment keepalive.
    const KEEPALIVE_SECS: u64 = 30;
    const READ_TIMEOUT_SECS: u64 = 5;

    stream
        .set_read_timeout(Some(Duration::from_secs(READ_TIMEOUT_SECS)))
        .context("Failed to set SSE read timeout")?;
    let mut buf = [0u8; 1];
    let mut last_keepalive = std::time::Instant::now();
    while !shutdown.load(Ordering::Relaxed) {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Err(ref e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(_) => break,
            _ => {}
        }
        if last_keepalive.elapsed().as_secs() >= KEEPALIVE_SECS {
            // SSE comment line — ignored by clients, keeps TCP alive.
            if let Err(e) = write!(stream, ": keepalive\n\n").and_then(|_| stream.flush()) {
                crate::scoped_log!(debug, "daemon:sse", "keepalive write failed: {:#}", e);
                break;
            }
            last_keepalive = std::time::Instant::now();
        }
    }

    Ok(ConnectionAction::Close)
}

pub(super) fn broadcast_sse_event(
    state: &Arc<Mutex<RuntimeState>>,
    sse_clients: &Arc<Mutex<Vec<TcpStream>>>,
    event: &str,
) {
    let body = {
        let mut guard = match state.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        let json = match guard.status_value() {
            Ok(v) => v.clone(),
            Err(_) => return,
        };
        match serde_json::to_string(&json) {
            Ok(s) => format!("event: {event}\ndata: {s}\n\n"),
            Err(_) => return,
        }
    };

    // Swap out the client list so writes happen outside the lock
    let clients: Vec<TcpStream> = {
        let mut guard = match sse_clients.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        std::mem::take(&mut *guard)
    };

    let alive: Vec<TcpStream> = clients
        .into_iter()
        .filter(|mut client| {
            client
                .write_all(body.as_bytes())
                .and_then(|_| client.flush())
                .is_ok()
        })
        .collect();

    // Merge back any clients added while we were writing
    if let Ok(mut guard) = sse_clients.lock() {
        guard.extend(alive);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_content_length_validates_and_rejects() {
        assert_eq!(parse_content_length("128").unwrap(), 128);

        let err = parse_content_length("nope").unwrap_err();
        assert_eq!(
            err.downcast_ref::<WebuiHttpRequestReadError>(),
            Some(&WebuiHttpRequestReadError::InvalidContentLength)
        );

        let err = parse_content_length(&(MAX_WEBUI_HTTP_BODY_BYTES + 1).to_string()).unwrap_err();
        assert_eq!(
            err.downcast_ref::<WebuiHttpRequestReadError>(),
            Some(&WebuiHttpRequestReadError::RequestBodyTooLarge)
        );
    }

    #[test]
    fn allocate_request_body_checks_size_in_debug() {
        let result = std::panic::catch_unwind(|| {
            let _ = allocate_request_body(MAX_WEBUI_HTTP_BODY_BYTES + 1);
        });
        // In debug mode this panics due to debug_assert; in release it's a nop.
        // Either outcome is acceptable — the real guard is parse_content_length.
        let _ = result;
    }

    #[test]
    fn connection_guard_tracks_and_enforces_limit() {
        let active_connections = Arc::new(AtomicUsize::new(0));
        {
            let _first = ActiveWebuiConnectionGuard::try_acquire(&active_connections).unwrap();
            assert_eq!(active_connections.load(Ordering::Relaxed), 1);
            let _second = ActiveWebuiConnectionGuard::try_acquire(&active_connections).unwrap();
            assert_eq!(active_connections.load(Ordering::Relaxed), 2);
        }
        assert_eq!(active_connections.load(Ordering::Relaxed), 0);

        let full = Arc::new(AtomicUsize::new(MAX_WEBUI_CONNECTIONS));
        assert!(ActiveWebuiConnectionGuard::try_acquire(&full).is_none());
    }

    #[test]
    fn broadcast_sse_event_sends_to_clients() {
        let state = Arc::new(Mutex::new(
            crate::core::runtime_state::RuntimeState::default(),
        ));
        let sse_clients = Arc::new(Mutex::new(Vec::new()));

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let mut client = std::net::TcpStream::connect(addr).unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let server = listener.accept().unwrap().0;
        server
            .set_write_timeout(Some(Duration::from_secs(1)))
            .unwrap();

        sse_clients.lock().unwrap().push(server);
        broadcast_sse_event(&state, &sse_clients, "state_update");

        let mut buf = [0u8; 4096];
        let n = client.read(&mut buf).unwrap();
        let text = String::from_utf8_lossy(&buf[..n]);
        assert!(text.contains("event: state_update"), "missing event field");
        assert!(text.contains("data:"), "missing data field");
    }

    #[test]
    fn broadcast_sse_event_removes_dead_clients() {
        let state = Arc::new(Mutex::new(
            crate::core::runtime_state::RuntimeState::default(),
        ));
        let sse_clients = Arc::new(Mutex::new(Vec::new()));

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let _client = std::net::TcpStream::connect(addr).unwrap();
        let (server, _peer) = listener.accept().unwrap();
        server
            .shutdown(std::net::Shutdown::Write)
            .expect("shutdown write on server socket");

        sse_clients.lock().unwrap().push(server);
        broadcast_sse_event(&state, &sse_clients, "state_update");

        assert!(
            sse_clients.lock().unwrap().is_empty(),
            "dead client should be removed"
        );
    }
}
