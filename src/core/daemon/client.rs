// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::{
    io::{BufRead, BufReader, Write},
    os::unix::{net::UnixStream, process::CommandExt},
    process::{Command, Stdio},
    thread,
    time::Duration,
};

use anyhow::{Context, Result, bail};
use serde::Serialize;

use super::protocol::{DaemonCommand, DaemonRequest, DaemonResponse};
use crate::{conf::cli::Cli, defs};

pub fn dispatch(cli: &Cli, command: DaemonCommand) -> Result<()> {
    let response = send_request(cli, command)?;
    ensure_ok(&response, "daemon request")?;

    if let Some(payload) = response.data {
        print_json(&payload).context("Failed to print daemon response")?;
    }
    Ok(())
}

fn ensure_ok(response: &DaemonResponse, context: &str) -> Result<()> {
    if !response.ok {
        if let Some(error) = &response.error {
            bail!(error.clone());
        }
        bail!("{context} failed without error message");
    }
    Ok(())
}

fn send_request(cli: &Cli, command: DaemonCommand) -> Result<DaemonResponse> {
    let mut stream = connect_or_start_daemon(cli)?;

    let request = DaemonRequest {
        command,
        config_path: cli.config.clone(),
    };
    let serialized =
        serde_json::to_string(&request).context("Failed to serialize daemon request")?;
    stream
        .write_all(serialized.as_bytes())
        .context("Failed to write daemon request")?;
    stream
        .write_all(b"\n")
        .context("Failed to terminate daemon request")?;
    stream.flush().context("Failed to flush daemon request")?;

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    let bytes = reader
        .read_line(&mut line)
        .context("Failed to read daemon response")?;
    if bytes == 0 {
        bail!("daemon closed the connection without a response");
    }

    serde_json::from_str(line.trim_end()).context("Failed to parse daemon response")
}

fn connect_socket() -> Result<UnixStream> {
    UnixStream::connect(defs::SOCKET_FILE)
        .with_context(|| format!("Failed to connect to daemon socket {}", defs::SOCKET_FILE))
}

fn connect_or_start_daemon(cli: &Cli) -> Result<UnixStream> {
    match connect_socket() {
        Ok(stream) => Ok(stream),
        Err(error) if daemon_is_absent(&error) => start_daemon(cli),
        Err(error) => Err(error),
    }
}

fn daemon_is_absent(error: &anyhow::Error) -> bool {
    error
        .chain()
        .filter_map(|cause| cause.downcast_ref::<std::io::Error>())
        .any(|error| {
            error
                .raw_os_error()
                .is_some_and(|code| code == libc::ENOENT || code == libc::ECONNREFUSED)
        })
}

fn start_daemon(cli: &Cli) -> Result<UnixStream> {
    let current_exe = std::env::current_exe().context("Failed to locate current binary")?;
    let mut command = Command::new(current_exe);
    command.arg("--config").arg(&cli.config);
    command
        .arg("daemon")
        .arg("serve")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    unsafe {
        command.pre_exec(|| {
            if libc::setsid() < 0 {
                return Err(std::io::Error::last_os_error());
            }
            match libc::fork() {
                -1 => Err(std::io::Error::last_os_error()),
                0 => Ok(()),
                _ => libc::_exit(0),
            }
        });
    }

    let mut intermediate = command.spawn().context("Failed to start daemon")?;
    intermediate
        .wait()
        .context("Failed to reap daemon launcher")?;

    let mut last_error = None;
    for _ in 0..30 {
        match connect_socket() {
            Ok(stream) => return Ok(stream),
            Err(error) => last_error = Some(error),
        }
        thread::sleep(Duration::from_millis(100));
    }

    Err(last_error.context("daemon startup produced no connection error")?)
        .context("daemon did not create its socket within 3 seconds")
}

fn print_json<T: Serialize>(payload: &T) -> Result<()> {
    println!(
        "{}",
        serde_json::to_string_pretty(payload).context("Failed to serialize daemon payload")?
    );
    Ok(())
}
