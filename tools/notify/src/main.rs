// SPDX-License-Identifier: GPL-3.0-only

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use notify::{NotifyRequest, maybe_send_output_dir_notification};

#[derive(Parser)]
#[command(version, about = "Send Hybrid Mount build artifacts to Telegram")]
struct Cli {
    #[arg(long, default_value = "output")]
    output: PathBuf,
    #[arg(long)]
    label: String,
    #[arg(long)]
    topic_id: Option<i64>,
}

fn main() -> Result<()> {
    let args = Cli::parse();
    let request = NotifyRequest::new(args.output, args.label).with_topic_id(args.topic_id);
    if maybe_send_output_dir_notification(&request)? {
        println!("notification sent");
    }
    Ok(())
}
