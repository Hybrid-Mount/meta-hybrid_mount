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

#[cfg(test)]
mod tests {
    use super::{extract_module_id, validate_module_id};
    use std::fs;

    use tempfile::tempdir;

    #[test]
    fn validate_module_id_accepts_valid_ids() {
        let valid_ids = ["Alpha", "alpha_1", "A.b-c_123"];
        for id in valid_ids {
            assert!(validate_module_id(id).is_ok(), "id should be valid: {id}");
        }
    }

    #[test]
    fn validate_module_id_rejects_invalid_ids() {
        let invalid_ids = ["1alpha", "alpha space", "", "-alpha"];
        for id in invalid_ids {
            assert!(validate_module_id(id).is_err(), "id should be invalid: {id}");
        }
    }

    #[test]
    fn extract_module_id_prefers_directory_with_module_prop() {
        let dir = tempdir().expect("failed to create temp dir");
        let module_dir = dir.path().join("my_module");
        let nested = module_dir.join("system/bin");
        fs::create_dir_all(&nested).expect("failed to create nested dir");
        fs::write(module_dir.join("module.prop"), "name=My Module\n")
            .expect("failed to write module.prop");

        let extracted = extract_module_id(&nested).expect("expected module id");
        assert_eq!(extracted, "my_module");
    }
}
