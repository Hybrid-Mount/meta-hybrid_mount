// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    fs,
    io::Write,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::{conf::config::Config, defs, utils};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Snapshot {
    pub id: String,
    pub timestamp: u64,
    pub label: String,
    pub reason: String,
    pub config_snapshot: Config,
    #[serde(default)]
    pub raw_config: Option<String>,
    #[serde(default)]
    pub raw_state: Option<String>,
}

pub enum RecoveryStatus {
    Standby,
    Restored,
}

const RECOVERY_COUNTER_FILE: &str = "/data/adb/meta-hybrid/boot_counter";
const RECOVERY_RESCUE_NOTICE: &str = "/data/adb/meta-hybrid/rescue_notice";
const BACKUP_DIR: &str = "/data/adb/meta-hybrid/backups";

pub fn ensure_recovery_state() -> Result<RecoveryStatus> {
    let path = Path::new(RECOVERY_COUNTER_FILE);
    let mut count = 0;

    if path.exists() {
        let content = fs::read_to_string(path).unwrap_or_default();
        count = content.trim().parse::<u8>().unwrap_or(0);
    }

    count += 1;

    {
        let mut file = fs::File::create(path).context("Failed to open boot counter for writing")?;
        write!(file, "{}", count)?;
        file.sync_all()
            .context("Failed to sync boot counter to disk")?;
    }

    log::info!(">> Recovery Protocol: Boot counter at {}", count);

    if count >= 3 {
        log::error!(">> RECOVERY TRIGGERED: Detected potential bootloop (3 failed boots).");
        log::warn!(">> Executing emergency rollback from Backups...");

        match restore_latest_snapshot() {
            Ok(snapshot_id) => {
                log::info!(">> Rollback successful. Resetting counter.");
                let _ = fs::remove_file(path);
                let notice = format!(
                    "System recovered from bootloop by restoring snapshot: {}",
                    snapshot_id
                );

                if let Err(e) = fs::write(RECOVERY_RESCUE_NOTICE, notice) {
                    log::warn!("Failed to write rescue notice: {}", e);
                }

                return Ok(RecoveryStatus::Restored);
            }
            Err(e) => {
                log::error!(
                    ">> Rollback failed: {}. Disabling all modules as last resort.",
                    e
                );
                disable_all_modules()?;
                let _ = fs::remove_file(path);
            }
        }
    }

    Ok(RecoveryStatus::Standby)
}

pub fn reset_recovery_state() {
    let path = Path::new(RECOVERY_COUNTER_FILE);

    if path.exists() {
        if let Err(e) = fs::remove_file(path) {
            log::warn!("Failed to reset boot counter: {}", e);
        } else {
            log::debug!("Recovery Protocol: Counter reset. Boot successful.");
        }
    }
}

pub fn create_snapshot(config: &Config, label: &str, reason: &str) -> Result<String> {
    if let Err(e) = fs::create_dir_all(BACKUP_DIR) {
        log::warn!("Failed to create backup dir: {}", e);
    }

    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let id = format!("snap_{}", now);
    let raw_config = fs::read_to_string(crate::conf::config::CONFIG_FILE_DEFAULT).ok();
    let raw_state = fs::read_to_string(crate::defs::STATE_FILE).ok();

    let snapshot = Snapshot {
        id: id.clone(),
        timestamp: now,
        label: label.to_string(),
        reason: reason.to_string(),
        config_snapshot: config.clone(),
        raw_config,
        raw_state,
    };

    let file_path = Path::new(BACKUP_DIR).join(format!("{}.json", id));
    let json = serde_json::to_string_pretty(&snapshot)?;

    utils::atomic_write(&file_path, json)?;

    if let Err(e) = prune_snapshots(config) {
        log::warn!("Failed to prune backups: {}", e);
    }

    Ok(id)
}

pub fn list_snapshots() -> Result<Vec<Snapshot>> {
    let mut snapshots = Vec::new();

    if !Path::new(BACKUP_DIR).exists() {
        return Ok(snapshots);
    }

    for entry in fs::read_dir(BACKUP_DIR)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let content = fs::read_to_string(&path)?;
            if let Ok(snapshot) = serde_json::from_str::<Snapshot>(&content) {
                snapshots.push(snapshot);
            }
        }
    }

    snapshots.sort_by_key(|b| std::cmp::Reverse(b.timestamp));
    Ok(snapshots)
}

pub fn delete_snapshot(id: &str) -> Result<()> {
    let file_path = Path::new(BACKUP_DIR).join(format!("{}.json", id));

    if file_path.exists() {
        fs::remove_file(&file_path)?;
        log::info!("Deleted Snapshot: {}", id);
        Ok(())
    } else {
        bail!("Snapshot {} not found", id);
    }
}

pub fn restore_snapshot(id: &str) -> Result<()> {
    let file_path = Path::new(BACKUP_DIR).join(format!("{}.json", id));

    if !file_path.exists() {
        bail!("Snapshot {} not found", id);
    }

    let content = fs::read_to_string(&file_path)?;
    let snapshot: Snapshot = serde_json::from_str(&content)?;

    log::info!(
        ">> Restoring Snapshot: {} ({})",
        snapshot.id,
        snapshot.label
    );

    if let Some(raw) = &snapshot.raw_config {
        log::info!(">> Restoring config from RAW content...");
        utils::atomic_write(crate::conf::config::CONFIG_FILE_DEFAULT, raw)?;
    } else {
        log::info!(">> Raw config missing, restoring from struct...");
        let toml_str = toml::to_string(&snapshot.config_snapshot)?;
        utils::atomic_write(crate::conf::config::CONFIG_FILE_DEFAULT, toml_str)?;
    }

    if let Some(state) = &snapshot.raw_state {
        log::info!(">> Restoring state from snapshot...");
        utils::atomic_write(crate::defs::STATE_FILE, state)?;
    } else {
        log::warn!(">> No state snapshot found. Skipping state restore.");
    }

    Ok(())
}

fn restore_latest_snapshot() -> Result<String> {
    let snapshots = list_snapshots()?;
    if let Some(latest) = snapshots.first() {
        restore_snapshot(&latest.id)?;
        Ok(latest.id.clone())
    } else {
        bail!("No snapshots found");
    }
}

fn prune_snapshots(config: &Config) -> Result<()> {
    let snapshots = list_snapshots()?;
    let max_count = config.backup.max_backups;
    let retention_days = config.backup.retention_days;
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let mut deleted_count = 0;

    let expiration_ts = if retention_days > 0 {
        now.saturating_sub(retention_days * 86400)
    } else {
        0
    };

    for (i, snapshot) in snapshots.iter().enumerate() {
        let mut should_delete = false;
        if max_count > 0 && i >= max_count {
            should_delete = true;
        }
        if retention_days > 0 && snapshot.timestamp < expiration_ts && i > 0 {
            should_delete = true;
        }

        if should_delete {
            let path = Path::new(BACKUP_DIR).join(format!("{}.json", snapshot.id));
            if let Err(e) = fs::remove_file(&path) {
                log::warn!("Failed to delete old snapshot {}: {}", snapshot.id, e);
            } else {
                deleted_count += 1;
            }
        }
    }

    if deleted_count > 0 {
        log::info!("Backup Prune: Deleted {} old snapshots.", deleted_count);
    }

    Ok(())
}

fn disable_all_modules() -> Result<()> {
    let modules_dir = Path::new(defs::MODULES_DIR);
    if modules_dir.exists() {
        for entry in fs::read_dir(modules_dir)? {
            let entry = entry?;
            let disable_path = entry.path().join("disable");
            if !disable_path.exists() {
                fs::File::create(disable_path)?;
            }
        }
    }
    Ok(())
}
