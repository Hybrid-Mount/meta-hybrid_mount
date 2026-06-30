// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::path::Path;
#[cfg(target_os = "android")]
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::conf::config::Config;

pub fn handle_gen_config(output: &Path, force: bool) -> Result<()> {
    if output.exists() && !force {
        bail!(
            "Config already exists at {}. Use --force to overwrite.",
            output.display()
        );
    }

    Config::default()
        .save_to_file(output)
        .with_context(|| format!("Failed to save generated config to {}", output.display()))
}

pub fn handle_logs(lines: usize) -> Result<()> {
    #[cfg(target_os = "android")]
    {
        let output = Command::new("logcat")
            .args(["-d", "-v", "brief", "-s", "Hybrid_Logger"])
            .output()
            .context("Failed to execute logcat for Hybrid Mount logs")?;

        if !output.status.success() {
            bail!(
                "logcat exited with status {} while fetching Hybrid Mount logs",
                output.status
            );
        }

        let selected: Vec<String> = String::from_utf8(output.stdout)
            .context("logcat returned non-UTF-8 output")?
            .lines()
            .rev()
            .take(lines)
            .map(str::to_owned)
            .collect();

        if selected.is_empty() {
            println!("No Hybrid Mount logcat entries were found.");
            return Ok(());
        }

        for line in selected.into_iter().rev() {
            println!("{line}");
        }

        Ok(())
    }

    #[cfg(not(target_os = "android"))]
    {
        let _ = lines;
        println!("Hybrid Mount logs are emitted to Android logcat with tag Hybrid_Logger.");
        println!(
            "Run `adb shell logcat -d -v brief -s Hybrid_Logger` on the device to inspect them."
        );
        Ok(())
    }
}
