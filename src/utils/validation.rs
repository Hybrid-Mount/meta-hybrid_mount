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

use std::{
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

use anyhow::{Result, bail};

pub static KSU: AtomicBool = AtomicBool::new(false);

pub fn check_ksu() {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    let status = ksu::version().is_some();

    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    let status = false;

    KSU.store(status, Ordering::Relaxed);
}

pub fn validate_module_id(module_id: &str) -> Result<()> {
    let mut chars = module_id.bytes();
    let valid = matches!(chars.next(), Some(b'a'..=b'z' | b'A'..=b'Z'))
        && chars
            .next()
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        && chars.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'));

    if valid {
        Ok(())
    } else {
        bail!("Invalid module ID: '{module_id}'. Must match /^[a-zA-Z][a-zA-Z0-9._-]+$/")
    }
}

pub fn extract_module_id(path: &Path) -> Option<String> {
    const MAX_DEPTH: usize = 64;
    let mut current = path;
    for _ in 0..MAX_DEPTH {
        if current.join("module.prop").exists() {
            return current
                .file_name()
                .map(|s| s.to_string_lossy().into_owned());
        }
        match current.parent() {
            Some(p) => current = p,
            None => break,
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn validate_module_id_accepts_and_rejects() {
        assert!(validate_module_id("MyModule").is_ok());
        assert!(validate_module_id("ab").is_ok());
        assert!(validate_module_id("a1_b.c-d").is_ok());
        assert!(validate_module_id("ABC123").is_ok());

        assert!(validate_module_id("").is_err());
        assert!(validate_module_id("1abc").is_err());
        assert!(validate_module_id("-abc").is_err());
        assert!(validate_module_id("ab/cd").is_err());
        assert!(validate_module_id("ab cd").is_err());
        assert!(validate_module_id("ab@cd").is_err());
    }

    #[test]
    fn extract_from_module_prop() {
        let tmp = TempDir::new().unwrap();
        let module_dir = tmp.path().join("my_module");
        fs::create_dir(&module_dir).unwrap();
        fs::write(module_dir.join("module.prop"), "").unwrap();

        let id = extract_module_id(&module_dir.join("system/app"));
        assert_eq!(id.as_deref(), Some("my_module"));
    }

    #[test]
    fn extract_returns_none_without_module_prop() {
        let tmp = TempDir::new().unwrap();
        let module_dir = tmp.path().join("fallback_mod");
        fs::create_dir_all(module_dir.join("sub/dir")).unwrap();
        // no module.prop anywhere — can't determine module ID
        let id = extract_module_id(&module_dir.join("sub/dir/leaf"));
        assert_eq!(id, None);
    }

    #[test]
    fn extract_at_root_returns_none() {
        let id = extract_module_id(Path::new("/"));
        assert_eq!(id, None);
    }
}
