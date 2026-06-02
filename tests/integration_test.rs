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
    assert_eq!(config.mountsource, "APatch");
}

#[test]
fn backend_capabilities_detect_does_not_panic() {
    let config = Config::default();
    let capabilities = BackendCapabilities::detect(&config);

    // Kasumi availability varies by platform — just verify detect() doesn't panic
    let _ = capabilities.can_use_kasumi();
    let _ = capabilities.kasumi_status();
}
