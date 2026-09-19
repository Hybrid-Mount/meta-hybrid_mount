// SPDX-License-Identifier: GPL-3.0-only

use super::*;
use crate::config::Mode;
use crate::mount_tree::{MountTree, NodeFileType};
use crate::test_support::mount_source as source;

#[test]
fn file_and_symlink_become_inject_rules_in_tree_order() {
    let mut tree = MountTree::default();
    tree.insert(
        "/system/etc/hosts",
        source(
            "m",
            "system/etc/hosts",
            NodeFileType::RegularFile,
            Mode::Vfs,
        ),
    );
    tree.insert(
        "/system/etc/link",
        source("m", "system/etc/link", NodeFileType::Symlink, Mode::Vfs),
    );

    let rules = build_vfs_rules(&tree);

    assert_eq!(
        rules,
        vec![
            VfsRule {
                action: VfsAction::Inject {
                    virtual_path: "/system/etc/hosts".to_owned(),
                    real_path: PathBuf::from("/data/adb/modules/m/system/etc/hosts"),
                },
                module_id: ModuleId::try_from("m").unwrap(),
            },
            VfsRule {
                action: VfsAction::Inject {
                    virtual_path: "/system/etc/link".to_owned(),
                    real_path: PathBuf::from("/data/adb/modules/m/system/etc/link"),
                },
                module_id: ModuleId::try_from("m").unwrap(),
            },
        ]
    );
}

#[test]
fn whiteout_becomes_whiteout_rule() {
    let mut tree = MountTree::default();
    tree.insert(
        "/system/etc/hidden.xml",
        source(
            "m",
            "system/etc/hidden.xml",
            NodeFileType::Whiteout,
            Mode::Vfs,
        ),
    );

    assert_eq!(
        build_vfs_rules(&tree),
        vec![VfsRule {
            action: VfsAction::Whiteout {
                virtual_path: "/system/etc/hidden.xml".to_owned(),
            },
            module_id: ModuleId::try_from("m").unwrap(),
        }]
    );
}

#[test]
fn duplicate_targets_emit_one_rule_from_the_last_source() {
    let mut tree = MountTree::default();
    tree.insert(
        "/system/etc/hosts",
        source(
            "a_mod",
            "system/etc/hosts",
            NodeFileType::RegularFile,
            Mode::Vfs,
        ),
    );
    tree.insert(
        "/system/etc/hosts",
        source(
            "z_mod",
            "system/etc/hosts",
            NodeFileType::RegularFile,
            Mode::Vfs,
        ),
    );

    let rules = build_vfs_rules(&tree);

    assert_eq!(
        rules,
        vec![VfsRule {
            action: VfsAction::Inject {
                virtual_path: "/system/etc/hosts".to_owned(),
                real_path: PathBuf::from("/data/adb/modules/z_mod/system/etc/hosts"),
            },
            module_id: ModuleId::try_from("z_mod").unwrap(),
        }]
    );
}

#[test]
fn replace_directory_and_its_subtree_become_opaque_rules() {
    let mut tree = MountTree::default();
    let mut dir = source("m", "system/etc", NodeFileType::Directory, Mode::Vfs);
    dir.replace = true;
    tree.insert("/system/etc", dir);
    tree.insert(
        "/system/etc/sub/bar.conf",
        source(
            "m",
            "system/etc/sub/bar.conf",
            NodeFileType::RegularFile,
            Mode::Vfs,
        ),
    );

    let module = ModuleId::try_from("m").unwrap();
    assert_eq!(
        build_vfs_rules(&tree),
        vec![
            VfsRule {
                action: VfsAction::OpaqueDir {
                    virtual_path: "/system/etc".to_owned(),
                },
                module_id: module.clone(),
            },
            VfsRule {
                action: VfsAction::OpaqueDir {
                    virtual_path: "/system/etc/sub".to_owned(),
                },
                module_id: module.clone(),
            },
            VfsRule {
                action: VfsAction::Inject {
                    virtual_path: "/system/etc/sub/bar.conf".to_owned(),
                    real_path: PathBuf::from("/data/adb/modules/m/system/etc/sub/bar.conf"),
                },
                module_id: module,
            },
        ]
    );
}

#[test]
fn directory_without_replace_stays_structural() {
    let mut tree = MountTree::default();
    tree.insert(
        "/system/etc",
        source("m", "system/etc", NodeFileType::Directory, Mode::Vfs),
    );
    tree.insert(
        "/system/etc/sub/bar.conf",
        source(
            "m",
            "system/etc/sub/bar.conf",
            NodeFileType::RegularFile,
            Mode::Vfs,
        ),
    );

    assert_eq!(
        build_vfs_rules(&tree),
        vec![VfsRule {
            action: VfsAction::Inject {
                virtual_path: "/system/etc/sub/bar.conf".to_owned(),
                real_path: PathBuf::from("/data/adb/modules/m/system/etc/sub/bar.conf"),
            },
            module_id: ModuleId::try_from("m").unwrap(),
        }]
    );
}

#[test]
fn directory_and_other_backends_produce_no_rules() {
    let mut tree = MountTree::default();
    tree.insert(
        "/system/etc",
        source("m", "system/etc", NodeFileType::Directory, Mode::Vfs),
    );
    tree.insert(
        "/system/etc/hosts",
        source(
            "m",
            "system/etc/hosts",
            NodeFileType::RegularFile,
            Mode::Overlay,
        ),
    );

    assert!(build_vfs_rules(&tree).is_empty());
}
