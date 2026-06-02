// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use anyhow::{Context, Result};

#[cfg(feature = "kasumi")]
use crate::conf::cli::{HideCommands, KasumiCommands, KasumiRuleCommands, LkmCommands};
use crate::{
    conf::{
        cli::{ApiCommands, Cli, Commands, DaemonCommands},
        cli_handlers, loader,
    },
    core::{
        api,
        daemon::{self, DaemonCommand, dispatch},
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
        ApiCommands::Storage => DaemonCommand::ApiStorage,
        ApiCommands::MountStats => DaemonCommand::ApiMountStats,
        ApiCommands::MountTopology => DaemonCommand::ApiMountTopology,
        ApiCommands::Partitions => DaemonCommand::ApiPartitions,
        ApiCommands::SystemInfo => DaemonCommand::ApiSystemInfo,
        ApiCommands::Version => DaemonCommand::ApiVersion,
        ApiCommands::ConfigGet => DaemonCommand::ApiConfigGet,
        ApiCommands::ConfigSet { config } => DaemonCommand::ApiConfigSet {
            config: parse_json(config, "Failed to parse config JSON payload")?,
        },
        ApiCommands::ConfigPatch {
            patch,
            apply_runtime,
        } => DaemonCommand::ApiConfigPatch {
            patch: parse_json(patch, "Failed to parse config patch JSON payload")?,
            apply_runtime: *apply_runtime,
        },
        ApiCommands::ConfigReset => DaemonCommand::ApiConfigReset,
        ApiCommands::ModulesList { path } => DaemonCommand::ApiModulesList { path: path.clone() },
        ApiCommands::ModulesApply { modules } => DaemonCommand::ApiModulesApply {
            modules: serde_json::from_str(modules)
                .context("Failed to parse modules JSON payload")?,
        },
        #[cfg(feature = "kasumi")]
        ApiCommands::Lkm => DaemonCommand::ApiLkm,
        #[cfg(feature = "kasumi")]
        ApiCommands::Features => return Ok(None),
        #[cfg(feature = "kasumi")]
        ApiCommands::Hooks => DaemonCommand::ApiHooks,
        ApiCommands::KernelUname => DaemonCommand::ApiKernelUname,
        ApiCommands::OpenUrl { url } => DaemonCommand::ApiOpenUrl { url: url.clone() },
        ApiCommands::Reboot => DaemonCommand::ApiReboot,
        #[cfg(feature = "kasumi")]
        ApiCommands::KasumiMapsAdd { rule } => DaemonCommand::ApiKasumiMapsAdd {
            rule: parse_json(rule, "Failed to parse Kasumi maps rule JSON payload")?,
        },
        #[cfg(feature = "kasumi")]
        ApiCommands::KasumiMapsClear => DaemonCommand::ApiKasumiMapsClear,
    }))
}

fn daemon_daemon_command(command: &DaemonCommands) -> DaemonCommand {
    match command {
        DaemonCommands::Ping => DaemonCommand::Ping,
        DaemonCommands::WebuiStart => DaemonCommand::WebuiStart,
        DaemonCommands::Stop => DaemonCommand::Shutdown,
        DaemonCommands::Status => DaemonCommand::Status,
        DaemonCommands::Launch | DaemonCommands::Serve => unreachable!("handled before dispatch"),
    }
}

#[cfg(feature = "kasumi")]
fn lkm_daemon_command(command: &LkmCommands) -> DaemonCommand {
    match command {
        LkmCommands::Load => DaemonCommand::LkmLoad,
        LkmCommands::Unload => DaemonCommand::LkmUnload,
        LkmCommands::Status => DaemonCommand::LkmStatus,
    }
}

#[cfg(feature = "kasumi")]
fn hide_daemon_command(command: &HideCommands) -> DaemonCommand {
    match command {
        HideCommands::List => DaemonCommand::HideList,
        HideCommands::Add { path } => DaemonCommand::HideAdd { path: path.clone() },
        HideCommands::Remove { path } => DaemonCommand::HideRemove { path: path.clone() },
        HideCommands::Apply => DaemonCommand::HideApply,
    }
}

#[cfg(feature = "kasumi")]
fn kasumi_daemon_command(command: &KasumiCommands) -> DaemonCommand {
    match command {
        KasumiCommands::Status => DaemonCommand::KasumiStatus,
        KasumiCommands::List => DaemonCommand::KasumiList,
        KasumiCommands::Version => DaemonCommand::KasumiVersion,
        KasumiCommands::Features => DaemonCommand::KasumiFeatures,
        KasumiCommands::Hooks => DaemonCommand::KasumiHooks,
        KasumiCommands::ApplyConfigRuntime => DaemonCommand::KasumiApplyConfigRuntime,
        KasumiCommands::Clear => DaemonCommand::KasumiClear,
        KasumiCommands::ReleaseConnection => DaemonCommand::KasumiReleaseConnection,
        KasumiCommands::InvalidateCache => DaemonCommand::KasumiInvalidateCache,
        KasumiCommands::FixMounts => DaemonCommand::KasumiFixMounts,
        KasumiCommands::RestoreUnameGlobal => DaemonCommand::KasumiRestoreUnameGlobal,
        KasumiCommands::SetUname {
            mode,
            release,
            version,
        } => DaemonCommand::KasumiSetUname {
            mode: mode.clone(),
            release: release.clone(),
            version: version.clone(),
        },
        KasumiCommands::ClearUname { mode } => {
            DaemonCommand::KasumiClearUname { mode: mode.clone() }
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
        } => DaemonCommand::KasumiRuleAdd {
            target: target.clone(),
            source: source.clone(),
            file_type: *file_type,
        },
        KasumiRuleCommands::Merge { target, source } => DaemonCommand::KasumiRuleMerge {
            target: target.clone(),
            source: source.clone(),
        },
        KasumiRuleCommands::Hide { path } => DaemonCommand::KasumiRuleHide { path: path.clone() },
        KasumiRuleCommands::Delete { path } => {
            DaemonCommand::KasumiRuleDelete { path: path.clone() }
        }
        KasumiRuleCommands::AddDir {
            target_base,
            source_dir,
        } => DaemonCommand::KasumiRuleAddDir {
            target_base: target_base.clone(),
            source_dir: source_dir.clone(),
        },
        KasumiRuleCommands::RemoveDir {
            target_base,
            source_dir,
        } => DaemonCommand::KasumiRuleRemoveDir {
            target_base: target_base.clone(),
            source_dir: source_dir.clone(),
        },
    }
}

fn parse_json(input: &str, context: &'static str) -> Result<serde_json::Value> {
    serde_json::from_str(input).context(context)
}
