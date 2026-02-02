// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    path::Path,
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

use anyhow::{Result, bail};
use regex_lite::Regex;

use crate::defs;

pub static KSU: AtomicBool = AtomicBool::new(false);

static MODULE_ID_REGEX: OnceLock<Regex> = OnceLock::new();

pub fn check_ksu() {
    let status = ksu::version().is_some();
    KSU.store(status, Ordering::Relaxed);
}

pub fn validate_module_id(module_id: &str) -> Result<()> {
    let re = MODULE_ID_REGEX
        .get_or_init(|| Regex::new(r"^[a-zA-Z][a-zA-Z0-9._-]+$").expect("Invalid Regex pattern"));
    if re.is_match(module_id) {
        Ok(())
    } else {
        bail!("Invalid module ID: '{module_id}'. Must match /^[a-zA-Z][a-zA-Z0-9._-]+$/")
    }
}

pub fn extract_module_id(path: &Path) -> Option<String> {
    let mut current = path;
    loop {
        if current.join("module.prop").exists() {
            return current.file_name().map(|s| s.to_string_lossy().to_string());
        }
        match current.parent() {
            Some(p) => current = p,
            None => break,
        }
    }

    path.parent()
        .and_then(|p| p.file_name())
        .map(|s| s.to_string_lossy().to_string())
}

pub fn check_zygisksu_enforce_status() -> bool {
    std::fs::read_to_string(defs::ZYGISKSU_DENYLIST_FILE)
        .map(|s| s.trim() != "0")
        .unwrap_or(false)
}
