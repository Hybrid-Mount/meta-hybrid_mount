// ReHybrid-Mount
//
// SPDX-License-Identifier: GPL-3.0-only

mod cli;
mod config;
mod defs;
mod errors;
mod logging;
mod magic_mount;
#[cfg(any(target_os = "linux", target_os = "android", test))]
mod module_status;
mod overlayfs;
mod pipeline;
mod plan;
mod scanner;
mod state;
mod storage;
mod sys;
mod utils;

use std::env;
use std::process;

use errors::Result;

fn main() {
    logging::init();
    logging::install_panic_hook();

    match run() {
        Ok(()) => {}
        Err(err) => {
            log::error!("{err}");
            eprintln!("hybrid-mount: {err}");
            process::exit(1);
        }
    }
}

fn run() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    cli::run(&args)
}
