// ReHybrid-Mount
//
// SPDX-License-Identifier: GPL-3.0-only

mod config;
mod defs;
mod errors;
mod logging;
mod magic_mount;
mod utils;

use std::env;
use std::process;

use errors::{Error, Result};

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

/// Stage 1 的 CLI 占位:完整命令分派(无参数挂载流水线、
/// show-config / save-config / gen-config 等)在 Stage 5 实现。
fn run() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.is_empty() {
        return Err(Error::msg(
            "mount pipeline is not implemented yet (planned for Stage 5)",
        ));
    }

    Err(Error::msg(format!("unknown command: {}", args.join(" "))))
}
