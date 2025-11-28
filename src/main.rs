#![deny(clippy::all, clippy::pedantic)]
#![warn(clippy::nursery)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::cast_possible_wrap
)]

mod config;
mod defs;
mod magic_mount;
mod utils;

use std::io::Write;

use anyhow::{Context, Result};
use env_logger::Builder;

use crate::{
    config::Config,
    defs::{CONFIG_FILE_DEFAULT, DISABLE_FILE_NAME, REMOVE_FILE_NAME, SKIP_MOUNT_FILE_NAME},
    magic_mount::UMOUNT,
};

fn load_config() -> Config {
    if let Ok(config) = Config::load_default() {
        log::info!("Loaded config from default location: {CONFIG_FILE_DEFAULT}",);
        return config;
    }

    log::info!("Using default configuration (no config file found)");
    Config::default()
}

fn init_logger(verbose: bool) {
    let level = if verbose {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Info
    };

    let mut builder = Builder::new();

    builder.format(|buf, record| {
        writeln!(
            buf,
            "[{}] [{}] {}",
            record.level(),
            record.target(),
            record.args()
        )
    });
    builder.filter_level(level).init();

    log::info!("log level: {}", level.as_str());
}

fn main() -> Result<()> {
    // 加载配置
    let config = load_config();

    let args: Vec<_> = std::env::args().collect();
    if args.len() > 1 && args[1] == "scan" {
        let mut modules = Vec::new();
        for entry in config.moduledir.read_dir()?.flatten() {
            if !entry.file_type()?.is_dir() {
                continue;
            }

            if entry.path().join(DISABLE_FILE_NAME).exists()
                || entry.path().join(REMOVE_FILE_NAME).exists()
                || entry.path().join(SKIP_MOUNT_FILE_NAME).exists()
            {
                continue;
            }

            let mod_system = entry.path().join("system");
            if !mod_system.is_dir() {
                continue;
            }

            log::debug!("collecting {}", entry.path().display());

            modules.push(entry.file_name().into_string().unwrap());
            //        has_file |= system.collect_module_files(&mod_system)?;
        }

        for module in modules {
            println!("{module}");
        }
        return Ok(());
    }
    // 初始化日志
    init_logger(config.verbose);

    log::info!("Magic Mount Starting");
    log::info!("module dir      : {}", config.moduledir.display());

    let tempdir = if let Some(temp) = config.tempdir {
        log::info!("temp dir (cfg)  : {}", temp.display());
        temp
    } else {
        let temp = utils::select_temp_dir().context("failed to select temp dir automatically")?;
        log::info!("temp dir (auto) : {}", temp.display());
        temp
    };

    log::info!("mount source    : {}", config.mountsource);
    log::info!("verbose mode    : {}", config.verbose);
    log::info!(
        "extra partitions: {}",
        if config.partitions.is_empty() {
            "None".to_string()
        } else {
            format!("{:?}", config.partitions)
        }
    );

    utils::ensure_temp_dir(&tempdir)?;

    if config.umount {
        UMOUNT.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    let result = magic_mount::magic_mount(
        &tempdir,
        &config.moduledir,
        &config.mountsource,
        &config.partitions,
    );

    utils::cleanup_temp_dir(&tempdir);

    match result {
        Ok(()) => {
            log::info!("Magic Mount Completed Successfully");
            Ok(())
        }
        Err(e) => {
            log::error!("Magic Mount Failed");
            for cause in e.chain() {
                log::error!("{cause:#?}");
            }
            log::error!("{:#?}", e.backtrace());
            Err(e)
        }
    }
}
