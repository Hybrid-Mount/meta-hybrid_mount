// SPDX-License-Identifier: GPL-3.0-only

//! 结构化错误分类与上下文展示测试。

use std::error::Error as StdError;
use std::io::{self, ErrorKind};
use std::path::PathBuf;
use std::time::Duration;

use crate::sys::process::{
    CaptureMode, CommandSpec, ExitPolicy, ExitStatus, ProcessError, ProcessErrorKind, run_command,
};

use super::*;

#[test]
fn io_context_error_displays_operation_path_and_source() {
    let err = Error::Mount(Box::new(ContextError::new(
        "mount overlay target",
        Some(PathBuf::from("/system")),
        CausalError::from(io::Error::new(ErrorKind::PermissionDenied, "denied")),
    )));

    let text = err.to_string();
    assert!(text.contains("mount overlay target"));
    assert!(text.contains("/system"));
    assert!(text.contains("denied"));
}

#[test]
fn io_error_keeps_context_path_and_source_as_structured_fields() {
    let source = io::Error::new(ErrorKind::PermissionDenied, "denied");
    let err = Error::IoContext(Box::new(IoError::new(
        "read module directory",
        Some(PathBuf::from("/data/adb/modules/alpha")),
        io::Error::new(ErrorKind::PermissionDenied, "denied"),
    )));

    let text = err.to_string();
    assert!(text.contains("read module directory"));
    assert!(text.contains("/data/adb/modules/alpha"));
    assert!(text.contains("denied"));
    assert_eq!(err.classify(), ErrorClass::ManualRecovery);
    assert!(err.requires_manual_intervention());
    match &err {
        Error::IoContext(inner) => assert_eq!(
            inner
                .source()
                .and_then(|source| source.downcast_ref::<io::Error>())
                .map(io::Error::kind),
            Some(source.kind())
        ),
        other => panic!("expected IoContext, got {other:?}"),
    }
}

#[test]
fn interrupted_io_error_is_classified_transient() {
    let err = Error::Io(io::Error::from(ErrorKind::Interrupted));

    assert_eq!(err.classify(), ErrorClass::Transient);
    assert!(err.is_retryable());
    assert!(!err.requires_manual_intervention());
}

#[test]
fn permission_denied_io_error_requires_manual_intervention() {
    let err = Error::Io(io::Error::from(ErrorKind::PermissionDenied));

    assert_eq!(err.classify(), ErrorClass::ManualRecovery);
    assert!(err.requires_manual_intervention());
    assert!(!err.is_retryable());
}

#[test]
fn plan_conflict_requires_manual_intervention() {
    let err = Error::PlanConflict {
        target: "/system/etc/hosts".to_owned(),
        first_backend: "overlay".to_owned(),
        first_source: "alpha".to_owned(),
        second_backend: "magic".to_owned(),
        second_source: "beta".to_owned(),
    };

    assert_eq!(err.classify(), ErrorClass::ManualRecovery);
    assert!(err.requires_manual_intervention());
    assert!(!err.is_retryable());
}

#[test]
fn subprocess_timeout_is_classified_transient() {
    let err = Error::from(ProcessError {
        operation: "e2fsck repair",
        program: "e2fsck".to_owned(),
        args: vec!["-y".to_owned()],
        cwd: None,
        kind: ProcessErrorKind::Timeout {
            limit: Duration::from_secs(30),
        },
    });

    assert_eq!(err.classify(), ErrorClass::Transient);
    assert!(err.is_retryable());
}

#[test]
fn subprocess_unexpected_exit_is_classified_permanent() {
    let err = Error::from(ProcessError {
        operation: "format ext4 image",
        program: "mke2fs".to_owned(),
        args: vec!["-t".to_owned(), "ext4".to_owned()],
        cwd: None,
        kind: ProcessErrorKind::UnexpectedExit(Box::new(crate::sys::process::UnexpectedExit {
            status: ExitStatus::Exited(8),
            stdout: None,
            stderr: None,
        })),
    });

    assert_eq!(err.classify(), ErrorClass::Permanent);
    assert!(!err.is_retryable());
}

#[test]
fn subprocess_runner_reports_declared_exit_policy_violation_as_error() {
    let spec = CommandSpec::new("definitely-not-a-real-command-xyz")
        .capture(CaptureMode::None)
        .exit_policy(ExitPolicy::Success);
    let err = run_command(&spec).unwrap_err();

    assert!(matches!(err.kind, ProcessErrorKind::Spawn { .. }));
}
