// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

pub mod discovery;
pub mod listing;

use std::fs;

pub use discovery::*;

#[cfg(not(feature = "control-plane"))]
use crate::domain::MountMode;
use crate::{conf::config::Config, defs, domain::ModuleRules};

pub fn load_module_rules(config: &Config, module_id: &str) -> ModuleRules {
    let mut rules = ModuleRules {
        default_mode: config.default_mode.as_mount_mode(),
        ..Default::default()
    };

    if let Some(global_rules) = config.rules.get(module_id) {
        rules.default_mode = global_rules.default_mode;
        rules.paths.extend(global_rules.paths.clone());
    }

    #[cfg(not(feature = "control-plane"))]
    if let Some(marker_mode) = module_mount_mode_marker(&config.moduledir.join(module_id)) {
        rules.default_mode = marker_mode;
    }

    rules
}

#[cfg(not(feature = "control-plane"))]
pub fn module_mount_mode_marker(module_path: &std::path::Path) -> Option<MountMode> {
    let markers = scan_known_markers(module_path);
    if markers.overlay {
        Some(MountMode::Overlay)
    } else if markers.magic {
        Some(MountMode::Magic)
    } else {
        None
    }
}

pub fn is_reserved_module_dir(id: &str) -> bool {
    matches!(
        id,
        "hybrid-mount"
            | "hybrid_mount"
            | "lost+found"
            | ".git"
            | ".github"
            | ".hg"
            | ".idea"
            | ".svn"
            | ".vscode"
            | "__pycache__"
            | "node_modules"
    )
}

pub fn mount_block_markers(module_path: &std::path::Path) -> Vec<&'static str> {
    let found = scan_known_markers(module_path);
    let mut markers = Vec::new();
    if found.disable {
        markers.push(defs::DISABLE_FILE_NAME);
    }
    if found.remove {
        markers.push(defs::REMOVE_FILE_NAME);
    }
    if found.mount_error {
        markers.push(defs::MOUNT_ERROR_FILE_NAME);
    }
    if found.skip_mount {
        markers.push(defs::SKIP_MOUNT_FILE_NAME);
    }
    markers
}

#[derive(Default)]
struct KnownMarkers {
    disable: bool,
    remove: bool,
    mount_error: bool,
    skip_mount: bool,
    overlay: bool,
    magic: bool,
}

fn scan_known_markers(module_path: &std::path::Path) -> KnownMarkers {
    let mut found = KnownMarkers::default();
    let Ok(entries) = fs::read_dir(module_path) else {
        return found;
    };

    for entry in entries.flatten() {
        let name = entry.file_name();
        let bytes = name.as_encoded_bytes();
        found.disable |= bytes.eq_ignore_ascii_case(defs::DISABLE_FILE_NAME.as_bytes());
        found.remove |= bytes.eq_ignore_ascii_case(defs::REMOVE_FILE_NAME.as_bytes());
        found.mount_error |= bytes.eq_ignore_ascii_case(defs::MOUNT_ERROR_FILE_NAME.as_bytes());
        found.skip_mount |= bytes.eq_ignore_ascii_case(defs::SKIP_MOUNT_FILE_NAME.as_bytes());
        found.overlay |= bytes.eq_ignore_ascii_case(b"overlay");
        found.magic |= bytes.eq_ignore_ascii_case(b"magic");
    }

    found
}

pub fn has_mount_block_marker(module_path: &std::path::Path) -> bool {
    !mount_block_markers(module_path).is_empty()
}

#[cfg(all(test, not(feature = "control-plane")))]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;
    use crate::domain::DefaultMode;

    #[test]
    fn module_mount_mode_marker_detects_mode_files() {
        let temp = TempDir::new().unwrap();
        let module_path = temp.path().join("module");
        fs::create_dir_all(&module_path).unwrap();

        assert_eq!(module_mount_mode_marker(&module_path), None);

        fs::write(module_path.join("MAGIC"), b"").unwrap();
        assert_eq!(
            module_mount_mode_marker(&module_path),
            Some(MountMode::Magic)
        );
    }

    #[test]
    fn module_mount_mode_marker_prefers_overlay_when_multiple_markers_exist() {
        let temp = TempDir::new().unwrap();
        let module_path = temp.path().join("module");
        fs::create_dir_all(&module_path).unwrap();
        fs::write(module_path.join("OVERLAY"), b"").unwrap();
        fs::write(module_path.join("MAGIC"), b"").unwrap();

        assert_eq!(
            module_mount_mode_marker(&module_path),
            Some(MountMode::Overlay)
        );
    }

    #[test]
    fn module_mount_mode_marker_ignores_kasumi_for_nano() {
        let temp = TempDir::new().unwrap();
        let module_path = temp.path().join("module");
        fs::create_dir_all(&module_path).unwrap();
        fs::write(module_path.join("KASUMI"), b"").unwrap();

        assert_eq!(module_mount_mode_marker(&module_path), None);
    }

    #[test]
    fn load_module_rules_uses_mode_marker_for_nano_default() {
        let temp = TempDir::new().unwrap();
        let module_path = temp.path().join("module");
        fs::create_dir_all(&module_path).unwrap();
        fs::write(module_path.join("MaGiC"), b"").unwrap();

        let mut config = Config {
            moduledir: temp.path().to_path_buf(),
            default_mode: DefaultMode::Overlay,
            ..Config::default()
        };
        config.rules.insert(
            "module".to_string(),
            ModuleRules {
                default_mode: MountMode::Overlay,
                ..Default::default()
            },
        );

        assert_eq!(
            load_module_rules(&config, "module").default_mode,
            MountMode::Magic
        );
    }
}

#[cfg(test)]
mod marker_case_tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;
    use crate::defs;

    #[test]
    fn mount_block_markers_detect_case_insensitive_files() {
        let temp = TempDir::new().unwrap();
        let module_path = temp.path().join("module");
        fs::create_dir_all(&module_path).unwrap();
        fs::write(module_path.join("DISABLE"), b"").unwrap();
        fs::write(module_path.join("ReMoVe"), b"").unwrap();
        fs::write(module_path.join("MOUNT_ERROR"), b"").unwrap();
        fs::write(module_path.join("skip_Mount"), b"").unwrap();

        let markers = mount_block_markers(&module_path);
        assert_eq!(
            markers,
            vec![
                defs::DISABLE_FILE_NAME,
                defs::REMOVE_FILE_NAME,
                defs::MOUNT_ERROR_FILE_NAME,
                defs::SKIP_MOUNT_FILE_NAME,
            ]
        );
        assert!(has_mount_block_marker(&module_path));
    }
}
