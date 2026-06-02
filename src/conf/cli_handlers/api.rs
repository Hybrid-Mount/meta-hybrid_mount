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
        let payload = api::build_features_payload();
        print_json(&payload, "features payload")
    }
}
