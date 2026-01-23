// Copyright 2026 Hybrid Mount Authors
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{fs::File, path::Path};

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::{
    conf::{
        cli::{Cli, PoaceaeAction},
        config::{CONFIG_FILE_DEFAULT, Config},
    },
    core::{granary, inventory, modules, planner, poaceae, storage}, // Added poaceae
};

#[derive(Serialize)]
struct DiagnosticIssueJson {
    level: String,
    context: String,
    message: String,
}

fn load_config(cli: &Cli) -> Result<Config> {
    if let Some(config_path) = &cli.config {
        return Config::from_file(config_path).with_context(|| {
            format!(
                "Failed to load config from custom path: {}",
                config_path.display()
            )
        });
    }

    match Config::load_default() {
        Ok(config) => Ok(config),
        Err(e) => {
            let is_not_found = e
                .root_cause()
                .downcast_ref::<std::io::Error>()
                .map(|io_err| io_err.kind() == std::io::ErrorKind::NotFound)
                .unwrap_or(false);

            if is_not_found {
                Ok(Config::default())
            } else {
                Err(e).context(format!(
                    "Failed to load default config from {}",
                    CONFIG_FILE_DEFAULT
                ))
            }
        }
    }
}

pub fn handle_gen_config(output: &Path) -> Result<()> {
    Config::default()
        .save_to_file(output)
        .with_context(|| format!("Failed to save generated config to {}", output.display()))
}

pub fn handle_show_config(cli: &Cli) -> Result<()> {
    let config = load_config(cli)?;

    let json = serde_json::to_string(&config).context("Failed to serialize config to JSON")?;

    println!("{}", json);

    Ok(())
}

pub fn handle_save_config(cli: &Cli, payload: &str) -> Result<()> {
    if let Ok(old_config) = load_config(cli)
        && let Err(e) = granary::create_snapshot(&old_config, "Auto-Backup", "Pre-WebUI Save")
    {
        log::warn!("Failed to create Backup: {}", e);
    }

    let json_bytes = (0..payload.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&payload[i..i + 2], 16))
        .collect::<Result<Vec<u8>, _>>()
        .context("Failed to decode hex payload")?;

    let config: Config =
        serde_json::from_slice(&json_bytes).context("Failed to parse config JSON payload")?;

    config
        .save_to_file(CONFIG_FILE_DEFAULT)
        .context("Failed to save config file")?;

    println!("Configuration saved successfully.");

    Ok(())
}

pub fn handle_save_module_rules(module_id: &str, payload: &str) -> Result<()> {
    let json_bytes = (0..payload.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&payload[i..i + 2], 16))
        .collect::<Result<Vec<u8>, _>>()
        .context("Failed to decode hex payload")?;

    let _rules: inventory::ModuleRules =
        serde_json::from_slice(&json_bytes).context("Failed to parse module rules JSON")?;

    let rules_dir = Path::new("/data/adb/meta-hybrid/rules");

    if !rules_dir.exists() {
        std::fs::create_dir_all(rules_dir).with_context(|| {
            format!(
                "Failed to create rules directory at {}",
                rules_dir.display()
            )
        })?;
    }

    let rule_file = rules_dir.join(format!("{}.json", module_id));

    std::fs::write(&rule_file, json_bytes)
        .with_context(|| format!("Failed to write rule file to {}", rule_file.display()))?;

    println!("Module rules saved for {}", module_id);

    Ok(())
}

pub fn handle_storage() -> Result<()> {
    storage::print_status().context("Failed to retrieve storage status")
}

pub fn handle_modules(cli: &Cli) -> Result<()> {
    let config = load_config(cli)?;

    modules::print_list(&config).context("Failed to list modules")
}

pub fn handle_conflicts(cli: &Cli) -> Result<()> {
    let config = load_config(cli)?;

    let module_list = inventory::scan(&config.moduledir, &config)
        .context("Failed to scan modules for conflict analysis")?;

    let plan = planner::generate(&config, &module_list, &config.moduledir)
        .context("Failed to generate plan for conflict analysis")?;

    let report = plan.analyze();

    let json =
        serde_json::to_string(&report.conflicts).context("Failed to serialize conflict report")?;

    println!("{}", json);

    Ok(())
}

pub fn handle_diagnostics(cli: &Cli) -> Result<()> {
    let config = load_config(cli)?;

    let module_list = inventory::scan(&config.moduledir, &config)
        .context("Failed to scan modules for diagnostics")?;

    let plan = planner::generate(&config, &module_list, &config.moduledir)
        .context("Failed to generate plan for diagnostics")?;

    let report = plan.analyze();

    let json_issues: Vec<DiagnosticIssueJson> = report
        .diagnostics
        .into_iter()
        .map(|i| DiagnosticIssueJson {
            level: match i.level {
                planner::DiagnosticLevel::Info => "Info".to_string(),
                planner::DiagnosticLevel::Warning => "Warning".to_string(),
                planner::DiagnosticLevel::Critical => "Critical".to_string(),
            },
            context: i.context,
            message: i.message,
        })
        .collect();

    let json =
        serde_json::to_string(&json_issues).context("Failed to serialize diagnostics report")?;

    println!("{}", json);

    Ok(())
}

pub fn handle_system_action(cli: &Cli, action: &str, value: Option<&str>) -> Result<()> {
    let config = load_config(cli)?;

    match action {
        "backup-list" => {
            let snapshots = granary::list_snapshots()?;

            let json = serde_json::to_string(&snapshots)?;

            println!("{}", json);
        }
        "backup-create" => {
            let reason = value.unwrap_or("Manual Backup");

            granary::create_snapshot(&config, "Manual Snapshot", reason)?;

            println!("Snapshot created.");
        }
        "backup-delete" => {
            if let Some(id) = value {
                granary::delete_snapshot(id)?;

                println!("Snapshot {} deleted.", id);
            } else {
                bail!("Missing Snapshot ID");
            }
        }
        "backup-restore" => {
            if let Some(id) = value {
                granary::restore_snapshot(id)?;

                println!("Snapshot {} restored. Please reboot.", id);
            } else {
                bail!("Missing Snapshot ID");
            }
        }
        "granary-list" => handle_system_action(cli, "backup-list", value)?,
        "granary-create" => handle_system_action(cli, "backup-create", value)?,
        "granary-delete" => handle_system_action(cli, "backup-delete", value)?,
        "granary-restore" => handle_system_action(cli, "backup-restore", value)?,
        _ => bail!("Unknown action: {}", action),
    }

    Ok(())
}

pub fn handle_poaceae(target_path: &str, action: &PoaceaeAction) -> Result<()> {
    let file = File::open(target_path)
        .with_context(|| format!("Failed to open PoaceaeFS root at {}", target_path))?;

    match action {
        PoaceaeAction::Hide { name } => {
            poaceae::hide(&file, name)?;
            println!("Hidden: {}", name);
        }
        PoaceaeAction::Unhide { name } => {
            poaceae::unhide(&file, name)?;
            println!("Unhidden: {}", name);
        }
        PoaceaeAction::Redirect { src, dst } => {
            poaceae::redirect(&file, src, dst)?;
            println!("Redirected: {} -> {}", src, dst);
        }
        PoaceaeAction::Unredirect { src } => {
            poaceae::unredirect(&file, src)?;
            println!("Removed redirect: {}", src);
        }
        PoaceaeAction::Spoof {
            name,
            uid,
            gid,
            mode,
            mtime,
        } => {
            poaceae::spoof(&file, name, *uid, *gid, *mode, *mtime)?;
            println!(
                "Spoofed: {} (uid={}, gid={}, mode={:o})",
                name, uid, gid, mode
            );
        }
        PoaceaeAction::Unspoof { name } => {
            poaceae::unspoof(&file, name)?;
            println!("Removed spoof: {}", name);
        }
        PoaceaeAction::Merge { src, target } => {
            poaceae::merge(&file, src, target)?;
            println!("Merged: {} -> {}", src, target);
        }
        PoaceaeAction::Unmerge { src } => {
            poaceae::unmerge(&file, src)?;
            println!("Removed merge: {}", src);
        }
        PoaceaeAction::Trust { gid } => {
            poaceae::set_trust(&file, *gid)?;
            println!("Trusted GID set to: {}", gid);
        }
    }
    Ok(())
}
