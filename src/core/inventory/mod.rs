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

pub mod discovery;
pub mod listing;

pub use discovery::*;

#[cfg(not(feature = "control-plane"))]
use crate::domain::MountMode;
use crate::{conf::config::Config, defs, domain::ModuleRules, utils};

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
    [MountMode::Overlay, MountMode::Magic]
        .into_iter()
        .find(|mode| utils::dir_contains_entry_case_insensitive(module_path, mode.as_strategy()))
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
    let mut markers = Vec::new();
    if utils::dir_contains_entry_case_insensitive(module_path, defs::DISABLE_FILE_NAME) {
        markers.push(defs::DISABLE_FILE_NAME);
    }
    if utils::dir_contains_entry_case_insensitive(module_path, defs::REMOVE_FILE_NAME) {
        markers.push(defs::REMOVE_FILE_NAME);
    }
    if utils::dir_contains_entry_case_insensitive(module_path, defs::MOUNT_ERROR_FILE_NAME) {
        markers.push(defs::MOUNT_ERROR_FILE_NAME);
    }
    if utils::dir_contains_entry_case_insensitive(module_path, defs::SKIP_MOUNT_FILE_NAME) {
        markers.push(defs::SKIP_MOUNT_FILE_NAME);
    }
    markers
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
