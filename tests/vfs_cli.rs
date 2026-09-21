// SPDX-License-Identifier: GPL-3.0-only

//! Process-level checks never issue a valid kernel mutation or load request.

use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_hybrid-mount"))
        .args(args)
        .env("RUST_LOG", "off")
        .output()
        .expect("run hybrid-mount")
}

#[test]
fn vfs_without_arguments_shows_help_instead_of_running_mount_pipeline() {
    let output = run(&["vfs"]);
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("Usage: hybrid-mount vfs"));
    assert!(output.stderr.is_empty());
}

#[test]
fn vfs_argument_errors_exit_two_and_write_only_to_stderr() {
    for args in [
        vec!["vfs", "clear"],
        vec!["vfs", "clear", "all"],
        vec!["vfs", "clear", "typo", "--yes"],
        vec!["vfs", "rule", "add", "/v", "/r", "/odd", "--json"],
        vec!["vfs", "uid", "add", "10000", "invalid"],
        vec!["vfs", "uid", "clear"],
        vec!["vfs", "load", "unexpected"],
    ] {
        let output = run(&args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("vfs help"),
            "{args:?}"
        );
    }
}

#[test]
fn old_top_level_version_and_unknown_command_keep_their_contract() {
    let output = run(&["version"]);
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(run(&["not-a-command"]).status.code(), Some(1));
}

#[test]
#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn host_reports_platform_errors_and_preserves_legacy_doctor_json() {
    for args in [
        vec!["vfs", "version"],
        vec!["vfs", "rule", "list", "--json"],
        vec!["vfs", "load"],
    ] {
        let output = run(&args);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("linux/android"));
    }
    let old = run(&["vfs-doctor"]);
    let new = run(&["vfs", "doctor", "--json"]);
    assert!(old.status.success() && new.status.success());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&old.stdout).unwrap(),
        serde_json::from_slice::<serde_json::Value>(&new.stdout).unwrap()
    );
}
