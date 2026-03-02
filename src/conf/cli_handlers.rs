// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::{
    conf::{
        cli::Cli,
        config::{self, Config},
    },
    core::{inventory, inventory::model as modules, ops::planner},
    defs, utils,
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
                    defs::CONFIG_FILE
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

pub fn handle_save_config(payload: &str) -> Result<()> {
    let json_bytes = (0..payload.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&payload[i..i + 2], 16))
        .collect::<Result<Vec<u8>, _>>()
        .context("Failed to decode hex payload")?;

    let config: Config =
        serde_json::from_slice(&json_bytes).context("Failed to parse config JSON payload")?;

    config
        .save_to_file(defs::CONFIG_FILE)
        .context("Failed to save config file")?;

    println!("Configuration saved successfully.");

    Ok(())
}

pub fn handle_save_module_rules(module_id: &str, payload: &str) -> Result<()> {
    utils::validate_module_id(module_id)?;
    let json_bytes = (0..payload.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&payload[i..i + 2], 16))
        .collect::<Result<Vec<u8>, _>>()
        .context("Failed to decode hex payload")?;

    let new_rules: config::ModuleRules =
        serde_json::from_slice(&json_bytes).context("Failed to parse module rules JSON")?;
    let mut config = Config::load_default().unwrap_or_default();

    config.rules.insert(module_id.to_string(), new_rules);

    config
        .save_to_file(defs::CONFIG_FILE)
        .context("Failed to update config file with new rules")?;

    println!("Module rules saved for {} into config.toml", module_id);

    Ok(())
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
