// SPDX-License-Identifier: GPL-3.0-only

//! The shared subprocess runner used on production paths.
//!
//! Design constraints:
//! - No shell string assembly: the program and its arguments are passed separately.
//! - stdout/stderr keep only a bounded head + tail, and draining continues past the cap,
//!   so large output cannot OOM us or block the child on a full pipe.
//! - The total timeout is independent of the I/O drain timeout: after the direct child is
//!   killed, grandchildren can inherit and hold the stdout/stderr pipes open, so the drain thread needs its own deadline.
//! - Every call site must declare its acceptable exit codes; `ExitPolicy::Any` is only for
//!   known cases like insmod, where the exit code carries no result and only the side effect is authoritative.
//! - Errors and logs never carry environment contents; KernelSU/APatch module names go only into the child's env.

use std::fmt;
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus as StdExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;

/// Default total bytes kept per stream, split evenly between head and tail.
pub const DEFAULT_OUTPUT_CAPACITY_BYTES: usize = 32 * 1024;
/// Default deadline for waiting on the output drain threads once the direct child exits.
pub const DEFAULT_DRAIN_TIMEOUT: Duration = Duration::from_secs(2);
/// Polling interval for waiting on the child.
const WAIT_POLL_INTERVAL: Duration = Duration::from_millis(10);
/// Pipe read chunk size. Far below the cap, so the buffer stays bounded.
const READ_CHUNK_BYTES: usize = 8 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputStream {
    Stdout,
    Stderr,
}

impl fmt::Display for OutputStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stdout => f.write_str("stdout"),
            Self::Stderr => f.write_str("stderr"),
        }
    }
}

/// Cross-platform child exit status. `Signaled` is never constructed on non-Unix targets,
/// but keeping the variant spares the error-classification and logging code a platform cfg.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExitStatus {
    Exited(i32),
    Signaled(i32),
}

impl ExitStatus {
    fn from_std(status: StdExitStatus) -> Self {
        #[cfg(unix)]
        if let Some(signal) = status.signal() {
            return Self::Signaled(signal);
        }
        Self::Exited(status.code().unwrap_or(-1))
    }
}

impl fmt::Display for ExitStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exited(code) => write!(f, "exit code {code}"),
            Self::Signaled(signal) => write!(f, "signal {signal}"),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CaptureMode {
    /// Allocates no output buffer; stdout/stderr go straight to a `/dev/null` equivalent.
    #[default]
    None,
    Stdout,
    Stderr,
    Both,
}

impl CaptureMode {
    const fn captures_stdout(self) -> bool {
        matches!(self, Self::Stdout | Self::Both)
    }

    const fn captures_stderr(self) -> bool {
        matches!(self, Self::Stderr | Self::Both)
    }
}

/// Explicitly declares the acceptable exit codes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExitPolicy {
    /// Accepts exit code 0 only.
    Success,
    /// Accepts only the listed codes, such as 0..=3 for `e2fsck`.
    Accepted(&'static [i32]),
    /// The exit code carries no result and only the side-effect check is authoritative (LKM `insmod` deliberately returns -EAGAIN).
    Any,
}

impl ExitPolicy {
    fn accepts(self, status: ExitStatus) -> bool {
        match self {
            Self::Success => status == ExitStatus::Exited(0),
            Self::Accepted(codes) => {
                matches!(status, ExitStatus::Exited(code) if codes.contains(&code))
            }
            Self::Any => true,
        }
    }
}

/// Bounded head + tail output buffer.
///
/// Keeps the first `head_budget` bytes and the last `tail_budget` bytes, recording how
/// much was dropped in `omitted`. Callers must keep pushing new bytes once the buffer is
/// full, or the child blocks on pipe backpressure.
#[derive(Debug)]
pub struct OutputCapture {
    head: Vec<u8>,
    tail: Vec<u8>,
    omitted: u64,
    head_budget: usize,
    tail_budget: usize,
}

impl OutputCapture {
    pub fn new(max_bytes: usize) -> Self {
        let head_budget = max_bytes / 2;
        let tail_budget = max_bytes - head_budget;
        Self {
            head: Vec::with_capacity(head_budget),
            tail: Vec::with_capacity(tail_budget),
            omitted: 0,
            head_budget,
            tail_budget,
        }
    }

    pub fn push(&mut self, chunk: &[u8]) {
        if chunk.is_empty() {
            return;
        }
        if self.head_budget == 0 && self.tail_budget == 0 {
            self.omitted = self.omitted.saturating_add(chunk.len() as u64);
            return;
        }

        let mut rest = chunk;
        if self.head.len() < self.head_budget {
            let take = self
                .head_budget
                .saturating_sub(self.head.len())
                .min(rest.len());
            self.head.extend_from_slice(&rest[..take]);
            rest = &rest[take..];
        }
        if rest.is_empty() {
            return;
        }

        if rest.len() >= self.tail_budget {
            // Keep only the last tail_budget bytes, counting both the old tail and the dropped prefix.
            self.omitted = self.omitted.saturating_add(self.tail.len() as u64);
            self.tail.clear();
            let keep = self.tail_budget.min(rest.len());
            let start = rest.len() - keep;
            self.tail.extend_from_slice(&rest[start..]);
            self.omitted = self.omitted.saturating_add(start as u64);
            return;
        }

        let overflow = self
            .tail
            .len()
            .saturating_add(rest.len())
            .saturating_sub(self.tail_budget);
        if overflow > 0 {
            self.tail.drain(..overflow);
            self.omitted = self.omitted.saturating_add(overflow as u64);
        }
        self.tail.extend_from_slice(rest);
    }

    pub fn head(&self) -> &[u8] {
        &self.head
    }

    pub fn tail(&self) -> &[u8] {
        &self.tail
    }

    pub fn omitted_bytes(&self) -> u64 {
        self.omitted
    }

    pub fn is_empty(&self) -> bool {
        self.head.is_empty() && self.tail.is_empty()
    }

    /// Restricted text view for errors and logs: head, an omission marker, then tail.
    pub fn render(&self) -> String {
        let mut text = String::from_utf8_lossy(&self.head).into_owned();
        if self.omitted > 0 {
            text.push_str("\n[... omitted ");
            text.push_str(&self.omitted.to_string());
            text.push_str(" bytes ...]\n");
        }
        text.push_str(&String::from_utf8_lossy(&self.tail));
        text
    }
}

/// Full spec for one subprocess call. Environment variables go only into the child, never into Debug, logs or errors.
#[derive(Clone)]
pub struct CommandSpec {
    pub operation: &'static str,
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub envs: Vec<(String, String)>,
    pub timeout: Option<Duration>,
    pub drain_timeout: Duration,
    pub capture: CaptureMode,
    pub max_output_bytes: usize,
    pub exit_policy: ExitPolicy,
}

impl fmt::Debug for CommandSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let env_keys = self
            .envs
            .iter()
            .map(|(key, _)| format!("{key}=<redacted>"))
            .collect::<Vec<_>>();
        f.debug_struct("CommandSpec")
            .field("operation", &self.operation)
            .field("program", &self.program)
            .field("args", &self.args)
            .field("cwd", &self.cwd)
            .field("envs", &env_keys)
            .field("timeout", &self.timeout)
            .field("drain_timeout", &self.drain_timeout)
            .field("capture", &self.capture)
            .field("max_output_bytes", &self.max_output_bytes)
            .field("exit_policy", &self.exit_policy)
            .finish()
    }
}

impl CommandSpec {
    pub fn new(program: impl Into<String>) -> Self {
        Self {
            operation: "run subprocess",
            program: program.into(),
            args: Vec::new(),
            cwd: None,
            envs: Vec::new(),
            timeout: None,
            drain_timeout: DEFAULT_DRAIN_TIMEOUT,
            capture: CaptureMode::None,
            max_output_bytes: DEFAULT_OUTPUT_CAPACITY_BYTES,
            exit_policy: ExitPolicy::Success,
        }
    }

    pub fn operation(mut self, operation: &'static str) -> Self {
        self.operation = operation;
        self
    }

    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.envs.push((key.into(), value.into()));
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn capture(mut self, capture: CaptureMode) -> Self {
        self.capture = capture;
        self
    }

    pub fn exit_policy(mut self, policy: ExitPolicy) -> Self {
        self.exit_policy = policy;
        self
    }

    pub fn accepted_exit_codes(self, codes: &'static [i32]) -> Self {
        self.exit_policy(ExitPolicy::Accepted(codes))
    }

    pub fn any_exit_status(self) -> Self {
        self.exit_policy(ExitPolicy::Any)
    }
}

#[derive(Debug)]
pub struct CommandOutcome {
    pub status: ExitStatus,
    pub stdout: Option<OutputCapture>,
    pub stderr: Option<OutputCapture>,
}

impl CommandOutcome {
    pub fn stdout_text(&self) -> Option<String> {
        self.stdout.as_ref().map(OutputCapture::render)
    }

    pub fn stderr_text(&self) -> Option<String> {
        self.stderr.as_ref().map(OutputCapture::render)
    }
}

/// Structured failure payload for a non-zero exit. The output buffer is boxed to keep the error enum small.
#[derive(Debug)]
pub struct UnexpectedExit {
    pub status: ExitStatus,
    pub stdout: Option<OutputCapture>,
    pub stderr: Option<OutputCapture>,
}

impl fmt::Display for UnexpectedExit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unexpected {status}", status = self.status)?;
        if let Some(stderr) = &self.stderr
            && !stderr.is_empty()
        {
            write!(f, "\nstderr:\n{}", stderr.render())?;
        }
        if let Some(stdout) = &self.stdout
            && !stdout.is_empty()
        {
            write!(f, "\nstdout:\n{}", stdout.render())?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum ProcessErrorKind {
    Spawn {
        source: io::Error,
    },
    Wait {
        source: io::Error,
    },
    /// Reading output failed, or the drain thread could not start.
    Reader {
        stream: OutputStream,
        source: io::Error,
    },
    PipeMissing {
        stream: OutputStream,
    },
    Timeout {
        limit: Duration,
    },
    DrainTimeout {
        stream: OutputStream,
        limit: Duration,
    },
    UnexpectedExit(Box<UnexpectedExit>),
}

impl fmt::Display for ProcessErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn { source } => write!(f, "could not be spawned: {source}"),
            Self::Wait { source } => write!(f, "status wait failed: {source}"),
            Self::Reader { stream, source } => {
                write!(f, "{stream} reader failed: {source}")
            }
            Self::PipeMissing { stream } => {
                write!(f, "{stream} pipe was not created after spawn")
            }
            Self::Timeout { limit } => {
                write!(f, "timed out after {:.1}s", limit.as_secs_f64())
            }
            Self::DrainTimeout { stream, limit } => write!(
                f,
                "{stream} drain timed out after {:.1}s (grandchildren may hold the pipe)",
                limit.as_secs_f64()
            ),
            Self::UnexpectedExit(failure) => write!(f, "{failure}"),
        }
    }
}

#[derive(Debug)]
pub struct ProcessError {
    pub operation: &'static str,
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub kind: ProcessErrorKind,
}

impl fmt::Display for ProcessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} failed: {} {}",
            self.operation,
            self.program,
            self.args.join(" ")
        )?;
        if let Some(cwd) = &self.cwd {
            write!(f, " (cwd={})", cwd.display())?;
        }
        write!(f, ": {}", self.kind)
    }
}

impl std::error::Error for ProcessError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            ProcessErrorKind::Spawn { source }
            | ProcessErrorKind::Wait { source }
            | ProcessErrorKind::Reader { source, .. } => Some(source),
            ProcessErrorKind::PipeMissing { .. }
            | ProcessErrorKind::Timeout { .. }
            | ProcessErrorKind::DrainTimeout { .. }
            | ProcessErrorKind::UnexpectedExit(_) => None,
        }
    }
}

pub type ProcessResult<T> = std::result::Result<T, ProcessError>;

fn process_error(spec: &CommandSpec, kind: ProcessErrorKind) -> ProcessError {
    ProcessError {
        operation: spec.operation,
        program: spec.program.clone(),
        args: spec.args.clone(),
        cwd: spec.cwd.clone(),
        kind,
    }
}

/// Runs one subprocess call. The call site picks the total timeout via [`CommandSpec::timeout`];
/// the I/O drain timeout defaults to [`DEFAULT_DRAIN_TIMEOUT`] and can be overridden.
pub fn run_command(spec: &CommandSpec) -> ProcessResult<CommandOutcome> {
    let mut command = Command::new(&spec.program);
    command.args(&spec.args);
    if let Some(cwd) = &spec.cwd {
        command.current_dir(cwd);
    }
    for (key, value) in &spec.envs {
        command.env(key, value);
    }
    command.stdout(pipe_for(spec.capture.captures_stdout()));
    command.stderr(pipe_for(spec.capture.captures_stderr()));

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(source) => return Err(process_error(spec, ProcessErrorKind::Spawn { source })),
    };

    let stdout_rx = if spec.capture.captures_stdout() {
        let stream = match child.stdout.take() {
            Some(stream) => stream,
            None => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(process_error(
                    spec,
                    ProcessErrorKind::PipeMissing {
                        stream: OutputStream::Stdout,
                    },
                ));
            }
        };
        Some(
            match spawn_drain(OutputStream::Stdout, stream, spec.max_output_bytes) {
                Ok(rx) => rx,
                Err(source) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(process_error(
                        spec,
                        ProcessErrorKind::Reader {
                            stream: OutputStream::Stdout,
                            source,
                        },
                    ));
                }
            },
        )
    } else {
        None
    };

    let stderr_rx = if spec.capture.captures_stderr() {
        let stream = match child.stderr.take() {
            Some(stream) => stream,
            None => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(process_error(
                    spec,
                    ProcessErrorKind::PipeMissing {
                        stream: OutputStream::Stderr,
                    },
                ));
            }
        };
        Some(
            match spawn_drain(OutputStream::Stderr, stream, spec.max_output_bytes) {
                Ok(rx) => rx,
                Err(source) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(process_error(
                        spec,
                        ProcessErrorKind::Reader {
                            stream: OutputStream::Stderr,
                            source,
                        },
                    ));
                }
            },
        )
    } else {
        None
    };

    let status = match wait_child(&mut child, spec.timeout) {
        Ok(status) => ExitStatus::from_std(status),
        Err(kind) => {
            // The primary error is a timeout or wait failure, but the drain threads still get their
            // own deadline so a pipe inherited by a grandchild cannot hold them forever.
            if let Some(rx) = stdout_rx {
                let _ = collect_drain(rx, OutputStream::Stdout, spec.drain_timeout);
            }
            if let Some(rx) = stderr_rx {
                let _ = collect_drain(rx, OutputStream::Stderr, spec.drain_timeout);
            }
            return Err(process_error(spec, kind));
        }
    };

    // Even when one stream errors first, still wait for the other so no drain thread is left behind.
    let stdout_result = stdout_rx
        .map(|rx| collect_drain(rx, OutputStream::Stdout, spec.drain_timeout))
        .transpose();
    let stderr_result = stderr_rx
        .map(|rx| collect_drain(rx, OutputStream::Stderr, spec.drain_timeout))
        .transpose();

    let (stdout, stderr) = match (stdout_result, stderr_result) {
        (Ok(stdout), Ok(stderr)) => (stdout, stderr),
        (Err(kind), _) | (_, Err(kind)) => return Err(process_error(spec, kind)),
    };

    if spec.exit_policy.accepts(status) {
        Ok(CommandOutcome {
            status,
            stdout,
            stderr,
        })
    } else {
        Err(process_error(
            spec,
            ProcessErrorKind::UnexpectedExit(Box::new(UnexpectedExit {
                status,
                stdout,
                stderr,
            })),
        ))
    }
}

fn pipe_for(capture: bool) -> Stdio {
    if capture {
        Stdio::piped()
    } else {
        Stdio::null()
    }
}

fn spawn_drain(
    stream: OutputStream,
    mut reader: impl Read + Send + 'static,
    max_output_bytes: usize,
) -> io::Result<Receiver<io::Result<OutputCapture>>> {
    let (sender, receiver) = mpsc::channel();
    let mut capture = OutputCapture::new(max_output_bytes);
    thread::Builder::new()
        .name(format!("hybrid-mount-{stream}-drain"))
        .spawn(move || {
            let result = drain_reader(&mut reader, &mut capture);
            let _ = sender.send(result.map(|()| capture));
        })?;
    Ok(receiver)
}

fn drain_reader(reader: &mut impl Read, capture: &mut OutputCapture) -> io::Result<()> {
    let mut chunk = [0_u8; READ_CHUNK_BYTES];
    loop {
        match reader.read(&mut chunk) {
            Ok(0) => return Ok(()),
            Ok(read) => capture.push(&chunk[..read]),
            Err(err) if err.kind() == io::ErrorKind::Interrupted => {}
            Err(err) => return Err(err),
        }
    }
}

fn collect_drain(
    receiver: Receiver<io::Result<OutputCapture>>,
    stream: OutputStream,
    drain_timeout: Duration,
) -> Result<OutputCapture, ProcessErrorKind> {
    match receiver.recv_timeout(drain_timeout) {
        Ok(Ok(capture)) => Ok(capture),
        Ok(Err(source)) => Err(ProcessErrorKind::Reader { stream, source }),
        Err(RecvTimeoutError::Timeout) => Err(ProcessErrorKind::DrainTimeout {
            stream,
            limit: drain_timeout,
        }),
        Err(RecvTimeoutError::Disconnected) => Err(ProcessErrorKind::Reader {
            stream,
            source: io::Error::new(
                io::ErrorKind::BrokenPipe,
                "output drain thread disconnected",
            ),
        }),
    }
}

fn wait_child(
    child: &mut Child,
    timeout: Option<Duration>,
) -> Result<StdExitStatus, ProcessErrorKind> {
    let deadline = timeout.map(|limit| Instant::now() + limit);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => {
                if let Some(deadline) = deadline
                    && Instant::now() >= deadline
                {
                    // kill + wait only reaps the direct child; pipes inherited by grandchildren are handled
                    // by the drain timeout, which is exactly why the two timeouts must be independent.
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(ProcessErrorKind::Timeout {
                        limit: timeout.unwrap_or_default(),
                    });
                }
                thread::sleep(WAIT_POLL_INTERVAL);
            }
            Err(source) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(ProcessErrorKind::Wait { source });
            }
        }
    }
}

#[cfg(test)]
#[path = "process_tests.rs"]
mod tests;
