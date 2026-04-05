// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

mod conf;
mod core;
mod defs;
mod mount;
mod sys;
mod utils;

use anyhow::Result;
use clap::Parser;
use conf::cli::Cli;
use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() -> Result<()> {
    if std::env::var("KSU_LATE_LOAD").is_ok() && std::env::var("KSU").is_ok() {
        panic!("! unsupported late load mode");
    }
    let cli = Cli::parse();
    core::entry::run(cli)
}
