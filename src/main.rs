// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use anyhow::Result;
#[cfg(feature = "control-plane")]
use clap::Parser;
#[cfg(feature = "control-plane")]
use hybrid_mount::conf::cli::Cli;
use hybrid_mount::core;

fn main() -> Result<()> {
    #[cfg(feature = "control-plane")]
    if matches!(std::env::var("KSU_LATE_LOAD").as_deref(), Ok("1")) {
        eprintln!("Late-load (jailbreak) mode is not supported");
        std::process::exit(1);
    }

    #[cfg(feature = "control-plane")]
    {
        let cli = Cli::parse();
        core::entry::run(cli)
    }

    #[cfg(not(feature = "control-plane"))]
    {
        core::startup::run_default()
    }
}
