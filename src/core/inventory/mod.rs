// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

pub mod discovery;

use std::fs;

use anyhow::Result;
#[cfg(not(feature = "control-plane"))]
use anyhow::bail;
pub use discovery::*;

#[cfg(not(feature = "control-plane"))]
use crate::domain::MountMode;
use crate::{conf::config::Config, defs, domain::ModuleRules};

pub fn load_module_rules(config: &Config, module_id: &str) -> Result<ModuleRules> {
    let mut rules = ModuleRules {
        default_mode: config.default_mode.as_mount_mode(),
        ..Default::default()
    };

    if let Some(global_rules) = config.rules.get(module_id) {
        rules.default_mode = global_rules.default_mode;
        rules.paths.extend(global_rules.paths.clone());
    }

    #[cfg(not(feature = "control-plane"))]
    if let Some(marker_mode) = module_mount_mode_marker(&config.moduledir.join(module_id))? {
        rules.default_mode = marker_mode;
    }

    Ok(rules)
}

#[cfg(not(feature = "control-plane"))]
pub fn module_mount_mode_marker(module_path: &std::path::Path) -> Result<Option<MountMode>> {
    let markers = scan_known_markers(module_path)?;
    if markers.overlay && markers.magic {
        bail!(
            "module contains conflicting overlay and magic markers: {}",
            module_path.display()
        );
    }
    if markers.overlay {
        Ok(Some(MountMode::Overlay))
    } else if markers.magic {
        Ok(Some(MountMode::Magic))
    } else {
        Ok(None)
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

pub fn mount_block_markers(module_path: &std::path::Path) -> Result<Vec<&'static str>> {
    let found = scan_known_markers(module_path)?;
    let mut markers = Vec::new();
    if found.disable {
        markers.push(defs::DISABLE_FILE_NAME);
    }
    if found.remove {
        markers.push(defs::REMOVE_FILE_NAME);
    }
    if found.skip_mount {
        markers.push(defs::SKIP_MOUNT_FILE_NAME);
    }
    Ok(markers)
}

#[derive(Default)]
struct KnownMarkers {
    disable: bool,
    remove: bool,
    skip_mount: bool,
    overlay: bool,
    magic: bool,
}

fn scan_known_markers(module_path: &std::path::Path) -> Result<KnownMarkers> {
    let mut found = KnownMarkers::default();
    let entries = fs::read_dir(module_path)?;

    for entry in entries {
        let entry = entry?;
        let name = entry.file_name();
        found.disable |= name == defs::DISABLE_FILE_NAME;
        found.remove |= name == defs::REMOVE_FILE_NAME;
        found.skip_mount |= name == defs::SKIP_MOUNT_FILE_NAME;
        found.overlay |= name == "overlay";
        found.magic |= name == "magic";
    }

    Ok(found)
}

pub fn has_mount_block_marker(module_path: &std::path::Path) -> Result<bool> {
    Ok(!mount_block_markers(module_path)?.is_empty())
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

        assert_eq!(module_mount_mode_marker(&module_path).unwrap(), None);

        fs::write(module_path.join("magic"), b"").unwrap();
        assert_eq!(
            module_mount_mode_marker(&module_path).unwrap(),
            Some(MountMode::Magic)
        );
    }

    #[test]
    fn module_mount_mode_marker_rejects_conflicting_markers() {
        let temp = TempDir::new().unwrap();
        let module_path = temp.path().join("module");
        fs::create_dir_all(&module_path).unwrap();
        fs::write(module_path.join("overlay"), b"").unwrap();
        fs::write(module_path.join("magic"), b"").unwrap();

        assert!(module_mount_mode_marker(&module_path).is_err());
    }

    #[test]
    fn load_module_rules_uses_mode_marker_for_nano_default() {
        let temp = TempDir::new().unwrap();
        let module_path = temp.path().join("module");
        fs::create_dir_all(&module_path).unwrap();
        fs::write(module_path.join("magic"), b"").unwrap();

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
            load_module_rules(&config, "module").unwrap().default_mode,
            MountMode::Magic
        );
    }
}
