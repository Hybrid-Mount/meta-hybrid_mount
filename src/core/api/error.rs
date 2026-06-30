// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ErrorPayload {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

/// Print a structured JSON error to stdout and exit with code 0.
/// This ensures the frontend can parse error details from ksu.exec() output
/// rather than relying solely on stderr + exit code.
pub fn print_json_error(err: &anyhow::Error) {
    let payload = ErrorPayload {
        kind: "error",
        error: format!("{:#}", err),
        code: None,
    };
    println!(
        "{}",
        serde_json::to_string(&payload).unwrap_or_else(|_| {
            r#"{"type":"error","error":"failed to serialize error payload"}"#.to_string()
        })
    );
}
