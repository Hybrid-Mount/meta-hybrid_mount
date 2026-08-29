// SPDX-License-Identifier: GPL-3.0-only

//! 日志初始化与 panic hook。
//!
//! Android 上输出到 logcat,主机侧(开发/单测)输出到 stderr。
//! 统一通过 `log` facade,日志级别可用 `RUST_LOG` 覆盖,默认 `info`。

use std::env;
use std::panic;
use std::thread;

use log::LevelFilter;
#[cfg(not(target_os = "android"))]
use log::{Log, Metadata, Record};

/// 初始化日志后端并设置全局级别。重复调用是安全的。
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

/// 安装 panic hook:记录线程名、位置与 payload 后保留默认输出。
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
