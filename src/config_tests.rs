// SPDX-License-Identifier: GPL-3.0-only

use super::*;

fn module_id(id: &str) -> ModuleId {
    ModuleId::try_from(id).unwrap()
}

#[test]
fn defaults_match_contract() {
    let config = Config::default();

    assert_eq!(config.moduledir, PathBuf::from("/data/adb/modules"));
    assert_eq!(config.mountsource, "KSU");
    assert_eq!(config.overlay_mode, OverlayMode::Ext4);
    assert!(!config.disable_umount);
    assert_eq!(config.default_mode, Mode::Overlay);
    assert!(config.rules.is_empty());
}

#[test]
fn parses_empty_toml_as_defaults() {
    let config = Config::from_toml("").unwrap();
    assert_eq!(config, Config::default());
}

#[test]
fn default_config_toml_snapshot_is_stable() {
    let expected = r#"moduledir = "/data/adb/modules"
mountsource = "KSU"
overlay_mode = "ext4"
disable_umount = false
default_mode = "overlay"

[rules]
"#;

    assert_eq!(Config::default().to_toml().unwrap(), expected);
}

#[test]
fn global_ignore_is_rejected_without_changing_rule_ignore_semantics() {
    let err = Config::from_toml(
        r#"
default_mode = "ignore"
"#,
    )
    .unwrap_err();

    assert!(matches!(err, Error::UnsupportedGlobalDefaultMode), "{err}");

    // Per-module/per-path ignore stays legal.
    let config = Config::from_toml(
        r#"
default_mode = "overlay"

[rules.demo]
default_mode = "ignore"

[rules.demo.paths]
"system/etc/hosts" = "ignore"
"#,
    )
    .unwrap();

    assert_eq!(config.default_mode, Mode::Overlay);
    assert_eq!(config.rules["demo"].default_mode, Some(Mode::Ignore));
    assert_eq!(config.rules["demo"].paths["system/etc/hosts"], Mode::Ignore);
}

#[test]
fn parses_planned_example() {
    let text = r#"
moduledir = "/data/adb/modules"
mountsource = "KSU"
overlay_mode = "ext4"
disable_umount = false
default_mode = "overlay"

[rules."hosts_redirect"]
default_mode = "magic"

[rules."hosts_redirect".paths]
"system/etc/hosts" = "overlay"
"#;

    let config = Config::from_toml(text).unwrap();

    let rule = config.rules.get("hosts_redirect").unwrap();
    assert_eq!(rule.default_mode, Some(Mode::Magic));
    assert_eq!(rule.paths.get("system/etc/hosts"), Some(&Mode::Overlay));
}

#[test]
fn accepts_and_drops_obsolete_empty_custom_mounts_during_upgrade() {
    let config = Config::from_toml(
        r#"
moduledir = "/data/adb/modules"
default_mode = "magic"
custom_mounts = []
"#,
    )
    .unwrap();

    assert_eq!(config.default_mode, Mode::Magic);
    assert!(config.legacy_custom_mounts.is_empty());
    assert!(!config.to_toml().unwrap().contains("custom_mounts"));
}

#[test]
fn invalid_module_id_rule_key_is_rejected_with_context() {
    let err = Config::from_toml(
        r#"
default_mode = "overlay"

[rules."1bad"]
default_mode = "magic"
"#,
    )
    .unwrap_err();

    let message = err.to_string();
    assert!(message.contains("Invalid module ID"), "{message}");
    assert!(message.contains("1bad"), "{message}");
}

#[test]
fn toml_roundtrip_preserves_rules() {
    let mut config = Config {
        default_mode: Mode::Magic,
        ..Config::default()
    };
    config.rules.insert(
        module_id("demo"),
        ModuleRule {
            default_mode: Some(Mode::Ignore),
            paths: BTreeMap::from([
                ("system/etc/hosts".to_owned(), Mode::Overlay),
                ("system/bin/app".to_owned(), Mode::Magic),
            ]),
        },
    );

    let text = config.to_toml().unwrap();
    let reparsed = Config::from_toml(&text).unwrap();

    assert_eq!(reparsed, config);
}

#[test]
fn rejects_invalid_mode() {
    let err = Config::from_toml(r#"default_mode = "transparent""#).unwrap_err();
    let message = err.to_string();

    assert!(message.contains("default_mode"), "{message}");
}

#[test]
fn rejects_unknown_top_level_field() {
    let err = Config::from_toml(
        r#"
default_mode = "overlay"
unknown_option = true
"#,
    )
    .unwrap_err();

    assert!(err.to_string().contains("unknown field"), "{}", err);
}

#[test]
fn rejects_unknown_rule_field() {
    let err = Config::from_toml(
        r#"
[rules.demo]
unknown_option = "overlay"
"#,
    )
    .unwrap_err();

    assert!(err.to_string().contains("unknown field"), "{}", err);
}

#[test]
fn json_uses_contract_shape() {
    let config = Config::default();

    let value: serde_json::Value =
        serde_json::from_str(&serde_json::to_string_pretty(&config).unwrap()).unwrap();

    assert_eq!(
        value,
        serde_json::json!({
            "moduledir": "/data/adb/modules",
            "mountsource": "KSU",
            "overlay_mode": "ext4",
            "disable_umount": false,
            "default_mode": "overlay",
            "rules": {}
        })
    );
}

#[test]
fn webui_json_exposes_tmpfs_capability_without_persisting_it() {
    let config = Config::default();
    let value: serde_json::Value =
        serde_json::from_str(&config.to_webui_json(false).unwrap()).unwrap();

    assert_eq!(value["tmpfs_xattr_supported"], false);
    assert!(!config.to_toml().unwrap().contains("tmpfs_xattr_supported"));
}

#[test]
fn webui_json_exposes_config_missing_but_toml_never_persists_it() {
    let config = Config {
        config_missing: true,
        ..Config::default()
    };

    let value: serde_json::Value =
        serde_json::from_str(&config.to_webui_json(false).unwrap()).unwrap();

    assert_eq!(value["config_missing"], true);
    assert!(!config.to_toml().unwrap().contains("config_missing"));
}

#[test]
fn save_creates_parent_and_load_roundtrips() {
    let dir = test_dir("save-load");
    let path = dir.join("nested").join("config.toml");

    let mut config = Config {
        disable_umount: true,
        ..Config::default()
    };
    config.rules.insert(
        module_id("demo"),
        ModuleRule {
            default_mode: Some(Mode::Magic),
            paths: BTreeMap::new(),
        },
    );

    config.save(&path).unwrap();
    let loaded = Config::load(&path).unwrap();

    assert_eq!(loaded, config);
    cleanup(&dir);
}

#[test]
fn write_default_resets_disk_content() {
    let dir = test_dir("write-default");
    let path = dir.join("config.toml");

    let config = Config {
        default_mode: Mode::Magic,
        ..Config::default()
    };
    config.save(&path).unwrap();

    let written = Config::write_default(&path).unwrap();

    assert_eq!(written, Config::default());
    assert_eq!(Config::load(&path).unwrap(), Config::default());
    cleanup(&dir);
}

#[test]
fn save_rejects_global_ignore_and_leaves_existing_file_untouched() {
    let dir = test_dir("save-reject-ignore");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    Config::default().save(&path).unwrap();
    let before = fs::read_to_string(&path).unwrap();

    let invalid = Config {
        default_mode: Mode::Ignore,
        ..Config::default()
    };
    let err = invalid.save(&path).unwrap_err();

    assert!(matches!(err, Error::UnsupportedGlobalDefaultMode), "{err}");
    assert_eq!(fs::read_to_string(&path).unwrap(), before);
    cleanup(&dir);
}

#[cfg(unix)]
#[test]
fn save_refuses_to_replace_symlinked_config() {
    use std::os::unix::fs::symlink;

    let dir = test_dir("save-symlink");
    fs::create_dir_all(&dir).unwrap();
    let target = dir.join("real.toml");
    Config::default().save(&target).unwrap();
    let before = fs::read_to_string(&target).unwrap();

    let link = dir.join("config.toml");
    symlink(&target, &link).unwrap();

    let err = Config::default().save(&link).unwrap_err();

    assert!(err.to_string().contains("symlinked config"), "{err}");
    assert_eq!(fs::read_to_string(&target).unwrap(), before);
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    cleanup(&dir);
}

#[test]
fn load_or_default_uses_defaults_and_marks_missing_file() {
    let dir = test_dir("load-or-default");
    let missing = dir.join("missing.toml");

    let config = Config::load_or_default(&missing).unwrap();

    assert!(config.config_missing);
    let expected = Config {
        config_missing: true,
        ..Config::default()
    };
    assert_eq!(config, expected);

    cleanup(&dir);
}

#[test]
fn load_or_default_rejects_corrupt_config_with_path_context() {
    let dir = test_dir("load-or-default-corrupt");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    fs::write(&path, "default_mode = not-valid").unwrap();

    let err = Config::load_or_default(&path).unwrap_err();

    let message = err.to_string();
    assert!(message.contains(&path.display().to_string()), "{message}");
    assert!(message.contains("parse config"), "{message}");
    cleanup(&dir);
}

#[test]
fn load_wraps_read_errors_with_path_context() {
    let dir = test_dir("load-read-error");
    fs::create_dir_all(&dir).unwrap();
    // 目录不是可读的 TOML 文件，`read_to_string` 在任何平台都会失败。
    let path = dir.join("unreadable.toml");
    fs::create_dir_all(&path).unwrap();

    let err = Config::load(&path).unwrap_err();
    let message = err.to_string();
    assert!(message.contains(&path.display().to_string()), "{message}");
    assert!(message.contains("read config"), "{message}");

    cleanup(&dir);
}

#[test]
fn load_reads_deduplicated_module_blacklist_without_persisting_it_in_config() {
    let dir = test_dir("module-blacklist");
    let path = dir.join("config.toml");
    Config::default().save(&path).unwrap();
    fs::write(
        dir.join(defs::MODULE_BLACKLIST_FILE_NAME),
        r#"blacklist = ["blocked", " blocked ", "other", ""]"#,
    )
    .unwrap();

    let loaded = Config::load(&path).unwrap();

    assert!(loaded.is_module_blacklisted("blocked"));
    assert!(loaded.is_module_blacklisted("other"));
    assert_eq!(loaded.module_blacklist.len(), 2);
    assert!(!loaded.to_toml().unwrap().contains("module_blacklist"));
    assert!(!loaded.to_toml().unwrap().contains("blocked"));
    cleanup(&dir);
}

#[test]
fn bundled_blacklist_contains_move_certificate() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("module")
        .join(defs::MODULE_BLACKLIST_FILE_NAME);

    let blacklist = read_module_blacklist(&path).unwrap();

    assert!(blacklist.contains(&module_id("MoveCertificate")));
}

#[test]
fn invalid_blacklist_module_id_fails_closed_with_path_context() {
    let dir = test_dir("module-blacklist-invalid-id");
    let path = dir.join("config.toml");
    Config::default().save(&path).unwrap();
    let blacklist_path = dir.join(defs::MODULE_BLACKLIST_FILE_NAME);
    fs::write(&blacklist_path, r#"blacklist = ["1bad"]"#).unwrap();

    let err = Config::load(&path).unwrap_err();
    let message = err.to_string();
    assert!(
        message.contains("Invalid module ID") || message.contains("invalid module id"),
        "{message}"
    );
    assert!(
        message.contains(&blacklist_path.display().to_string()),
        "{message}"
    );

    cleanup(&dir);
}

#[test]
fn missing_main_config_loads_blacklist_but_corrupt_blacklist_fails_closed() {
    let dir = test_dir("module-blacklist-fallback");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    let blacklist_path = dir.join(defs::MODULE_BLACKLIST_FILE_NAME);
    fs::write(&blacklist_path, r#"blacklist = ["blocked"]"#).unwrap();

    let loaded = Config::load_or_default(&path).unwrap();
    assert!(loaded.config_missing);
    assert!(loaded.is_module_blacklisted("blocked"));
    assert_eq!(loaded.default_mode, Mode::Overlay);

    fs::write(&blacklist_path, "blacklist = not-valid").unwrap();
    let err = Config::load_or_default(&path).unwrap_err();
    assert!(
        err.to_string().contains("module blacklist")
            && err
                .to_string()
                .contains(&blacklist_path.display().to_string()),
        "{err}"
    );

    // 配置存在时黑名单损坏同样 fail-closed。
    fs::write(&path, "default_mode = \"magic\"\n").unwrap();
    assert!(Config::load_or_default(&path).is_err());
    cleanup(&dir);
}

#[test]
fn patch_merges_while_preserving_untouched_fields() {
    let mut config = Config {
        default_mode: Mode::Magic,
        ..Config::default()
    };
    config.rules.insert(
        module_id("keep"),
        ModuleRule {
            default_mode: Some(Mode::Ignore),
            paths: BTreeMap::from([("system/etc/a".to_owned(), Mode::Overlay)]),
        },
    );

    let patch: ConfigPatch = serde_json::from_str(
        r#"{"disable_umount":true,"rules":{"new":{"default_mode":"overlay","paths":{"system/etc/hosts":"magic"}}}}"#,
    )
    .unwrap();
    config.apply_patch(patch).unwrap();

    assert!(config.disable_umount);
    assert_eq!(config.default_mode, Mode::Magic);
    assert_eq!(config.rules["keep"].default_mode, Some(Mode::Ignore));
    assert_eq!(config.rules["keep"].paths["system/etc/a"], Mode::Overlay);
    assert_eq!(config.rules["new"].default_mode, Some(Mode::Overlay));
    assert_eq!(config.rules["new"].paths["system/etc/hosts"], Mode::Magic);
}

#[test]
fn patch_rejects_ignore_as_global_default_without_partial_update() {
    let mut config = Config {
        default_mode: Mode::Magic,
        ..Config::default()
    };
    let patch: ConfigPatch =
        serde_json::from_str(r#"{"default_mode":"ignore","disable_umount":true}"#).unwrap();

    let err = config.apply_patch(patch).unwrap_err();

    assert!(matches!(err, Error::UnsupportedGlobalDefaultMode), "{err}");
    // 校验失败不能留下半个 patch。
    assert_eq!(config.default_mode, Mode::Magic);
    assert!(!config.disable_umount);
}

#[test]
fn patch_null_clears_module_default_mode() {
    let mut config = Config::default();
    config.rules.insert(
        module_id("m"),
        ModuleRule {
            default_mode: Some(Mode::Magic),
            paths: BTreeMap::new(),
        },
    );

    let patch: ConfigPatch =
        serde_json::from_str(r#"{"rules":{"m":{"default_mode":null}}}"#).unwrap();
    config.apply_patch(patch).unwrap();

    assert_eq!(config.rules["m"].default_mode, None);
}

#[test]
fn patch_can_replace_all_rules_for_full_editor_save() {
    let mut config = Config::default();
    config
        .rules
        .insert(module_id("stale"), ModuleRule::default());
    config
        .rules
        .insert(module_id("keep"), ModuleRule::default());

    let patch: ConfigPatch =
        serde_json::from_str(r#"{"replace_rules":true,"rules":{"keep":{"default_mode":"magic"}}}"#)
            .unwrap();
    config.apply_patch(patch).unwrap();

    assert!(!config.rules.contains_key("stale"));
    assert_eq!(config.rules.len(), 1);
    assert_eq!(config.rules["keep"].default_mode, Some(Mode::Magic));
}

#[test]
fn payload_hex_roundtrips_through_save() {
    let dir = test_dir("payload");
    let path = dir.join("config.toml");

    let json = r#"{"default_mode":"magic","disable_umount":true}"#;
    let payload_hex = hex::encode(json);
    save_config_payload(&path, &payload_hex).unwrap();

    let saved = Config::load(&path).unwrap();
    assert_eq!(saved.default_mode, Mode::Magic);
    assert!(saved.disable_umount);
    assert_eq!(saved.moduledir, PathBuf::from("/data/adb/modules"));

    cleanup(&dir);
}

#[test]
fn payload_arg_requires_marker_and_rejects_invalid_hex() {
    assert_eq!(
        parse_payload_arg(&["--payload".to_owned(), "7b7d".to_owned()]).unwrap(),
        "7b7d"
    );
    assert!(parse_payload_arg(&["x".to_owned()]).is_err());
    assert!(decode_payload_arg("zz").is_err());
    assert_eq!(decode_payload_arg("7b7d").unwrap(), "{}");
}

fn test_dir(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("hybrid-mount-{tag}-{}", std::process::id()))
}

fn cleanup(dir: &Path) {
    fs::remove_dir_all(dir).ok();
}
