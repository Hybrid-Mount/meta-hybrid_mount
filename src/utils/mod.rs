// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

pub mod path;
pub mod validation;

use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

use anyhow::Result;

pub use self::{path::*, validation::*};
#[macro_export]
macro_rules! scoped_log {
    ($level:ident, $scope:literal, $fmt:literal $(, $args:expr)* $(,)?) => {
        log::$level!(concat!("[", $scope, "] ", $fmt) $(, $args)*)
    };
}

pub fn get_mnt() -> PathBuf {
    for _ in 0..100 {
        let mut name = String::new();
        for _ in 0..10 {
            name.push(fastrand::alphanumeric());
        }
        let path = Path::new("/mnt").join(name);
        if !path.exists() {
            return path;
        }
    }
    // Fallback: use timestamp to avoid collision after exhausting random attempts
    Path::new("/mnt").join(format!("mnt_{}", std::process::id()))
}

pub fn init_logging() -> Result<()> {
    static LOGGER_INIT: OnceLock<()> = OnceLock::new();
    if LOGGER_INIT.get().is_some() {
        return Ok(());
    }

    #[cfg(target_os = "android")]
    {
        android_logger::init_once(
            android_logger::Config::default()
                .with_max_level(log::LevelFilter::Trace)
                .with_tag("Hybrid_Logger"),
        );
        LOGGER_INIT.set(()).ok();
    }

    #[cfg(not(target_os = "android"))]
    {
        use std::io::Write;

        let mut builder = env_logger::Builder::new();
        builder.format(|buf, record| {
            writeln!(
                buf,
                "[{}] [{}] {}",
                record.level(),
                record.target(),
                record.args()
            )
        });
        builder
            .filter_level(log::LevelFilter::Trace)
            .try_init()
            .ok();
        LOGGER_INIT.set(()).ok();
    }
    Ok(())
}
