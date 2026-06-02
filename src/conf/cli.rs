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
    #[cfg(feature = "kasumi")]
    Lkm {
        #[command(subcommand)]
        command: LkmCommands,
    },
    #[cfg(feature = "kasumi")]
    Hide {
        #[command(subcommand)]
        command: HideCommands,
    },
    #[cfg(feature = "kasumi")]
    Kasumi {
        #[command(subcommand)]
        command: KasumiCommands,
    },
}

#[cfg(feature = "kasumi")]
#[derive(Subcommand, Debug)]
pub enum KasumiCommands {
    Status,
    List,
    Version,
    Features,
    Hooks,
    #[command(name = "apply-config-runtime")]
    ApplyConfigRuntime,
    Clear,
    #[command(name = "release-connection")]
    ReleaseConnection,
    #[command(name = "invalidate-cache")]
    InvalidateCache,
    #[command(name = "fix-mounts")]
    FixMounts,
    #[command(name = "restore-uname-global")]
    RestoreUnameGlobal,
    #[command(name = "set-uname")]
    SetUname {
        #[arg(long = "mode", default_value = "scoped")]
        mode: String,
        release: String,
        version: String,
    },
    #[command(name = "clear-uname")]
    ClearUname {
        #[arg(long = "mode", default_value = "scoped")]
        mode: String,
    },
    Rule {
        #[command(subcommand)]
        command: KasumiRuleCommands,
    },
}

#[cfg(feature = "kasumi")]
#[derive(Subcommand, Debug)]
pub enum HideCommands {
    List,
    Add { path: PathBuf },
    Remove { path: PathBuf },
    Apply,
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
    #[cfg(feature = "kasumi")]
    Lkm,
    #[cfg(feature = "kasumi")]
    Features,
    #[cfg(feature = "kasumi")]
    Hooks,
    #[command(name = "kernel-uname")]
    KernelUname,
    #[command(name = "open-url")]
    OpenUrl {
        url: String,
    },
    Reboot,
    #[cfg(feature = "kasumi")]
    #[command(name = "kasumi-maps-add")]
    KasumiMapsAdd {
        rule: String,
    },
    #[cfg(feature = "kasumi")]
    #[command(name = "kasumi-maps-clear")]
    KasumiMapsClear,
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

#[cfg(feature = "kasumi")]
#[derive(Subcommand, Debug)]
pub enum LkmCommands {
    Load,
    Unload,
    Status,
}

#[cfg(feature = "kasumi")]
#[derive(Subcommand, Debug)]
pub enum KasumiRuleCommands {
    Add {
        target: PathBuf,
        source: PathBuf,
        #[arg(long = "type")]
        file_type: Option<i32>,
    },
    Merge {
        target: PathBuf,
        source: PathBuf,
    },
    Hide {
        path: PathBuf,
    },
    Delete {
        path: PathBuf,
    },
    #[command(name = "add-dir")]
    AddDir {
        target_base: PathBuf,
        source_dir: PathBuf,
    },
    #[command(name = "remove-dir")]
    RemoveDir {
        target_base: PathBuf,
        source_dir: PathBuf,
    },
}
