// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use super::config::CONFIG_FILE_DEFAULT;

#[derive(Parser, Debug)]
#[command(name = "meta-hybrid", version, about = "Hybrid Mount Metamodule")]
pub struct Cli {
    #[arg(short = 'c', long = "config")]
    pub config: Option<PathBuf>,
    #[arg(short = 'm', long = "moduledir")]
    pub moduledir: Option<PathBuf>,
    #[arg(short = 's', long = "mountsource")]
    pub mountsource: Option<String>,
    #[arg(short = 'v', long = "verbose")]
    pub verbose: bool,
    #[arg(short = 'p', long = "partitions", value_delimiter = ',')]
    pub partitions: Vec<String>,
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    GenConfig {
        #[arg(short = 'o', long = "output", default_value = CONFIG_FILE_DEFAULT)]
        output: PathBuf,
    },
    ShowConfig,
    #[command(name = "save-config")]
    SaveConfig {
        #[arg(long)]
        payload: String,
    },
    #[command(name = "save-module-rules")]
    SaveModuleRules {
        #[arg(long)]
        module: String,
        #[arg(long)]
        payload: String,
    },
    Storage,
    Modules,
    Conflicts,
    Diagnostics,
    #[command(name = "system-action")]
    SystemAction {
        #[arg(long)]
        action: String,
        #[arg(long)]
        value: Option<String>,
    },
    Poaceae {
        #[arg(short, long, default_value = "/data/adb/poaceaefs_mount")]
        target: String,

        #[command(subcommand)]
        action: PoaceaeAction,
    },
}

#[derive(Subcommand, Debug)]
pub enum PoaceaeAction {
    Hide {
        name: String,
    },
    Unhide {
        name: String,
    },
    Redirect {
        src: String,
        #[arg(long)]
        dst: String,
    },
    Unredirect {
        src: String,
    },
    Spoof {
        name: String,
        #[arg(long, default_value_t = 0)]
        uid: u32,
        #[arg(long, default_value_t = 0)]
        gid: u32,
        #[arg(long, default_value_t = 0)]
        mode: u16,
        #[arg(long, default_value_t = 0)]
        mtime: u64,
    },
    Unspoof {
        name: String,
    },
    Merge {
        src: String,
        #[arg(long)]
        target: String,
    },
    Unmerge {
        src: String,
    },
    Trust {
        gid: u32,
    },
}
