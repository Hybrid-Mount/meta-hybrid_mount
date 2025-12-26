// Copyright 2025 Meta-Hybrid Mount Authors
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::{
    conf::{
        cli::Cli,
        config::{CONFIG_FILE_DEFAULT, Config},
    },
    core::{executor, granary, inventory, modules, planner, storage, winnow},
    utils,
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
        && let Err(e) = granary::create_silo(&old_config, "Auto-Backup", "Pre-WebUI Save")
    {
        log::warn!("Failed to create Granary backup: {}", e);
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

pub fn handle_save_rules(module: &str, payload: &str) -> Result<()> {
    utils::validate_module_id(module).with_context(|| format!("Invalid module ID: {}", module))?;

    let json_bytes = (0..payload.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&payload[i..i + 2], 16))
        .collect::<Result<Vec<u8>, _>>()
        .context("Failed to decode hex payload")?;

    let _: inventory::ModuleRules =
        serde_json::from_slice(&json_bytes).context("Invalid rules JSON")?;

    let rules_dir = std::path::Path::new("/data/adb/meta-hybrid/rules");

    std::fs::create_dir_all(rules_dir).context("Failed to create rules directory")?;

    let file_path = rules_dir.join(format!("{}.json", module));

    std::fs::write(&file_path, json_bytes)
        .with_context(|| format!("Failed to write rules file: {}", file_path.display()))?;

    println!("Rules for module '{}' saved.", module);

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

    let report = plan.analyze_conflicts();

    let winnowed = winnow::sift_conflicts(report.details, &config.winnowing);

    let json = serde_json::to_string(&winnowed).context("Failed to serialize conflict report")?;

    println!("{}", json);

    Ok(())
}

pub fn handle_diagnostics(cli: &Cli) -> Result<()> {
    let config = load_config(cli)?;

    let module_list = inventory::scan(&config.moduledir, &config)
        .context("Failed to scan modules for diagnostics")?;

    let plan = planner::generate(&config, &module_list, &config.moduledir)
        .context("Failed to generate plan for diagnostics")?;

    let issues = executor::diagnose_plan(&plan);

    let json_issues: Vec<DiagnosticIssueJson> = issues
        .into_iter()
        .map(|i| DiagnosticIssueJson {
            level: match i.level {
                executor::DiagnosticLevel::Info => "Info".to_string(),
                executor::DiagnosticLevel::Warning => "Warning".to_string(),
                executor::DiagnosticLevel::Critical => "Critical".to_string(),
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
    let mut config = load_config(cli)?;

    match action {
        "granary-list" => {
            let silos = granary::list_silos()?;

            let json = serde_json::to_string(&silos)?;

            println!("{}", json);
        }
        "granary-create" => {
            let reason = value.unwrap_or("Manual Backup");

            granary::create_silo(&config, "Manual Snapshot", reason)?;

            println!("Silo created.");
        }
        "granary-delete" => {
            if let Some(id) = value {
                granary::delete_silo(id)?;

                println!("Silo {} deleted.", id);
            } else {
                bail!("Missing Silo ID");
            }
        }
        "granary-restore" => {
            if let Some(id) = value {
                granary::restore_silo(id)?;

                println!("Silo {} restored. Please reboot.", id);
            } else {
                bail!("Missing Silo ID");
            }
        }
        "winnow-set" => {
            if let Some(val) = value
                && let Some((path, id)) = val.split_once(':')
            {
                config.winnowing.set_rule(path, id);

                config.save_to_file(CONFIG_FILE_DEFAULT)?;

                println!("Winnowing rule set: {} -> {}", path, id);
            }
        }
        _ => bail!("Unknown action: {}", action),
    }

    Ok(())
}
