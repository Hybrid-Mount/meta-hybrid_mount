// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::fs;

use crate::{
    conf::schema::Config,
    core::{
        api::modules::{
            ModuleApplyEntry, apply::apply_modules_payload, payload::build_scanned_modules_payload,
        },
        runtime_state::RuntimeState,
    },
    defs,
    domain::{DefaultMode, ModuleRules, MountMode},
};

#[test]
fn runtime_modules_payload_keeps_runtime_rules_and_metadata() {
    let temp = tempfile::tempdir().unwrap();
    let module_dir = temp.path().join("alpha");
    fs::create_dir_all(&module_dir).unwrap();
    fs::write(
        module_dir.join("module.prop"),
        "id=alpha\nname=Alpha\nversion=1.0\nauthor=Alice\ndescription=Test module\n",
    )
    .unwrap();
    let mut config = Config {
        moduledir: temp.path().to_path_buf(),
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

    let modules = build_scanned_modules_payload(&config, &state, &config.moduledir).unwrap();
    assert_eq!(modules.len(), 1);

    let module = &modules[0];
    assert_eq!(module.id, "alpha");
    assert_eq!(module.mode, MountMode::Overlay);
    assert!(module.is_mounted);
    assert!(module.enabled);
    assert_eq!(module.rules.default_mode, MountMode::Overlay);
    assert_eq!(module.name, "Alpha");
    assert_eq!(module.version, "1.0");
    assert_eq!(module.author, "Alice");
    assert_eq!(module.description, "Test module");
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
fn scanned_modules_payload_skips_directory_without_module_prop() {
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir_all(temp.path().join("incomplete")).unwrap();
    let valid_dir = temp.path().join("valid");
    fs::create_dir_all(&valid_dir).unwrap();
    fs::write(
        valid_dir.join("module.prop"),
        "id=valid\nname=Valid\nversion=1.0\nauthor=Alice\ndescription=Valid module\n",
    )
    .unwrap();

    let config = Config {
        moduledir: temp.path().to_path_buf(),
        ..Default::default()
    };

    let modules =
        build_scanned_modules_payload(&config, &RuntimeState::default(), temp.path()).unwrap();

    assert_eq!(modules.len(), 1);
    assert_eq!(modules[0].id, "valid");
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
            rules: ModuleRules {
                default_mode: MountMode::Magic,
                ..Default::default()
            },
        }],
    )
    .unwrap();

    let saved = Config::load_from_file(&config_path).unwrap();
    assert_eq!(payload.updated, 1);
    assert_eq!(saved.rules["alpha"].default_mode, MountMode::Magic);
    assert!(module_dir.join(defs::DISABLE_FILE_NAME).exists());
}

#[test]
fn apply_modules_payload_rejects_rules_for_a_missing_module() {
    let temp = tempfile::tempdir().unwrap();
    let config_path = temp.path().join("config.toml");
    let modules_dir = temp.path().join("modules");

    let config = Config {
        moduledir: modules_dir.clone(),
        ..Default::default()
    };
    config.save_to_file(&config_path).unwrap();

    let result = apply_modules_payload(
        &config_path,
        &[ModuleApplyEntry {
            id: "missing".to_string(),
            enabled: None,
            rules: ModuleRules {
                default_mode: MountMode::Magic,
                ..Default::default()
            },
        }],
    );

    let saved = Config::load_from_file(&config_path).unwrap();
    assert!(result.is_err());
    assert!(!saved.rules.contains_key("missing"));
}

#[test]
fn apply_modules_payload_validates_entire_batch_before_side_effects() {
    let temp = tempfile::tempdir().unwrap();
    let config_path = temp.path().join("config.toml");
    let modules_dir = temp.path().join("modules");
    let first_dir = modules_dir.join("first");
    fs::create_dir_all(&first_dir).unwrap();
    fs::create_dir_all(&modules_dir).unwrap();
    fs::write(modules_dir.join("invalid"), b"not a directory").unwrap();

    let config = Config {
        moduledir: modules_dir.clone(),
        ..Default::default()
    };
    config.save_to_file(&config_path).unwrap();

    let result = apply_modules_payload(
        &config_path,
        &[
            ModuleApplyEntry {
                id: "first".to_string(),
                enabled: Some(false),
                rules: ModuleRules::default(),
            },
            ModuleApplyEntry {
                id: "invalid".to_string(),
                enabled: Some(false),
                rules: ModuleRules::default(),
            },
        ],
    );

    assert!(result.is_err());
    assert!(!first_dir.join(defs::DISABLE_FILE_NAME).exists());
    let saved = Config::load_from_file(&config_path).unwrap();
    assert!(saved.rules.is_empty());
}

#[test]
fn apply_modules_payload_rejects_non_file_disable_marker_before_side_effects() {
    let temp = tempfile::tempdir().unwrap();
    let config_path = temp.path().join("config.toml");
    let module_dir = temp.path().join("modules").join("alpha");
    fs::create_dir_all(module_dir.join(defs::DISABLE_FILE_NAME)).unwrap();

    let config = Config {
        moduledir: temp.path().join("modules"),
        ..Default::default()
    };
    config.save_to_file(&config_path).unwrap();

    let result = apply_modules_payload(
        &config_path,
        &[ModuleApplyEntry {
            id: "alpha".to_string(),
            enabled: Some(true),
            rules: ModuleRules {
                default_mode: MountMode::Magic,
                ..Default::default()
            },
        }],
    );

    let saved = Config::load_from_file(&config_path).unwrap();
    assert!(result.is_err());
    assert!(saved.rules.is_empty());
    assert!(module_dir.join(defs::DISABLE_FILE_NAME).is_dir());
}
