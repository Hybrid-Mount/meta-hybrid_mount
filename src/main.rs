// Copyright 2025 Meta-Hybrid Mount Authors
// SPDX-License-Identifier: GPL-3.0-or-later
#![feature(lock_value_accessors)]

mod conf;
mod core;
mod defs;
mod mount;
#[cfg(any(target_os = "linux", target_os = "android"))]
mod try_umount;
mod utils;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Parser;
use conf::config::{CONFIG_FILE_DEFAULT, Config};
use mimalloc::MiMalloc;

use crate::core::img::copy_sparse_file;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn load_config() -> Result<Config> {
    match Config::load_default() {
        Ok(config) => Ok(config),
        Err(e) => {
            let is_not_found = e
                .root_cause()
                .downcast_ref::<std::io::Error>()
                .map(|io_err| io_err.kind() == std::io::ErrorKind::NotFound)
                .unwrap_or(false);

            if is_not_found {
                Ok(Config::default())
            } else {
                Err(e).context(format!(
                    "Failed to load default config from {}",
                    CONFIG_FILE_DEFAULT
                ))
            }
        }
    }
}

fn main() -> Result<()> {
    let mut config = load_config()?;

    #[cfg(all(target_os = "android", target_arch = "aarch64"))]
    {
        use log::LevelFilter;

        android_logger::init_once(
            android_logger::Config::default()
                .with_max_level(LevelFilter::Trace)
                .with_tag("Meta-Hybrid"),
        );
    }

    let args: Vec<_> = std::env::args().collect();

    if args.len() > 1 {
        match args[1].as_str() {
            "xcp" => copy_sparse_file(args[2].clone(), args[3].clone(), false)?,
            _ => {}
        }
    }

    if utils::check_zygisksu_enforce_status() {
        if config.allow_umount_coexistence {
            if config.verbose {
                println!(
                    ">> ZygiskSU Enforce!=0 detected, but Umount Coexistence enabled. Respecting \
                     user config."
                );
            }
        } else {
            if config.verbose {
                println!(">> ZygiskSU Enforce!=0 detected. Forcing DISABLE_UMOUNT to TRUE.");
            }

            config.disable_umount = true;
        }
    }

    core::mount(config)?;

    Ok(())
}
