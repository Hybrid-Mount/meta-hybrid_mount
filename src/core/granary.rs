// Copyright 2025 Meta-Hybrid Mount Authors
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
pub struct Silo {
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

const RATOON_COUNTER_FILE: &str = "/data/adb/meta-hybrid/ratoon_counter";

const RATOON_RESCUE_NOTICE: &str = "/data/adb/meta-hybrid/rescue_notice";

const GRANARY_DIR: &str = "/data/adb/meta-hybrid/granary";

pub fn engage_ratoon_protocol() -> Result<()> {
    let path = Path::new(RATOON_COUNTER_FILE);

    let mut count = 0;

    if path.exists() {
        let content = fs::read_to_string(path).unwrap_or_default();

        count = content.trim().parse::<u8>().unwrap_or(0);
    }

    count += 1;

    // Use explicit file operations to ensure persistence against kernel panic
    {
        let mut file =
            fs::File::create(path).context("Failed to open Ratoon counter for writing")?;

        write!(file, "{}", count)?;

        file.sync_all()
            .context("Failed to sync Ratoon counter to disk")?;
    }

    log::info!(">> Ratoon Protocol: Boot counter at {}", count);

    if count >= 3 {
        log::error!(">> RATOON TRIGGERED: Detected potential bootloop (3 failed boots).");

        log::warn!(">> Executing emergency rollback from Granary...");

        match restore_latest_silo() {
            Ok(silo_id) => {
                log::info!(">> Rollback successful. Resetting counter.");

                let _ = fs::remove_file(path);

                // Write notice for WebUI/User
                let notice = format!(
                    "System recovered from bootloop by restoring snapshot: {}",
                    silo_id
                );

                if let Err(e) = fs::write(RATOON_RESCUE_NOTICE, notice) {
                    log::warn!("Failed to write rescue notice: {}", e);
                }
            }
            Err(e) => {
                log::error!(
                    ">> Rollback failed: {}. Disabling all modules as last resort.",
                    e
                );

                disable_all_modules()?;

                // Also reset counter to avoid infinite loop of failing restores
                let _ = fs::remove_file(path);
            }
        }
    }

    Ok(())
}

pub fn disengage_ratoon_protocol() {
    let path = Path::new(RATOON_COUNTER_FILE);

    if path.exists() {
        if let Err(e) = fs::remove_file(path) {
            log::warn!("Failed to reset Ratoon counter: {}", e);
        } else {
            log::debug!("Ratoon Protocol: Counter reset. Boot successful.");
        }
    }
}

pub fn create_silo(config: &Config, label: &str, reason: &str) -> Result<String> {
    if let Err(e) = fs::create_dir_all(GRANARY_DIR) {
        log::warn!("Failed to create granary dir: {}", e);
    }

    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

    let id = format!("silo_{}", now);

    let raw_config = fs::read_to_string(crate::conf::config::CONFIG_FILE_DEFAULT).ok();

    let raw_state = fs::read_to_string(crate::defs::STATE_FILE).ok();

    let silo = Silo {
        id: id.clone(),
        timestamp: now,
        label: label.to_string(),
        reason: reason.to_string(),
        config_snapshot: config.clone(),
        raw_config,
        raw_state,
    };

    let file_path = Path::new(GRANARY_DIR).join(format!("{}.json", id));

    let json = serde_json::to_string_pretty(&silo)?;

    utils::atomic_write(&file_path, json)?;

    if let Err(e) = prune_silos(config) {
        log::warn!("Failed to prune granary: {}", e);
    }

    Ok(id)
}

pub fn list_silos() -> Result<Vec<Silo>> {
    let mut silos = Vec::new();

    if !Path::new(GRANARY_DIR).exists() {
        return Ok(silos);
    }

    for entry in fs::read_dir(GRANARY_DIR)? {
        let entry = entry?;

        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let content = fs::read_to_string(&path)?;

            if let Ok(silo) = serde_json::from_str::<Silo>(&content) {
                silos.push(silo);
            }
        }
    }

    silos.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    Ok(silos)
}

pub fn delete_silo(id: &str) -> Result<()> {
    let file_path = Path::new(GRANARY_DIR).join(format!("{}.json", id));

    if file_path.exists() {
        fs::remove_file(&file_path)?;

        log::info!("Deleted Silo: {}", id);

        Ok(())
    } else {
        bail!("Silo {} not found", id);
    }
}

pub fn restore_silo(id: &str) -> Result<()> {
    let file_path = Path::new(GRANARY_DIR).join(format!("{}.json", id));

    if !file_path.exists() {
        bail!("Silo {} not found", id);
    }

    let content = fs::read_to_string(&file_path)?;

    let silo: Silo = serde_json::from_str(&content)?;

    log::info!(">> Restoring Silo: {} ({})", silo.id, silo.label);

    if let Some(raw) = &silo.raw_config {
        log::info!(">> Restoring config from RAW content (preserving comments)...");

        utils::atomic_write(crate::conf::config::CONFIG_FILE_DEFAULT, raw)?;
    } else {
        log::info!(">> Raw config missing, restoring from struct snapshot...");

        let toml_str = toml::to_string(&silo.config_snapshot)?;

        utils::atomic_write(crate::conf::config::CONFIG_FILE_DEFAULT, toml_str)?;
    }

    if let Some(state) = &silo.raw_state {
        log::info!(">> Restoring state from snapshot...");

        utils::atomic_write(crate::defs::STATE_FILE, state)?;
    } else {
        log::warn!(">> No state snapshot found in this Silo. Skipping state restore.");
    }

    Ok(())
}

fn restore_latest_silo() -> Result<String> {
    let silos = list_silos()?;

    if let Some(latest) = silos.first() {
        restore_silo(&latest.id)?;

        Ok(latest.id.clone())
    } else {
        bail!("No silos found in Granary");
    }
}

fn prune_silos(config: &Config) -> Result<()> {
    let silos = list_silos()?;

    let max_count = config.granary.max_backups;

    let retention_days = config.granary.retention_days;

    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

    let mut deleted_count = 0;

    let expiration_ts = if retention_days > 0 {
        now.saturating_sub(retention_days * 86400)
    } else {
        0
    };

    for (i, silo) in silos.iter().enumerate() {
        let mut should_delete = false;

        if max_count > 0 && i >= max_count {
            should_delete = true;
        }

        if retention_days > 0 && silo.timestamp < expiration_ts && i > 0 {
            should_delete = true;
        }

        if should_delete {
            let path = Path::new(GRANARY_DIR).join(format!("{}.json", silo.id));

            if let Err(e) = fs::remove_file(&path) {
                log::warn!("Failed to delete old silo {}: {}", silo.id, e);
            } else {
                deleted_count += 1;
            }
        }
    }

    if deleted_count > 0 {
        log::info!("Granary Prune: Deleted {} old snapshots.", deleted_count);
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
