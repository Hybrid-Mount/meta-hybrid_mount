// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::{fs, path::PathBuf};

use crate::{
    conf::schema::Config,
    core::{
        api::modules::{
            ModuleApplyEntry,
            apply::apply_modules_payload,
            payload::{build_runtime_modules_payload, build_scanned_modules_payload},
        },
        runtime_state::RuntimeState,
    },
    defs,
    domain::{DefaultMode, ModuleRules, MountMode},
};

#[test]
fn runtime_modules_payload_keeps_runtime_rules_and_metadata() {
    let mut config = Config {
        moduledir: PathBuf::from("/modules"),
        default_mode: DefaultMode::Magic,
        ..Default::default()
    };
    config.rules.insert(
        "alpha".to_string(),
        ModuleRules {
            default_mode: MountMode::Overlay,
            ..Default::default()
        },
    );

    let mut state = RuntimeState::default();
    state.overlay_modules = vec!["alpha".to_string()];

    let modules = build_runtime_modules_payload(&config, &state);
    assert_eq!(modules.len(), 1);

    let module = &modules[0];
    assert_eq!(module.id, "alpha");
    assert_eq!(module.mode, MountMode::Overlay);
    assert!(module.is_mounted);
    assert!(module.enabled);
    assert_eq!(module.source_path, PathBuf::from("/modules/alpha"));
    assert_eq!(module.rules.default_mode, MountMode::Overlay);
    assert_eq!(module.name, "alpha");
    assert_eq!(module.version, "unknown");
    assert_eq!(module.author, "unknown");
    assert_eq!(module.description, "No description");
}

#[test]
fn scanned_modules_payload_includes_module_prop_metadata() {
    let temp = tempfile::tempdir().unwrap();
    let module_dir = temp.path().join("alpha");
    fs::create_dir_all(&module_dir).unwrap();
    fs::write(
        module_dir.join("module.prop"),
        "id=alpha\nname=Alpha Module\nversion=1.2.3\nauthor=Alice\ndescription=Alpha description\n",
    )
    .unwrap();

    let config = Config {
        moduledir: temp.path().to_path_buf(),
        default_mode: DefaultMode::Overlay,
        ..Default::default()
    };
    let state = RuntimeState::default();

    let modules = build_scanned_modules_payload(&config, &state, temp.path()).unwrap();
    assert_eq!(modules.len(), 1);

    let module = &modules[0];
    assert_eq!(module.id, "alpha");
    assert_eq!(module.name, "Alpha Module");
    assert_eq!(module.version, "1.2.3");
    assert_eq!(module.author, "Alice");
    assert_eq!(module.description, "Alpha description");
}

#[test]
fn runtime_modules_payload_includes_mount_error_marker_modules() {
    let temp = tempfile::tempdir().unwrap();
    let module_dir = temp.path().join("broken");
    fs::create_dir_all(&module_dir).unwrap();
    fs::write(module_dir.join("MOUNT_ERROR"), b"").unwrap();

    let config = Config {
        moduledir: temp.path().to_path_buf(),
        default_mode: DefaultMode::Overlay,
        ..Default::default()
    };
    let state = RuntimeState::default();

    let modules = build_runtime_modules_payload(&config, &state);
    assert_eq!(modules.len(), 1);

    let module = &modules[0];
    assert_eq!(module.id, "broken");
    assert!(!module.is_mounted);
    assert!(!module.enabled);
    assert_eq!(
        module.mount_error.as_deref(),
        Some("mount_error marker present")
    );
}

#[test]
fn apply_modules_payload_handles_case_insensitive_disable_marker() {
    let temp = tempfile::tempdir().unwrap();
    let config_path = temp.path().join("config.toml");
    let module_dir = temp.path().join("modules").join("broken");
    fs::create_dir_all(&module_dir).unwrap();
    fs::write(module_dir.join("DISABLE"), b"").unwrap();

    let config = Config {
        moduledir: temp.path().join("modules"),
        ..Default::default()
    };
    config.save_to_file(&config_path).unwrap();

    let payload = apply_modules_payload(
        &config_path,
        &[ModuleApplyEntry {
            id: "broken".to_string(),
            enabled: Some(false),
            source_path: Some(module_dir.clone()),
            rules: ModuleRules::default(),
        }],
    )
    .unwrap();

    assert_eq!(payload.updated, 1);
    assert!(module_dir.join(defs::DISABLE_FILE_NAME).exists());
    assert!(crate::utils::dir_contains_entry_case_insensitive(
        &module_dir,
        defs::DISABLE_FILE_NAME
    ));
}

#[test]
fn apply_modules_payload_rules_only_preserves_disable_marker() {
    let temp = tempfile::tempdir().unwrap();
    let config_path = temp.path().join("config.toml");
    let module_dir = temp.path().join("modules").join("alpha");
    fs::create_dir_all(&module_dir).unwrap();
    fs::write(module_dir.join(defs::DISABLE_FILE_NAME), b"").unwrap();

    let config = Config {
        moduledir: temp.path().join("modules"),
        ..Default::default()
    };
    config.save_to_file(&config_path).unwrap();

    let payload = apply_modules_payload(
        &config_path,
        &[ModuleApplyEntry {
            id: "alpha".to_string(),
            enabled: None,
            source_path: None,
            rules: ModuleRules {
                default_mode: MountMode::Magic,
                ..Default::default()
            },
        }],
    )
    .unwrap();

    let saved = Config::load_optional_from_file(&config_path).unwrap();
    assert_eq!(payload.updated, 1);
    assert_eq!(saved.rules["alpha"].default_mode, MountMode::Magic);
    assert!(module_dir.join(defs::DISABLE_FILE_NAME).exists());
}
