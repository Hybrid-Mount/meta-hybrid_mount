// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use anyhow::{Context, Result};

use crate::{
    conf::{
        cli::{ApiCommands, Cli, Commands, DaemonCommands},
        cli_handlers, loader,
    },
    core::{
        api,
        daemon::{
            self, dispatch,
            protocol::{ConfigCommand, DaemonCommand, ModulesCommand, SystemCommand},
        },
        startup,
    },
};

fn run_api_command<F>(f: F) -> Result<()>
where
    F: FnOnce() -> Result<()>,
{
    match f() {
        Ok(()) => Ok(()),
        Err(err) => {
            api::print_json_error(&err);
            Ok(())
        }
    }
}

pub fn run(cli: &Cli, command: &Commands) -> Result<()> {
    let _ = crate::utils::init_logging();

    match command {
        Commands::EmulatedSoftReboot => {
            let config = loader::load_config(cli)?;
            crate::core::late_load::detach_stale_mounts(&config).map(|_| ())
        }
        Commands::GenConfig { output, force } => cli_handlers::handle_gen_config(output, *force),
        Commands::Logs { lines } => cli_handlers::handle_logs(*lines),
        Commands::Api { command } => run_api_command(|| match api_daemon_command(command)? {
            Some(command) => dispatch(cli, command),
            None => cli_handlers::handle_api_features(),
        }),
        Commands::Daemon { command } => match command {
            DaemonCommands::Launch => startup::run_and_serve(cli),
            DaemonCommands::Serve => {
                let config = loader::load_config(cli)?;
                daemon::serve(config)
            }
            _ => run_api_command(|| dispatch(cli, daemon_daemon_command(command))),
        },
    }
}

fn api_daemon_command(command: &ApiCommands) -> Result<Option<DaemonCommand>> {
    Ok(Some(match command {
        ApiCommands::Storage => DaemonCommand::System(SystemCommand::ApiStorage),
        ApiCommands::MountStats => DaemonCommand::System(SystemCommand::ApiMountStats),
        ApiCommands::MountTopology => DaemonCommand::System(SystemCommand::ApiMountTopology),
        ApiCommands::Partitions => DaemonCommand::System(SystemCommand::ApiPartitions),
        ApiCommands::SystemInfo => DaemonCommand::System(SystemCommand::ApiSystemInfo),
        ApiCommands::Version => DaemonCommand::System(SystemCommand::ApiVersion),
        ApiCommands::ConfigGet => DaemonCommand::Config(ConfigCommand::Get),
        ApiCommands::ConfigSet { config } => DaemonCommand::Config(ConfigCommand::Set {
            config: parse_json(config, "Failed to parse config JSON payload")?,
        }),
        ApiCommands::ConfigPatch {
            patch,
            apply_runtime,
        } => DaemonCommand::Config(ConfigCommand::Patch {
            patch: parse_json(patch, "Failed to parse config patch JSON payload")?,
            apply_runtime: *apply_runtime,
        }),
        ApiCommands::ConfigReset => DaemonCommand::Config(ConfigCommand::Reset),
        ApiCommands::ModulesList { path } => {
            DaemonCommand::Modules(ModulesCommand::List { path: path.clone() })
        }
        ApiCommands::ModulesApply { modules } => DaemonCommand::Modules(ModulesCommand::Apply {
            modules: serde_json::from_str(modules)
                .context("Failed to parse modules JSON payload")?,
        }),
        ApiCommands::KernelUname => DaemonCommand::System(SystemCommand::ApiKernelUname),
        ApiCommands::OpenUrl { url } => {
            DaemonCommand::System(SystemCommand::ApiOpenUrl { url: url.clone() })
        }
        ApiCommands::Reboot => DaemonCommand::System(SystemCommand::ApiReboot),
    }))
}

fn daemon_daemon_command(command: &DaemonCommands) -> DaemonCommand {
    match command {
        DaemonCommands::Ping => DaemonCommand::System(SystemCommand::Ping),
        DaemonCommands::WebuiStart => DaemonCommand::System(SystemCommand::WebuiStart),
        DaemonCommands::Stop => DaemonCommand::System(SystemCommand::Shutdown),
        DaemonCommands::Status => DaemonCommand::System(SystemCommand::Status),
        DaemonCommands::Launch | DaemonCommands::Serve => unreachable!("handled before dispatch"),
    }
}
fn parse_json(input: &str, context: &'static str) -> Result<serde_json::Value> {
    serde_json::from_str(input).context(context)
}
