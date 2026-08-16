// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::defs;

#[derive(Parser, Debug)]
#[command(name = "hybrid-mount", version, about = "Hybrid Mount Metamodule")]
pub struct Cli {
    #[arg(short = 'c', long = "config", default_value = defs::CONFIG_FILE)]
    pub config: PathBuf,
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Detach stale mounts before KernelSU's emulated soft reboot
    /// when running in late-load (jailbreak) mode.
    EmulatedSoftReboot,
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
        /// Deprecated compatibility flag. Runtime application is not supported
        /// after the Kasumi backend removal; changes take effect after a reboot.
        #[arg(long = "apply-runtime")]
        apply_runtime: bool,
        patch: String,
    },
    #[command(name = "config-reset")]
    ConfigReset,
    #[command(name = "modules-list")]
    ModulesList,
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
