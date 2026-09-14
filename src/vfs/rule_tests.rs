// SPDX-License-Identifier: GPL-3.0-only

use super::*;
use crate::config::Mode;
use crate::module_id::ModuleId;
use crate::mount_tree::{MountSource, MountTree, NodeFileType};
use std::path::PathBuf;

fn source(module: &str, relative: &str, file_type: NodeFileType, backend: Mode) -> MountSource {
    MountSource {
        module_id: ModuleId::try_from(module).unwrap(),
        relative: relative.to_owned(),
        source_path: PathBuf::from(format!("/data/adb/modules/{module}/{relative}")),
        file_type,
        replace: false,
        backend,
    }
}

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
