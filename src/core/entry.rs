// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use anyhow::Result;

use crate::{conf::cli::Cli, core};

pub fn run(cli: Cli) -> Result<()> {
    if let Some(command) = &cli.command {
        return core::cli_commands::run(&cli, command);
    }

    core::startup::run(&cli)
}
