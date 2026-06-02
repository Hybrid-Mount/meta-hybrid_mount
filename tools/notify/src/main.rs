// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{env, process::exit};

use anyhow::Result;
use notify::{NotifyRequest, send_output_dir_notification};

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let topic_id = if args.len() > 1 {
        Some(args[1].parse::<i64>()?)
    } else {
        None
    };
    let event_label = if args.len() > 2 {
        &args[2]
    } else {
        "New Yield (新产物)"
    };

    match send_output_dir_notification(
        &NotifyRequest::new("output", event_label).with_topic_id(topic_id),
    ) {
        Ok(()) => Ok(()),
        Err(error) => {
            eprintln!("Error: {error}");
            exit(1);
        }
    }
}
