// SPDX-License-Identifier: GPL-3.0-only

use std::{io, process::Command};

fn command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_hybrid-mount-notify"));
    command
        .env_remove("TELEGRAM_BOT_TOKEN")
        .env_remove("TELEGRAM_CHAT_ID");
    command
}

#[test]
fn standalone_notification_skips_without_credentials() -> io::Result<()> {
    let output = command()
        .args([
            "--output",
            "missing-output",
            "--label",
            "预编译检查",
            "--topic-id",
            "37",
        ])
        .output()?;
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("skipping notification"));
    Ok(())
}

#[test]
fn malformed_topic_fails_before_any_notification() -> io::Result<()> {
    let output = command()
        .args(["--label", "test", "--topic-id", "invalid"])
        .output()?;
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("--topic-id"));
    Ok(())
}
