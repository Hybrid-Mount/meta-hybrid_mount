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

mod conf;
mod core;
mod defs;
mod domain;
mod mount;
mod sys;
mod utils;

use anyhow::Result;
use clap::Parser;
use conf::cli::Cli;
use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() -> Result<()> {
    if matches!(std::env::var("KSU_LATE_LOAD").as_deref(), Ok("1")) {
        panic!("不支持Late-load（越狱）模式");
    }

    let cli = Cli::parse();
    core::entry::run(cli)
}
