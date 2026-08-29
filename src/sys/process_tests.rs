// SPDX-License-Identifier: GPL-3.0-only

//! Host 可运行的统一子进程 runner 测试。
//!
//! 平台命令仅用于测试 runner 本身；生产代码永远不接受用户拼装的 shell 字符串。

use std::sync::mpsc;
use std::time::Duration;

use super::*;

#[cfg(windows)]
fn echo_spec(text: &str) -> CommandSpec {
    CommandSpec::new("cmd").args(["/C", "echo", text])
}

#[cfg(not(windows))]
fn echo_spec(text: &str) -> CommandSpec {
    CommandSpec::new("sh").args(["-c", &format!("printf {text}")])
}

#[cfg(windows)]
fn exit_code_spec(code: i32) -> CommandSpec {
    CommandSpec::new("cmd").args(["/C", &format!("exit {code}")])
}

#[cfg(not(windows))]
fn exit_code_spec(code: i32) -> CommandSpec {
    CommandSpec::new("sh").args(["-c", &format!("exit {code}")])
}

#[cfg(windows)]
fn sleep_spec(seconds: u64) -> CommandSpec {
    CommandSpec::new("cmd").args(["/C", &format!("ping -n {seconds} 127.0.0.1 >nul")])
}

#[cfg(not(windows))]
fn sleep_spec(seconds: u64) -> CommandSpec {
    CommandSpec::new("sh").args(["-c", &format!("sleep {seconds}")])
}

#[test]
fn head_tail_buffer_keeps_head_tail_and_counts_omitted_bytes() {
    let mut capture = OutputCapture::new(8);

    capture.push(b"abc");
    capture.push(b"defgh");
    assert_eq!(capture.head(), b"abcd");
    assert_eq!(capture.tail(), b"efgh");
    assert_eq!(capture.omitted_bytes(), 0);

    capture.push(b"X");
    assert_eq!(capture.head(), b"abcd");
    assert_eq!(capture.tail(), b"fghX");
    assert_eq!(capture.omitted_bytes(), 1);

    capture.push(&[0x61; 1024]);
    assert_eq!(capture.head(), b"abcd");
    assert_eq!(capture.tail(), b"aaaa");
    assert!(capture.omitted_bytes() > 1024);
    assert!(capture.render().contains("omitted"));
}

#[test]
fn head_tail_buffer_renders_non_utf8_without_panicking() {
    let mut capture = OutputCapture::new(16);
    capture.push(&[0xff, 0xfe, b'a', b'b']);

    let rendered = capture.render();
    assert!(!rendered.is_empty());
}

#[test]
fn command_spec_debug_redacts_environment_values() {
    let spec = CommandSpec::new("ksud")
        .env("KSU_MODULE", "hybrid_mount")
        .env("TELEGRAM_TOKEN", "do-not-leak");
    let debug = format!("{spec:?}");

    assert!(debug.contains("TELEGRAM_TOKEN=<redacted>"));
    assert!(!debug.contains("do-not-leak"));
    assert!(!debug.contains("hybrid_mount"));
}

#[test]
fn runner_captures_stdout_and_treats_zero_exit_as_success() {
    let spec = echo_spec("hello").capture(CaptureMode::Stdout);
    let outcome = run_command(&spec).unwrap();

    assert_eq!(outcome.status, ExitStatus::Exited(0));
    assert!(outcome.stdout_text().unwrap().contains("hello"));
    assert!(outcome.stderr.is_none());
}

#[test]
fn runner_rejects_unexpected_exit_when_policy_is_success() {
    let spec = exit_code_spec(3).capture(CaptureMode::Stderr);
    let err = run_command(&spec).unwrap_err();

    match err.kind {
        ProcessErrorKind::UnexpectedExit(ref failure) => {
            assert_eq!(failure.status, ExitStatus::Exited(3));
        }
        ref other => panic!("expected UnexpectedExit, got {other:?}"),
    }
    assert!(!err.to_string().is_empty());
}

#[test]
fn runner_accepts_explicitly_declared_exit_codes() {
    let spec = exit_code_spec(3).accepted_exit_codes(&[3]);
    let outcome = run_command(&spec).unwrap();

    assert_eq!(outcome.status, ExitStatus::Exited(3));
}

#[test]
fn runner_accepts_any_exit_status_only_when_declared() {
    let spec = exit_code_spec(7).any_exit_status();
    let outcome = run_command(&spec).unwrap();

    assert_eq!(outcome.status, ExitStatus::Exited(7));
}

#[test]
fn no_capture_mode_allocates_no_output_buffers() {
    let spec = echo_spec("ignored").capture(CaptureMode::None);
    let outcome = run_command(&spec).unwrap();

    assert!(outcome.stdout.is_none());
    assert!(outcome.stderr.is_none());
}

#[test]
fn runner_kills_child_after_total_timeout() {
    let spec = sleep_spec(30)
        .capture(CaptureMode::None)
        .timeout(Duration::from_millis(100));
    let err = run_command(&spec).unwrap_err();

    match err.kind {
        ProcessErrorKind::Timeout { limit } => assert_eq!(limit, Duration::from_millis(100)),
        ref other => panic!("expected Timeout, got {other:?}"),
    }
}

#[test]
fn drain_timeout_is_reported_when_reader_never_finishes() {
    let (_sender, receiver) = mpsc::channel::<std::io::Result<OutputCapture>>();
    let err = collect_drain(receiver, OutputStream::Stderr, Duration::from_millis(20)).unwrap_err();

    match err {
        ProcessErrorKind::DrainTimeout {
            stream: OutputStream::Stderr,
            limit,
        } => assert_eq!(limit, Duration::from_millis(20)),
        ref other => panic!("expected DrainTimeout, got {other:?}"),
    }
}
