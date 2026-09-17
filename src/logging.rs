// SPDX-License-Identifier: GPL-3.0-only

//!
//! logcat on Android, stderr on the host (development and unit tests).
//! Everything goes through the `log` facade; `RUST_LOG` overrides the level, which defaults to `info`.

use std::env;
use std::panic;
use std::thread;

use log::LevelFilter;
#[cfg(not(target_os = "android"))]
use log::{Log, Metadata, Record};

/// Initialises the log backend and sets the global level. Safe to call repeatedly.
pub fn init() {
    let level = detect_level_filter();
    log::set_max_level(level);

    #[cfg(target_os = "android")]
    android_logger::init_once(android_logger::Config::default().with_max_level(level));

    #[cfg(not(target_os = "android"))]
    if log::set_logger(&StderrLogger).is_ok() {
        log::debug!("stderr logger installed");
    }
}

/// Installs a panic hook that records the thread name, location and payload, then keeps the default output.
pub fn install_panic_hook() {
    let default_hook = panic::take_hook();

    panic::set_hook(Box::new(move |info| {
        let thread_name = thread::current()
            .name()
            .map_or_else(|| "<unnamed>".to_owned(), str::to_owned);
        let location = info
            .location()
            .map_or_else(|| "<unknown location>".to_owned(), ToString::to_string);
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| info.payload().downcast_ref::<String>().map(String::as_str))
            .unwrap_or("<non-string panic payload>");

        log::error!("panic in thread '{thread_name}' at {location}: {message}");

        default_hook(info);
    }));
}

fn detect_level_filter() -> LevelFilter {
    let Ok(value) = env::var("RUST_LOG") else {
        return LevelFilter::Info;
    };

    match value.to_ascii_lowercase().as_str() {
        "off" => LevelFilter::Off,
        "error" => LevelFilter::Error,
        "warn" => LevelFilter::Warn,
        "debug" => LevelFilter::Debug,
        "trace" => LevelFilter::Trace,
        _ => LevelFilter::Info,
    }
}

#[cfg(not(target_os = "android"))]
struct StderrLogger;

#[cfg(not(target_os = "android"))]
impl Log for StderrLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            eprintln!(
                "[{}] {}: {}",
                record.level(),
                record.target(),
                record.args()
            );
        }
    }

    fn flush(&self) {}
}
