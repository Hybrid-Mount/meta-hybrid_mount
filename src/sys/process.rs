// SPDX-License-Identifier: GPL-3.0-only

//! 生产路径的统一子进程 runner。
//!
//! 设计约束：
//! - 不接受 shell 拼装字符串；程序与参数分别传入。
//! - stdout/stderr 只保留固定容量的 head + tail，超过上限后继续 drain，
//!   防止大输出 OOM 或子进程因管道写满而阻塞。
//! - 总超时与 I/O drain 超时独立：直接子进程被 kill 后，其孙进程可能继承
//!   stdout/stderr 管道并保持打开，drain 线程必须有自己的截止时间。
//! - 每个调用点必须显式声明可接受退出码；`ExitPolicy::Any` 只用于
//!   insmod 这类“退出码不承载结果、副作用才权威”的已知场景。
//! - 错误与日志不携带环境变量内容（KernelSU/APatch 模块名等仅写入子进程 env）。

use std::fmt;
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus as StdExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;

/// 每个流默认保留的总字节数(head 与 tail 各占一半)。
pub const DEFAULT_OUTPUT_CAPACITY_BYTES: usize = 32 * 1024;
/// 直接子进程退出后，等待输出 drain 线程的默认截止时间。
pub const DEFAULT_DRAIN_TIMEOUT: Duration = Duration::from_secs(2);
/// 子进程 wait 轮询间隔。
const WAIT_POLL_INTERVAL: Duration = Duration::from_millis(10);
/// 单次管道读取块大小。远小于容量上限，因此缓冲永远有界。
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

/// 跨平台子进程退出状态。`Signaled` 在非 Unix 目标上永远不会被构造，
/// 但保留变体可以让错误分类与日志代码不依赖平台 cfg。
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
    /// 不分配输出缓冲；stdout/stderr 直接连到 `/dev/null` 等价物。
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

/// 显式可接受退出码策略。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExitPolicy {
    /// 只接受退出码 0。
    Success,
    /// 只接受列出的退出码；如 `e2fsck` 的 0..=3。
    Accepted(&'static [i32]),
    /// 退出码不承载结果，副作用检查才权威（LKM `insmod` 故意返回 -EAGAIN）。
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

/// 有界 head + tail 输出缓冲。
///
/// 保留最前面的 `head_budget` 字节与最后面的 `tail_budget` 字节，
/// 中间丢弃量记录在 `omitted`。容量填满后调用方仍必须继续 push 新字节，
/// 否则子进程会因管道背压而阻塞。
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
            // 整段只保留最后 tail_budget 字节，旧 tail 与丢弃前缀一并计入。
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

    /// 用于错误与日志的受限文本视图：head + 省略标记 + tail。
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

/// 一次子进程调用的完整规格。环境变量只写进子进程，绝不进入 Debug/日志/错误。
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

    #[allow(dead_code)] // cwd 能力是 runner 契约的一部分，当前生产调用点都在默认工作目录
    pub fn cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
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

    #[allow(dead_code)] // 默认值已满足当前调用点；保留显式覆盖能力
    pub fn drain_timeout(mut self, drain_timeout: Duration) -> Self {
        self.drain_timeout = drain_timeout;
        self
    }

    pub fn capture(mut self, capture: CaptureMode) -> Self {
        self.capture = capture;
        self
    }

    #[allow(dead_code)] // 默认容量已满足当前调用点；保留显式覆盖能力
    pub fn output_capacity(mut self, max_bytes: usize) -> Self {
        self.max_output_bytes = max_bytes;
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

/// 非零退出码时的结构化失败载荷。输出缓冲 boxed，避免把错误枚举撑得过大。
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
    /// 输出读取失败或 drain 线程无法启动。
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

/// 执行一次子进程调用。总超时由调用点通过 [`CommandSpec::timeout`] 显式选择；
/// I/O drain 超时默认 [`DEFAULT_DRAIN_TIMEOUT`]，可显式覆盖。
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
            // 主错误是超时/等待失败，但仍给 drain 线程自己的截止时间，
            // 避免孙进程继承的管道把线程永远拖住。
            if let Some(rx) = stdout_rx {
                let _ = collect_drain(rx, OutputStream::Stdout, spec.drain_timeout);
            }
            if let Some(rx) = stderr_rx {
                let _ = collect_drain(rx, OutputStream::Stderr, spec.drain_timeout);
            }
            return Err(process_error(spec, kind));
        }
    };

    // 即使一个流先报错，也要尝试等待另一个流退出，避免遗留 drain 线程。
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
                    // kill + wait 只回收直接子进程。孙进程继承的管道由 drain
                    // 超时另行处理，这正是两个超时必须独立的原因。
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
