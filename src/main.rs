// SPDX-License-Identifier: GPL-3.0-only

#![cfg_attr(not(any(target_os = "linux", target_os = "android")), allow(dead_code))]

mod cli;
mod defs;
mod errors;
mod framework;
mod logging;
mod mount;
mod platform;
#[cfg(test)]
mod test_support;
mod timing;

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
            process::exit(err.exit_code());
        }
    }
}

fn run() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    cli::run(&args)
}
