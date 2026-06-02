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
