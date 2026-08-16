// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::{fs, path::Path};

use tempfile::TempDir;

use super::{coordinator::prepare_mount_plan_with_root, types::SHALLOW_OVERLAY_DIR};
use crate::{
    conf::config,
    core::{inventory::Module, ops::plan::MountPlan},
    domain::{ModuleRules, MountMode},
};

fn write_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

fn make_module(
    id: &str,
    source_path: &Path,
    default_mode: MountMode,
    rules: &[(&str, MountMode)],
) -> Module {
    Module {
        id: id.to_string(),
        source_path: source_path.to_path_buf(),
        rules: ModuleRules {
            default_mode,
            paths: rules
                .iter()
                .map(|(path, mode)| ((*path).to_string(), *mode))
                .collect(),
        },
    }
}

fn test_config() -> config::Config {
    config::Config {
        mountsource: "test".to_string(),
        ..config::Config::default()
    }
}

fn prepare_with_root(
    _config: &config::Config,
    modules: &[Module],
    target_base: &Path,
    system_root: &Path,
) -> MountPlan {
    prepare_mount_plan_with_root(
        modules,
        target_base,
        system_root,
        vec!["system".to_string()],
    )
    .unwrap()
}

#[test]
fn prepare_mount_plan_builds_overlay_op_from_prepared_storage() {
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("module");
    write_file(&source.join("system/bin/sh"), "shell");

    let system_root = temp.path().join("sysroot");
    fs::create_dir_all(system_root.join("system/bin")).unwrap();

    let storage = temp.path().join("storage");
    let module = make_module("foo", &source, MountMode::Overlay, &[]);

    let plan = prepare_with_root(&test_config(), &[module], &storage, &system_root);

    assert_eq!(plan.overlay_ops.len(), 1);
    assert_eq!(plan.magic_module_ids, Vec::<String>::new());
    assert!(storage.join("foo/system/bin/sh").exists());
    assert_eq!(
        plan.overlay_ops[0].target,
        system_root.join("system/bin").display().to_string()
    );
    assert_eq!(
        plan.overlay_ops[0].lowerdirs,
        vec![storage.join("foo/system/bin")]
    );
}

#[test]
fn prepare_mount_plan_marks_magic_modules_without_overlay() {
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("module");
    write_file(&source.join("system/bin/sh"), "shell");

    let system_root = temp.path().join("sysroot");
    fs::create_dir_all(system_root.join("system/bin")).unwrap();

    let storage = temp.path().join("storage");
    let module = make_module(
        "foo",
        &source,
        MountMode::Overlay,
        &[("system/bin", MountMode::Magic)],
    );

    let plan = prepare_with_root(&test_config(), &[module], &storage, &system_root);

    assert!(plan.overlay_ops.is_empty());
    assert_eq!(plan.magic_module_ids, vec!["foo".to_string()]);
    assert!(storage.join("foo/system/bin/sh").exists());
}

#[test]
fn prepare_mount_plan_drops_modules_without_plan_results() {
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("module");
    write_file(&source.join("system/bin/sh"), "shell");

    let system_root = temp.path().join("sysroot");
    fs::create_dir_all(system_root.join("system/bin")).unwrap();

    let storage = temp.path().join("storage");
    let module = make_module("foo", &source, MountMode::Ignore, &[]);

    let plan = prepare_with_root(&test_config(), &[module], &storage, &system_root);

    assert!(plan.overlay_ops.is_empty());
    assert!(plan.magic_module_ids.is_empty());
    assert!(!storage.join("foo").exists());
}

#[test]
fn prepare_mount_plan_preserves_overlay_direct_files_when_subtree_is_split() {
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("module");
    write_file(&source.join("module.prop"), "id=foo");
    write_file(&source.join("system/etc/permissions.xml"), "permissions");
    write_file(&source.join("system/etc/init/ignored.rc"), "ignored");

    let system_root = temp.path().join("sysroot");
    fs::create_dir_all(system_root.join("system/etc/init")).unwrap();

    let storage = temp.path().join("storage");
    let module = make_module(
        "foo",
        &source,
        MountMode::Overlay,
        &[("system/etc/init", MountMode::Ignore)],
    );

    let plan = prepare_with_root(&test_config(), &[module], &storage, &system_root);

    let shallow_etc = storage
        .join("foo")
        .join(SHALLOW_OVERLAY_DIR)
        .join("system/etc");
    assert_eq!(plan.overlay_ops.len(), 1);
    assert_eq!(plan.overlay_module_ids, vec!["foo".to_string()]);
    assert!(plan.magic_module_ids.is_empty());
    assert_eq!(
        plan.overlay_ops[0].target,
        system_root.join("system/etc").display().to_string()
    );
    assert_eq!(plan.overlay_ops[0].lowerdirs, vec![shallow_etc.clone()]);
    assert!(storage.join("foo/system/etc/permissions.xml").exists());
    assert!(shallow_etc.join("permissions.xml").exists());
    assert!(!shallow_etc.join("init").exists());
}

#[test]
fn prepare_mount_plan_preserves_overlay_replace_marker_when_subtree_is_split() {
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("module");
    write_file(&source.join("module.prop"), "id=foo");
    write_file(&source.join("system/etc/.REPLACE"), "");
    write_file(&source.join("system/etc/init/ignored.rc"), "ignored");

    let system_root = temp.path().join("sysroot");
    fs::create_dir_all(system_root.join("system/etc/init")).unwrap();

    let storage = temp.path().join("storage");
    let module = make_module(
        "foo",
        &source,
        MountMode::Overlay,
        &[("system/etc/init", MountMode::Ignore)],
    );

    let plan = prepare_with_root(&test_config(), &[module], &storage, &system_root);

    let shallow_etc = storage
        .join("foo")
        .join(SHALLOW_OVERLAY_DIR)
        .join("system/etc");
    assert_eq!(plan.overlay_ops.len(), 1);
    assert_eq!(
        plan.overlay_ops[0].target,
        system_root.join("system/etc").display().to_string()
    );
    assert_eq!(plan.overlay_ops[0].lowerdirs, vec![shallow_etc.clone()]);
    assert!(shallow_etc.is_dir());
    assert!(!shallow_etc.join(".REPLACE").exists());
    assert!(!shallow_etc.join("init").exists());
}

#[test]
fn prepare_mount_plan_skips_replace_marker_entries() {
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("module");
    fs::create_dir_all(source.join("system")).unwrap();
    write_file(&source.join("system/.REPLACE"), "");
    write_file(&source.join("system/bin/sh"), "shell");

    let system_root = temp.path().join("sysroot");
    fs::create_dir_all(system_root.join("system/bin")).unwrap();

    let storage = temp.path().join("storage");
    let module = make_module("foo", &source, MountMode::Overlay, &[]);

    let plan = prepare_with_root(&test_config(), &[module], &storage, &system_root);

    assert!(!plan.overlay_ops.is_empty());
    assert!(!storage.join("foo/system/.REPLACE").exists());
    assert!(storage.join("foo/system/bin/sh").exists());
}

#[test]
fn prepare_mount_plan_keeps_replace_only_overlay_dir() {
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("module");
    write_file(&source.join("system/app/.RePlAcE"), "");

    let system_root = temp.path().join("sysroot");
    fs::create_dir_all(system_root.join("system/app")).unwrap();

    let storage = temp.path().join("storage");
    let module = make_module("foo", &source, MountMode::Overlay, &[]);

    let plan = prepare_with_root(&test_config(), &[module], &storage, &system_root);

    assert_eq!(plan.overlay_ops.len(), 1);
    assert_eq!(
        plan.overlay_ops[0].target,
        system_root.join("system/app").display().to_string()
    );
    assert!(storage.join("foo/system/app").is_dir());
    assert!(!storage.join("foo/system/app/.RePlAcE").exists());
}

#[test]
fn prepare_mount_plan_marks_magic_for_replace_only_dir() {
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("module");
    write_file(&source.join("system/.rEpLaCe"), "");

    let system_root = temp.path().join("sysroot");
    fs::create_dir_all(system_root.join("system")).unwrap();

    let storage = temp.path().join("storage");
    let module = make_module("foo", &source, MountMode::Magic, &[]);

    let plan = prepare_with_root(&test_config(), &[module], &storage, &system_root);

    assert!(plan.overlay_ops.is_empty());
    assert_eq!(plan.magic_module_ids, vec!["foo".to_string()]);
    assert!(storage.join("foo/system").is_dir());
    assert!(!storage.join("foo/system/.rEpLaCe").exists());
}
