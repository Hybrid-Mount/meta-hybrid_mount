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

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::defs;

#[derive(Parser, Debug)]
#[command(name = "hybrid-mount", version, about = "Hybrid Mount Metamodule")]
pub struct Cli {
    #[arg(short = 'c', long = "config")]
    pub config: Option<PathBuf>,
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    GenConfig {
        #[arg(short = 'o', long = "output", default_value = defs::CONFIG_FILE)]
        output: PathBuf,
        #[arg(long)]
        force: bool,
    },
    Logs {
        #[arg(long, default_value_t = 200)]
        lines: usize,
    },
    Api {
        #[command(subcommand)]
        command: ApiCommands,
    },
    Daemon {
        #[command(subcommand)]
        command: DaemonCommands,
    },
}

#[derive(Subcommand, Debug)]
pub enum ApiCommands {
    Storage,
    #[command(name = "mount-stats")]
    MountStats,
    #[command(name = "mount-topology")]
    MountTopology,
    Partitions,
    #[command(name = "system-info")]
    SystemInfo,
    Version,
    #[command(name = "config-get")]
    ConfigGet,
    #[command(name = "config-set")]
    ConfigSet {
        config: String,
    },
    #[command(name = "config-patch")]
    ConfigPatch {
        #[arg(long = "apply-runtime")]
        apply_runtime: bool,
        patch: String,
    },
    #[command(name = "config-reset")]
    ConfigReset,
    #[command(name = "modules-list")]
    ModulesList {
        #[arg(long)]
        path: Option<PathBuf>,
    },
    #[command(name = "modules-apply")]
    ModulesApply {
        modules: String,
    },
    #[command(name = "kernel-uname")]
    KernelUname,
    #[command(name = "open-url")]
    OpenUrl {
        url: String,
    },
    Reboot,
}

#[derive(Subcommand, Debug)]
pub enum DaemonCommands {
    Launch,
    Serve,
    Ping,
    #[command(name = "webui-start")]
    WebuiStart,
    Stop,
    Status,
}
