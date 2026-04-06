// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{path::Path, process::Command};

use anyhow::{Context, Result, bail};
#[cfg(any(target_os = "linux", target_os = "android"))]
use ksu::NukeExt4Sysfs;

#[cfg(any(target_os = "linux", target_os = "android"))]
fn execute_ksu_nuke(path: &Path) -> Result<()> {
    let mut nuke = NukeExt4Sysfs::new();
    nuke.add(path);
    nuke.execute()
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn execute_apatch_nuke(path: &Path) -> Result<()> {
    let kp_bin = std::env::var("HYBRID_MOUNT_APATCH_KP_BIN")
        .unwrap_or_else(|_| "/data/adb/ap/bin/kp".to_string());
    if !Path::new(&kp_bin).exists() {
        bail!("apatch kp tool not found: {kp_bin}");
    }

    let kpm_module = std::env::var("HYBRID_MOUNT_APATCH_KPM_MODULE")
        .unwrap_or_else(|_| "/data/adb/hybrid-mount/kpm/nuke_ext4_sysfs.kpm".to_string());
    let kpm_id =
        std::env::var("HYBRID_MOUNT_APATCH_KPM_ID").unwrap_or_else(|_| "nuke_ext4_sysfs".into());
    let call_mode =
        std::env::var("HYBRID_MOUNT_APATCH_KPM_CALL_MODE").unwrap_or_else(|_| "control".into());

    let load_status = Command::new(&kp_bin)
        .args(["kpm", "load", &kpm_module])
        .status()
        .with_context(|| format!("failed to load kpm module with {kp_bin}"))?;
    if !load_status.success() {
        bail!(
            "kpm load failed: module={kpm_module}, code={:?}",
            load_status.code()
        );
    }

    let path_str = path.to_string_lossy().to_string();
    let call_res = if call_mode.eq_ignore_ascii_case("nr") {
        let nr = std::env::var("HYBRID_MOUNT_APATCH_KPM_UNUSED_NR")
            .context("HYBRID_MOUNT_APATCH_KPM_UNUSED_NR is required when call mode is 'nr'")?;
        Command::new(&kp_bin)
            .args(["kpm", "call", &nr, &path_str])
            .status()
            .with_context(|| format!("failed to call kpm unused nr with {kp_bin}"))
    } else {
        let control_name = std::env::var("HYBRID_MOUNT_APATCH_KPM_CONTROL")
            .unwrap_or_else(|_| "nuke_ext4_sysfs".to_string());
        Command::new(&kp_bin)
            .args(["kpm", "control", &control_name, &path_str])
            .status()
            .with_context(|| format!("failed to call kpm control with {kp_bin}"))
    };

    let unload_status = Command::new(&kp_bin)
        .args(["kpm", "unload", &kpm_id])
        .status()
        .with_context(|| format!("failed to unload kpm module with {kp_bin}"))?;
    if !unload_status.success() {
        crate::scoped_log!(
            warn,
            "nuke",
            "kpm unload failed: module={}, code={:?}",
            kpm_id,
            unload_status.code()
        );
    }

    let call_status = call_res?;
    if !call_status.success() {
        bail!(
            "kpm invoke failed: mode={call_mode}, code={:?}",
            call_status.code()
        );
    }

    Ok(())
}

pub fn nuke_path(path: &Path) -> Result<()> {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        let result = if ksu::version().is_some() {
            execute_ksu_nuke(path)
        } else {
            execute_apatch_nuke(path)
        };

        match result {
            Ok(()) => {
                crate::scoped_log!(debug, "nuke", "execute success: path={}", path.display());
                Ok(())
            }
            Err(e) => {
                crate::scoped_log!(
                    warn,
                    "nuke",
                    "execute failed: path={}, error={:#}",
                    path.display(),
                    e
                );
                Err(e)
            }
        }
    }

    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        let _ = path;
        Ok(())
    }
}
