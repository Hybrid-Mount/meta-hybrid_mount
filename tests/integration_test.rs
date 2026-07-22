// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::path::PathBuf;

use hybrid_mount::{
    conf::schema::Config,
    core::backend_capabilities::BackendCapabilities,
    domain::{DefaultMode, ModuleRules, MountMode},
};

fn make_rules(default_mode: MountMode, paths: &[(&str, MountMode)]) -> ModuleRules {
    ModuleRules {
        default_mode,
        paths: paths.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
    }
}

#[test]
fn module_rules_longest_prefix_match() {
    let rules = make_rules(
        MountMode::Overlay,
        &[
            ("system", MountMode::Magic),
            ("system/app", MountMode::Kasumi),
            ("system/app/private", MountMode::Overlay),
        ],
    );

    assert_eq!(rules.get_mode("vendor"), MountMode::Overlay);
    assert_eq!(rules.get_mode("system"), MountMode::Magic);
    assert_eq!(rules.get_mode("system/app"), MountMode::Kasumi);
    assert_eq!(rules.get_mode("system/app/private"), MountMode::Overlay);
    assert_eq!(rules.get_mode("system/app/private/lib"), MountMode::Overlay);
}

#[test]
fn module_rules_path_component_boundary() {
    // "sys" should not match "system" as a prefix (not a full path component)
    let rules = make_rules(MountMode::Overlay, &[("sys", MountMode::Magic)]);

    assert_eq!(rules.get_mode("sys"), MountMode::Magic);
    assert_eq!(rules.get_mode("sys/app"), MountMode::Magic);
    assert_eq!(rules.get_mode("system"), MountMode::Overlay);
    assert_eq!(rules.get_mode("syscall"), MountMode::Overlay);
}

#[test]
fn default_mode_and_config_integration() {
    let config = Config {
        moduledir: PathBuf::from("/data/adb/modules"),
        default_mode: DefaultMode::Overlay,
        ..Config::default()
    };

    assert_eq!(config.moduledir, PathBuf::from("/data/adb/modules"));
    assert_eq!(config.default_mode, DefaultMode::Overlay);
    assert!(!config.disable_umount);
    assert!(matches!(config.mountsource.as_str(), "APatch" | "KSU"));
}

#[test]
fn backend_capabilities_detect_does_not_panic() {
    let config = Config::default();
    match BackendCapabilities::detect(&config) {
        Ok(capabilities) => {
            let _ = capabilities.can_use_kasumi();
            assert!(!capabilities.kasumi_status().is_empty());
        }
        Err(error) => assert!(!error.to_string().is_empty()),
    }
}
