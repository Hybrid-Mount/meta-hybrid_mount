// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::{
    fs,
    io::{BufRead, BufReader},
    path::Path,
    process::Command,
};

use crate::{core::storage::StorageMode, defs, sys::fs::atomic_write};

pub fn update_description(
    storage_mode: StorageMode,
    overlay_count: usize,
    magic_count: usize,
    blacklisted_count: usize,
) {
    let prop_path = Path::new(defs::MODULE_PROP_FILE);

    if !prop_path.exists() {
        return;
    }

    let desc_text =
        running_description(storage_mode, overlay_count, magic_count, blacklisted_count);

    set_description(prop_path, &desc_text);
}

fn running_description(
    storage_mode: StorageMode,
    overlay_count: usize,
    magic_count: usize,
    blacklisted_count: usize,
) -> String {
    let (mode_str, status_emoji) = match storage_mode {
        #[cfg(feature = "control-plane")]
        StorageMode::Tmpfs => ("Tmpfs", "🐾"),
        StorageMode::Ext4 => ("Ext4", "💿"),
    };

    let mut stats = Vec::new();
    stats.push(format!("Overlay:{}", overlay_count));
    stats.push(format!("Magic:{}", magic_count));
    if blacklisted_count > 0 {
        stats.push(format!("Blacklist:{}", blacklisted_count));
    }

    let stats_str = stats.join("  ");

    format!(
        "😋 运行中喵～ ({}) {}  {}",
        mode_str, status_emoji, stats_str
    )
}

pub fn update_crash_description(reason: &str) {
    let prop_path = Path::new(defs::MODULE_PROP_FILE);

    if !prop_path.exists() {
        return;
    }

    let desc_text = format!("😭 崩溃了呜～ 原因: {}", reason);
    set_description(prop_path, &desc_text);
}

fn set_description(prop_path: &Path, desc_text: &str) {
    let cmd = if crate::utils::KSU.load(std::sync::atomic::Ordering::Relaxed) {
        "ksud"
    } else {
        "apd"
    };

    let output = match Command::new(cmd)
        .args([
            "module",
            "config",
            "set",
            "override.description",
            desc_text,
            "--temp",
        ])
        .envs([
            ("KSU_MODULE", env!("MODULE_ID")),
            ("AP_MODULE", env!("MODULE_ID")),
        ])
        .output()
    {
        Ok(c) => c,
        Err(_) => {
            legacy_set_description(prop_path, desc_text);
            return;
        }
    };

    if output.status.success() {
        log::debug!("module config override.description successful set!");
    } else {
        log::warn!(
            "failed to set module config override.description: {}, fallback to write regular file",
            String::from_utf8_lossy(&output.stderr)
        );
        legacy_set_description(prop_path, desc_text);
    }
}

fn legacy_set_description(prop_path: &Path, desc_text: &str) {
    let lines: Vec<String> = match fs::File::open(prop_path) {
        Ok(file) => BufReader::new(file)
            .lines()
            .map_while(Result::ok)
            .map(|line| {
                if line.starts_with("description=") {
                    format!("description={}", desc_text)
                } else {
                    line
                }
            })
            .collect(),
        Err(err) => {
            crate::scoped_log!(
                warn,
                "module_status",
                "failed to read module.prop: path={}, error={}",
                prop_path.display(),
                err
            );
            return;
        }
    };

    let content = lines.join("\n");
    if let Err(err) = atomic_write(prop_path, format!("{}\n", content)) {
        crate::scoped_log!(
            warn,
            "module_status",
            "description update failed: path={}, error={}",
            prop_path.display(),
            err
        );
    }
}

#[cfg(test)]
mod tests {
    use super::running_description;
    use crate::core::storage::StorageMode;

    #[test]
    fn running_description_hides_kasumi_stats() {
        #[cfg(feature = "control-plane")]
        let desc = running_description(StorageMode::Tmpfs, 2, 3, 0);
        #[cfg(not(feature = "control-plane"))]
        let desc = running_description(StorageMode::Ext4, 2, 3, 0);

        assert!(!desc.contains("Kasumi:"));
        assert!(desc.contains("Overlay:2"));
        assert!(desc.contains("Magic:3"));
    }

    #[test]
    fn running_description_shows_blacklisted_count_when_nonzero() {
        #[cfg(feature = "control-plane")]
        let desc = running_description(StorageMode::Tmpfs, 2, 3, 1);
        #[cfg(not(feature = "control-plane"))]
        let desc = running_description(StorageMode::Ext4, 2, 3, 1);

        assert!(desc.contains("Blacklist:1"));
    }

    #[test]
    fn running_description_hides_blacklisted_count_when_zero() {
        #[cfg(feature = "control-plane")]
        let desc = running_description(StorageMode::Tmpfs, 2, 3, 0);
        #[cfg(not(feature = "control-plane"))]
        let desc = running_description(StorageMode::Ext4, 2, 3, 0);

        assert!(!desc.contains("Blacklist:"));
    }
}
