// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use anyhow::{Context, Result};

#[cfg(feature = "kasumi")]
use crate::core::api;

fn print_json<T: serde::Serialize>(payload: &T, description: &str) -> Result<()> {
    println!(
        "{}",
        serde_json::to_string_pretty(payload)
            .with_context(|| format!("Failed to serialize {description}"))?
    );
    Ok(())
}

pub fn handle_api_features() -> Result<()> {
    #[cfg(not(feature = "kasumi"))]
    {
        print_json(
            &serde_json::json!({ "bitmask": 0, "names": [] }),
            "features payload",
        )
    }

    #[cfg(feature = "kasumi")]
    {
        let payload = api::build_features_payload()?;
        print_json(&payload, "features payload")
    }
}
