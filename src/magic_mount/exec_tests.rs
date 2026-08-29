// SPDX-License-Identifier: GPL-3.0-only

use super::*;
use crate::module_id::ModuleId;
use crate::mount_tree::{MountSource, MountTree};

#[test]
fn injected_magic_mount_failure_fires_before_side_effects() {
    let _fault_guard = crate::sys::faults::test_lock();
    let root =
        std::env::temp_dir().join(format!("hybrid-mount-magic-fault-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let source = root.join("source");
    fs::write(&source, "data").unwrap();

    let mut tree = MountTree::default();
    tree.insert(
        "hosts",
        MountSource {
            module_id: ModuleId::try_from("m").unwrap(),
            relative: "hosts".to_owned(),
            source_path: source,
            file_type: NodeFileType::RegularFile,
            replace: false,
            backend: MountMode::Magic,
        },
    );
    let node = tree.root.children.get("hosts").unwrap();
    let mut stats = MagicMountStats::default();
    let mut calls = 0;
    let mut on_mount = |_: &str| calls += 1;

    crate::sys::faults::enable_next_magic_mount_failure();
    let mut mount = MagicMount::new(node, &root, &root, false, false, &mut stats, &mut on_mount);
    let err = mount.do_mount().unwrap_err();
    crate::sys::faults::reset();

    assert!(err.to_string().contains("injected magic mount"), "{err}");
    assert_eq!(calls, 0);
    assert_eq!(stats.mounted_files, 0);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn direct_child_failure_aborts_directory_execution() {
    let root = std::env::temp_dir().join(format!(
        "hybrid-mount-magic-direct-child-failure-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let parent_path = root.join("parent");
    fs::create_dir_all(&parent_path).unwrap();
    fs::write(parent_path.join("child"), b"target").unwrap();

    let mut tree = MountTree::default();
    tree.insert(
        "parent",
        MountSource {
            module_id: ModuleId::try_from("m").unwrap(),
            relative: "parent".to_owned(),
            source_path: parent_path.clone(),
            file_type: NodeFileType::Directory,
            replace: false,
            backend: MountMode::Magic,
        },
    );
    tree.insert(
        "parent/child",
        MountSource {
            module_id: ModuleId::try_from("m").unwrap(),
            relative: "parent/child".to_owned(),
            source_path: root.join("missing-child"),
            file_type: NodeFileType::RegularFile,
            replace: false,
            backend: MountMode::Magic,
        },
    );

    let node = tree.root.children.get("parent").unwrap();
    let work_dir = root.join("work");
    let mut stats = MagicMountStats::default();
    let mut mounted = Vec::new();
    let mut on_mount = |target: &str| mounted.push(target.to_owned());
    let mut mount = MagicMount::new(
        node,
        &root,
        &work_dir,
        false,
        false,
        &mut stats,
        &mut on_mount,
    );

    let error = mount.do_mount().unwrap_err();

    assert!(error.to_string().contains("mount module file"), "{error}");
    assert!(mounted.is_empty());
    assert_eq!(stats.mounted_files, 0);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn operation_results_only_mark_real_mounts_as_active() {
    let target = Path::new("/system/etc/hosts");
    let cases = [
        (MagicOperation::Bind, true),
        (MagicOperation::Move, true),
        (MagicOperation::Replace, true),
        (MagicOperation::Symlink, false),
        (MagicOperation::Whiteout, false),
        (MagicOperation::Noop, false),
    ];

    for (operation, is_mount_target) in cases {
        let result = MagicMountResult::new(operation, target);
        assert_eq!(result.is_mount_target(), is_mount_target);
        assert_eq!(result.target, target);
    }
}

#[test]
fn whiteout_returns_a_non_mount_result() {
    let node = MountNode {
        name: "deleted".to_owned(),
        children: std::collections::BTreeMap::new(),
        sources: vec![MountSource {
            module_id: ModuleId::try_from("m").unwrap(),
            relative: "deleted".to_owned(),
            source_path: PathBuf::from("/modules/m/deleted"),
            file_type: NodeFileType::Whiteout,
            replace: false,
            backend: MountMode::Magic,
        }],
        structural_sources: Vec::new(),
    };
    let mut stats = MagicMountStats::default();
    let mut mounted = Vec::new();
    let mut on_mount = |target: &str| mounted.push(target.to_owned());
    let mut mount = MagicMount::new(
        &node,
        Path::new("/system"),
        Path::new("/tmp/magic-staging"),
        false,
        false,
        &mut stats,
        &mut on_mount,
    );

    let result = mount.do_mount().unwrap();

    assert_eq!(result.operation, MagicOperation::Whiteout);
    assert!(!result.is_mount_target());
    assert!(mounted.is_empty());
    assert!(stats.active_mounts.is_empty());
}

#[test]
fn fake_bind_success_returns_and_registers_real_target() {
    let root = std::env::temp_dir().join(format!(
        "hybrid-mount-magic-fake-bind-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let target = root.join("hosts");
    let source = root.join("module-hosts");
    fs::create_dir_all(&root).unwrap();
    fs::write(&target, b"stock").unwrap();
    fs::write(&source, b"module").unwrap();

    let mut tree = MountTree::default();
    tree.insert(
        "hosts",
        MountSource {
            module_id: ModuleId::try_from("m").unwrap(),
            relative: "hosts".to_owned(),
            source_path: source,
            file_type: NodeFileType::RegularFile,
            replace: false,
            backend: MountMode::Magic,
        },
    );
    let node = tree.root.children.get("hosts").unwrap();
    let mut stats = MagicMountStats::default();
    let mut mounted = Vec::new();
    let mut on_mount = |path: &str| mounted.push(path.to_owned());
    let _fake_ops = crate::sys::faults::fake_magic_mount_ops();
    let mut mount = MagicMount::new(
        node,
        &root,
        &root.join("work"),
        false,
        false,
        &mut stats,
        &mut on_mount,
    );

    let result = mount.do_mount().unwrap();

    assert_eq!(result.operation, MagicOperation::Bind);
    assert_eq!(result.target, target);
    assert_eq!(mounted, vec![target.to_string_lossy().into_owned()]);
    assert_eq!(stats.active_mounts, mounted);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn readonly_remount_failure_is_propagated_without_active_mount() {
    let _fault_guard = crate::sys::faults::test_lock();
    let root = std::env::temp_dir().join(format!(
        "hybrid-mount-magic-remount-failure-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let target = root.join("hosts");
    let source = root.join("module-hosts");
    fs::create_dir_all(&root).unwrap();
    fs::write(&target, b"stock").unwrap();
    fs::write(&source, b"module").unwrap();

    let mut tree = MountTree::default();
    tree.insert(
        "hosts",
        MountSource {
            module_id: ModuleId::try_from("m").unwrap(),
            relative: "hosts".to_owned(),
            source_path: source,
            file_type: NodeFileType::RegularFile,
            replace: false,
            backend: MountMode::Magic,
        },
    );
    let node = tree.root.children.get("hosts").unwrap();
    let mut stats = MagicMountStats::default();
    let mut mounted = Vec::new();
    let mut on_mount = |path: &str| mounted.push(path.to_owned());
    let _fake_ops = crate::sys::faults::fake_magic_mount_ops();
    crate::sys::faults::enable_next_magic_remount_failure();
    crate::sys::faults::enable_next_unmount_ebusy_failure();
    let mut mount = MagicMount::new(
        node,
        &root,
        &root.join("work"),
        false,
        false,
        &mut stats,
        &mut on_mount,
    );

    let error = mount.do_mount().unwrap_err();

    crate::sys::faults::reset();
    assert!(error.to_string().contains("make file"), "{error}");
    assert!(
        error.to_string().contains("rollback magic mount"),
        "{error}"
    );
    assert!(error.to_string().contains("injected EBUSY"), "{error}");
    assert!(mounted.is_empty());
    assert_eq!(stats.mounted_files, 0);
    assert!(stats.active_mounts.is_empty());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn nested_bind_registers_staging_target_for_rollback() {
    let root = std::env::temp_dir().join(format!(
        "hybrid-mount-magic-nested-bind-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let target = root.join("parent");
    let module_dir = root.join("module-parent");
    let source = root.join("module-child");
    let work = root.join("work");
    fs::create_dir_all(&target).unwrap();
    fs::create_dir_all(&module_dir).unwrap();
    fs::write(&source, b"module").unwrap();
    fs::create_dir_all(&work).unwrap();

    let mut tree = MountTree::default();
    tree.insert(
        "parent",
        MountSource {
            module_id: ModuleId::try_from("m").unwrap(),
            relative: "parent".to_owned(),
            source_path: module_dir,
            file_type: NodeFileType::Directory,
            replace: true,
            backend: MountMode::Magic,
        },
    );
    tree.insert(
        "parent/child",
        MountSource {
            module_id: ModuleId::try_from("m").unwrap(),
            relative: "parent/child".to_owned(),
            source_path: source,
            file_type: NodeFileType::RegularFile,
            replace: false,
            backend: MountMode::Magic,
        },
    );
    let node = tree.root.children.get("parent").unwrap();
    let mut stats = MagicMountStats::default();
    let mut mounted = Vec::new();
    let mut on_mount = |path: &str| mounted.push(path.to_owned());
    let _fake_ops = crate::sys::faults::fake_magic_mount_ops();
    let mut mount = MagicMount::new(node, &root, &work, false, false, &mut stats, &mut on_mount);

    let result = mount.do_mount().unwrap();

    assert_eq!(result.operation, MagicOperation::Replace);
    assert_eq!(
        stats.active_mounts,
        vec![
            target.join("child").to_string_lossy().into_owned(),
            target.to_string_lossy().into_owned(),
        ]
    );
    assert_eq!(
        mounted,
        vec![
            work.join("parent/child").to_string_lossy().into_owned(),
            target.to_string_lossy().into_owned(),
        ]
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn fake_symlink_success_does_not_register_a_mount_target() {
    use std::os::unix::fs::symlink;

    let root = std::env::temp_dir().join(format!(
        "hybrid-mount-magic-fake-symlink-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let target = root.join("link");
    let source = root.join("module-link");
    let work = root.join("work");
    fs::create_dir_all(&work).unwrap();
    symlink("hosts", &source).unwrap();

    let mut tree = MountTree::default();
    tree.insert(
        "link",
        MountSource {
            module_id: ModuleId::try_from("m").unwrap(),
            relative: "link".to_owned(),
            source_path: source,
            file_type: NodeFileType::Symlink,
            replace: false,
            backend: MountMode::Magic,
        },
    );
    let node = tree.root.children.get("link").unwrap();
    let mut stats = MagicMountStats::default();
    let mut mounted = Vec::new();
    let mut on_mount = |path: &str| mounted.push(path.to_owned());
    let _fake_ops = crate::sys::faults::fake_magic_mount_ops();
    let mut mount = MagicMount::new(node, &root, &work, false, false, &mut stats, &mut on_mount);

    let result = mount.do_mount().unwrap();

    assert_eq!(result.operation, MagicOperation::Symlink);
    assert_eq!(result.target, target);
    assert!(mounted.is_empty());
    assert!(stats.active_mounts.is_empty());
    assert_eq!(
        fs::read_link(work.join("link")).unwrap(),
        PathBuf::from("hosts")
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn fake_directory_move_reports_real_and_staging_targets_separately() {
    let root = std::env::temp_dir().join(format!(
        "hybrid-mount-magic-fake-move-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let target = root.join("etc");
    let module_dir = root.join("module-etc");
    let source = module_dir.join("hosts");
    let work = root.join("work");
    fs::create_dir_all(&target).unwrap();
    fs::create_dir_all(&module_dir).unwrap();
    fs::create_dir_all(&work).unwrap();
    fs::write(&source, b"module").unwrap();

    let mut tree = MountTree::default();
    tree.insert(
        "etc",
        MountSource {
            module_id: ModuleId::try_from("m").unwrap(),
            relative: "etc".to_owned(),
            source_path: module_dir,
            file_type: NodeFileType::Directory,
            replace: false,
            backend: MountMode::Magic,
        },
    );
    tree.insert(
        "etc/hosts",
        MountSource {
            module_id: ModuleId::try_from("m").unwrap(),
            relative: "etc/hosts".to_owned(),
            source_path: source,
            file_type: NodeFileType::RegularFile,
            replace: false,
            backend: MountMode::Magic,
        },
    );
    let node = tree.root.children.get("etc").unwrap();
    let mut stats = MagicMountStats::default();
    let mut rollback_targets = Vec::new();
    let mut on_mount = |path: &str| rollback_targets.push(path.to_owned());
    let _fake_ops = crate::sys::faults::fake_magic_mount_ops();
    let mut mount = MagicMount::new(node, &root, &work, false, false, &mut stats, &mut on_mount);

    let result = mount.do_mount().unwrap();

    assert_eq!(result.operation, MagicOperation::Move);
    assert_eq!(result.target, target);
    assert_eq!(
        stats.active_mounts,
        vec![
            target.join("hosts").to_string_lossy().into_owned(),
            target.to_string_lossy().into_owned(),
        ]
    );
    assert_eq!(
        rollback_targets,
        vec![
            work.join("etc/hosts").to_string_lossy().into_owned(),
            target.to_string_lossy().into_owned(),
        ]
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn fake_replace_success_returns_replace_result_and_mount_target() {
    let root = std::env::temp_dir().join(format!(
        "hybrid-mount-magic-fake-replace-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let target = root.join("etc");
    let source = root.join("module-etc");
    let work = root.join("work");
    fs::create_dir_all(&target).unwrap();
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&work).unwrap();

    let mut tree = MountTree::default();
    tree.insert(
        "etc",
        MountSource {
            module_id: ModuleId::try_from("m").unwrap(),
            relative: "etc".to_owned(),
            source_path: source,
            file_type: NodeFileType::Directory,
            replace: true,
            backend: MountMode::Magic,
        },
    );
    let node = tree.root.children.get("etc").unwrap();
    let mut stats = MagicMountStats::default();
    let mut mounted = Vec::new();
    let mut on_mount = |path: &str| mounted.push(path.to_owned());
    let _fake_ops = crate::sys::faults::fake_magic_mount_ops();
    let mut mount = MagicMount::new(node, &root, &work, false, false, &mut stats, &mut on_mount);

    let result = mount.do_mount().unwrap();

    assert_eq!(result.operation, MagicOperation::Replace);
    assert_eq!(result.target, target);
    assert_eq!(mounted, vec![target.to_string_lossy().into_owned()]);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn operation_faults_fail_before_mount_syscalls() {
    let _fault_guard = crate::sys::faults::test_lock();
    crate::sys::faults::reset();

    crate::sys::faults::enable_next_magic_bind_failure();
    let error = magic_mount_bind(Path::new("/source"), Path::new("/target")).unwrap_err();
    assert!(error.to_string().contains("magic bind"), "{error}");

    crate::sys::faults::enable_next_magic_remount_failure();
    let error = magic_mount_remount(
        Path::new("/target"),
        MountFlags::RDONLY | MountFlags::BIND,
        "",
    )
    .unwrap_err();
    assert!(error.to_string().contains("magic remount"), "{error}");

    crate::sys::faults::enable_next_magic_move_failure();
    let error = magic_mount_move(Path::new("/source"), Path::new("/target")).unwrap_err();
    assert!(error.to_string().contains("magic move"), "{error}");

    crate::sys::faults::enable_next_magic_symlink_failure();
    let error = clone_symlink(Path::new("/source"), Path::new("/target")).unwrap_err();
    assert!(error.to_string().contains("magic symlink"), "{error}");

    crate::sys::faults::reset();
}

#[test]
fn magic_stats_track_successful_modules_without_mount_syscalls() {
    let mut tree = MountTree::default();
    tree.insert(
        "hosts",
        MountSource {
            module_id: ModuleId::try_from("hosts_mod").unwrap(),
            relative: "hosts".to_owned(),
            source_path: PathBuf::from("/data/adb/modules/hosts_mod/hosts"),
            file_type: NodeFileType::RegularFile,
            replace: false,
            backend: MountMode::Magic,
        },
    );

    let node = tree.root.children.get("hosts").unwrap();
    let mut stats = MagicMountStats::default();
    record_module_success(&mut stats, node);
    record_module_success(&mut stats, node);

    assert_eq!(
        stats.mounted_module_ids,
        BTreeSet::from(["hosts_mod".to_owned()])
    );
}
