// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::{Serialize, de::DeserializeOwned};

use crate::{
    conf::{
        cli::Cli,
        config::{self, Config},
        loader::{self, LoadPolicy},
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

fn decode_hex_json<T: DeserializeOwned>(payload: &str, type_name: &str) -> Result<T> {
    if !payload.len().is_multiple_of(2) {
        bail!("Invalid hex payload length for {}", type_name);
    }

    let json_bytes = (0..payload.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&payload[i..i + 2], 16))
        .collect::<Result<Vec<u8>, _>>()
        .with_context(|| format!("Failed to decode hex payload for {}", type_name))?;

    serde_json::from_slice(&json_bytes)
        .with_context(|| format!("Failed to parse {} JSON payload", type_name))
}

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

pub fn handle_show_config(cli: &Cli) -> Result<()> {
    let config = loader::load_config(cli, LoadPolicy::ErrorOnInvalidDefault)?;

    let json = serde_json::to_string(&config).context("Failed to serialize config to JSON")?;

    println!("{}", json);

    Ok(())
}

pub fn handle_save_config(payload: &str) -> Result<()> {
    let config: Config = decode_hex_json(payload, "config")?;

    config
        .save_to_file(defs::CONFIG_FILE)
        .context("Failed to save config file")?;

    println!("Configuration saved successfully.");

    Ok(())
}

pub fn handle_save_module_rules(module_id: &str, payload: &str) -> Result<()> {
    utils::validate_module_id(module_id)?;
    let new_rules: config::ModuleRules = decode_hex_json(payload, "module rules")?;
    let mut config = Config::load_default().unwrap_or_default();

    config.rules.insert(module_id.to_string(), new_rules);

    config
        .save_to_file(defs::CONFIG_FILE)
        .context("Failed to update config file with new rules")?;

    println!("Module rules saved for {} into config.toml", module_id);

    Ok(())
}

pub fn handle_modules(cli: &Cli) -> Result<()> {
    let config = loader::load_config(cli, LoadPolicy::ErrorOnInvalidDefault)?;

    modules::print_list(&config).context("Failed to list modules")
}

pub fn handle_analyze(cli: &Cli, kind: &str) -> Result<()> {
    let config = loader::load_config(cli, LoadPolicy::ErrorOnInvalidDefault)?;

    let module_list = inventory::scan(&config.moduledir, &config)
        .context("Failed to scan modules for diagnostics")?;

    let plan = planner::generate(&config, &module_list, &config.moduledir)
        .context("Failed to generate plan for diagnostics")?;

    let report = plan.analyze();

    match kind {
        "conflicts" => {
            let json = serde_json::to_string(&report.conflicts)
                .context("Failed to serialize conflict report")?;
            println!("{}", json);
        }
        "diagnostics" => {
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

            let json = serde_json::to_string(&json_issues)
                .context("Failed to serialize diagnostics report")?;
            println!("{}", json);
        }
        "all" => {
            let json = serde_json::to_string(&report).context("Failed to serialize report")?;
            println!("{}", json);
        }
        _ => bail!("Unsupported analyze kind: {}", kind),
    }

    Ok(())
}
