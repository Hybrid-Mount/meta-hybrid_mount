// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use anyhow::{Context, Result};

#[cfg(feature = "kasumi")]
use crate::conf::cli::{HideCommands, KasumiCommands, KasumiRuleCommands, LkmCommands};
#[cfg(feature = "kasumi")]
use crate::core::daemon::protocol::KasumiCommand;
use crate::{
    conf::{
        cli::{ApiCommands, Cli, Commands, DaemonCommands},
        cli_handlers, loader,
    },
    core::{
        daemon::{
            self, dispatch,
            protocol::{ConfigCommand, DaemonCommand, ModulesCommand, SystemCommand},
        },
        startup,
    },
};

pub fn run(cli: &Cli, command: &Commands) -> Result<()> {
    crate::utils::init_logging()?;

    match command {
        Commands::EmulatedSoftReboot => {
            let config = loader::load_config(cli)?;
            crate::core::late_load::detach_stale_mounts(&config).map(|_| ())
        }
        Commands::GenConfig { output, force } => cli_handlers::handle_gen_config(output, *force),
        Commands::Logs { lines } => cli_handlers::handle_logs(*lines),
        Commands::Api { command } => match api_daemon_command(command)? {
            Some(command) => dispatch(cli, command),
            None => cli_handlers::handle_api_features(),
        },
        Commands::Daemon { command } => match command {
            DaemonCommands::Launch => startup::run_and_serve(cli),
            DaemonCommands::Serve => daemon::serve(),
            _ => dispatch(cli, daemon_daemon_command(command)),
        },
        #[cfg(feature = "kasumi")]
        Commands::Lkm { command } => dispatch(cli, lkm_daemon_command(command)),
        #[cfg(feature = "kasumi")]
        Commands::Hide { command } => dispatch(cli, hide_daemon_command(command)),
        #[cfg(feature = "kasumi")]
        Commands::Kasumi { command } => dispatch(cli, kasumi_daemon_command(command)),
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
        ApiCommands::ModulesList => DaemonCommand::Modules(ModulesCommand::List),
        ApiCommands::ModulesApply { modules } => DaemonCommand::Modules(ModulesCommand::Apply {
            modules: serde_json::from_str(modules)
                .context("Failed to parse modules JSON payload")?,
        }),
        ApiCommands::KernelUname => DaemonCommand::System(SystemCommand::ApiKernelUname),
        ApiCommands::OpenUrl { url } => {
            DaemonCommand::System(SystemCommand::ApiOpenUrl { url: url.clone() })
        }
        ApiCommands::Reboot => DaemonCommand::System(SystemCommand::ApiReboot),
        #[cfg(feature = "kasumi")]
        ApiCommands::KasumiMapsAdd { rule } => DaemonCommand::Kasumi(KasumiCommand::MapsAdd {
            rule: parse_json(rule, "Failed to parse Kasumi maps rule JSON payload")?,
        }),
        #[cfg(feature = "kasumi")]
        ApiCommands::KasumiMapsClear => DaemonCommand::Kasumi(KasumiCommand::MapsClear),
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

#[cfg(feature = "kasumi")]
fn lkm_daemon_command(command: &LkmCommands) -> DaemonCommand {
    match command {
        LkmCommands::Load => DaemonCommand::Kasumi(KasumiCommand::LkmLoad),
        LkmCommands::Unload => DaemonCommand::Kasumi(KasumiCommand::LkmUnload),
        LkmCommands::Status => DaemonCommand::Kasumi(KasumiCommand::LkmStatus),
    }
}

#[cfg(feature = "kasumi")]
fn hide_daemon_command(command: &HideCommands) -> DaemonCommand {
    match command {
        HideCommands::List => DaemonCommand::Kasumi(KasumiCommand::HideList),
        HideCommands::Add { path } => {
            DaemonCommand::Kasumi(KasumiCommand::HideAdd { path: path.clone() })
        }
        HideCommands::Remove { path } => {
            DaemonCommand::Kasumi(KasumiCommand::HideRemove { path: path.clone() })
        }
        HideCommands::Apply => DaemonCommand::Kasumi(KasumiCommand::HideApply),
    }
}

#[cfg(feature = "kasumi")]
fn kasumi_daemon_command(command: &KasumiCommands) -> DaemonCommand {
    match command {
        KasumiCommands::Status => DaemonCommand::Kasumi(KasumiCommand::Status),
        KasumiCommands::List => DaemonCommand::Kasumi(KasumiCommand::List),
        KasumiCommands::Version => DaemonCommand::Kasumi(KasumiCommand::Version),
        KasumiCommands::Features => DaemonCommand::Kasumi(KasumiCommand::Features),
        KasumiCommands::Hooks => DaemonCommand::Kasumi(KasumiCommand::Hooks),
        KasumiCommands::ApplyConfigRuntime => {
            DaemonCommand::Kasumi(KasumiCommand::ApplyConfigRuntime)
        }
        KasumiCommands::Clear => DaemonCommand::Kasumi(KasumiCommand::Clear),
        KasumiCommands::ReleaseConnection => {
            DaemonCommand::Kasumi(KasumiCommand::ReleaseConnection)
        }
        KasumiCommands::InvalidateCache => DaemonCommand::Kasumi(KasumiCommand::InvalidateCache),
        KasumiCommands::FixMounts => DaemonCommand::Kasumi(KasumiCommand::FixMounts),
        KasumiCommands::RestoreUnameGlobal => {
            DaemonCommand::Kasumi(KasumiCommand::RestoreUnameGlobal)
        }
        KasumiCommands::SetUname {
            mode,
            release,
            version,
        } => DaemonCommand::Kasumi(KasumiCommand::SetUname {
            mode: mode.clone(),
            release: release.clone(),
            version: version.clone(),
        }),
        KasumiCommands::ClearUname { mode } => {
            DaemonCommand::Kasumi(KasumiCommand::ClearUname { mode: mode.clone() })
        }
        KasumiCommands::Rule { command } => kasumi_rule_daemon_command(command),
    }
}

#[cfg(feature = "kasumi")]
fn kasumi_rule_daemon_command(command: &KasumiRuleCommands) -> DaemonCommand {
    match command {
        KasumiRuleCommands::Add {
            target,
            source,
            file_type,
        } => DaemonCommand::Kasumi(KasumiCommand::RuleAdd {
            target: target.clone(),
            source: source.clone(),
            file_type: *file_type,
        }),
        KasumiRuleCommands::Merge { target, source } => {
            DaemonCommand::Kasumi(KasumiCommand::RuleMerge {
                target: target.clone(),
                source: source.clone(),
            })
        }
        KasumiRuleCommands::Hide { path } => {
            DaemonCommand::Kasumi(KasumiCommand::RuleHide { path: path.clone() })
        }
        KasumiRuleCommands::Delete { path } => {
            DaemonCommand::Kasumi(KasumiCommand::RuleDelete { path: path.clone() })
        }
        KasumiRuleCommands::AddDir {
            target_base,
            source_dir,
        } => DaemonCommand::Kasumi(KasumiCommand::RuleAddDir {
            target_base: target_base.clone(),
            source_dir: source_dir.clone(),
        }),
        KasumiRuleCommands::RemoveDir {
            target_base,
            source_dir,
        } => DaemonCommand::Kasumi(KasumiCommand::RuleRemoveDir {
            target_base: target_base.clone(),
            source_dir: source_dir.clone(),
        }),
    }
}

fn parse_json(input: &str, context: &'static str) -> Result<serde_json::Value> {
    serde_json::from_str(input).context(context)
}
