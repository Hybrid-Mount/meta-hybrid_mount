// SPDX-License-Identifier: GPL-3.0-only

use std::{io, process::Command};

#[test]
fn missing_credentials_skip_without_requiring_the_prebuilt_helper() -> io::Result<()> {
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(["notify", "--label", "test"])
        .env_remove("TELEGRAM_BOT_TOKEN")
        .env_remove("TELEGRAM_CHAT_ID")
        .env("HYBRID_MOUNT_NOTIFY", "missing-notification-helper")
        .output()?;
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("skipping notification"));
    Ok(())
}

#[test]
fn configured_notifications_require_an_installed_helper() -> io::Result<()> {
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(["notify", "--label", "test"])
        .env("TELEGRAM_BOT_TOKEN", "unused-test-value")
        .env("TELEGRAM_CHAT_ID", "unused-test-value")
        .env("HYBRID_MOUNT_NOTIFY", "missing-notification-helper")
        .output()?;
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("install the prebuilt"));
    Ok(())
}
