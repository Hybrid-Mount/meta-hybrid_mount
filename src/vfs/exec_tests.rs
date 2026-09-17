// SPDX-License-Identifier: GPL-3.0-only

use super::*;
use crate::config::Mode;
use crate::errors::Error;
use crate::mount_tree::{MountSource, MountTree, NodeFileType};
use crate::vfs::protocol::FLAG_WHITEOUT;
use std::path::PathBuf;

/// Kernel double: counts calls, captures the uids sent, and can fail mid-batch.
#[derive(Default)]
struct RecordingKernel {
    applied: usize,
    removed: usize,
    uids_added: Vec<u32>,
    fail_apply: bool,
}

impl VfsKernel for RecordingKernel {
    fn version(&mut self) -> Result<String> {
        Ok("hm1".to_owned())
    }

    fn apply_rules(&mut self, rules: &[EncodedRule]) -> Result<()> {
        self.applied += rules.len();
        if self.fail_apply {
            return Err(Error::VfsProtocol {
                detail: "mid-batch failure".to_owned(),
            });
        }
        Ok(())
    }

    fn add_uids(&mut self, uids: &[u32]) -> Result<()> {
        self.uids_added = uids.to_vec();
        Ok(())
    }

    fn remove_rules(&mut self, rules: &[EncodedRule]) -> Result<()> {
        self.removed += rules.len();
        Ok(())
    }
}

fn source(module: &str, relative: &str, file_type: NodeFileType) -> MountSource {
    MountSource {
        module_id: ModuleId::try_from(module).unwrap(),
        relative: relative.to_owned(),
        source_path: PathBuf::from(format!("/data/adb/modules/{module}/{relative}")),
        file_type,
        replace: false,
        backend: Mode::Vfs,
    }
}

/// A single-module plan; each key is both the in-module path and the target path.
fn plan_for(entries: &[(&str, NodeFileType)]) -> MountPlan {
    let mut tree = MountTree::default();
    for (relative, file_type) in entries {
        tree.insert(&format!("/{relative}"), source("m", relative, *file_type));
    }
    MountPlan {
        tree,
        vfs_module_ids: vec![ModuleId::try_from("m").unwrap()],
        ..MountPlan::default()
    }
}

fn hosts_and_whiteout() -> MountPlan {
    plan_for(&[
        ("system/etc/hosts", NodeFileType::RegularFile),
        ("system/etc/hidden.xml", NodeFileType::Whiteout),
    ])
}

#[test]
fn apply_rules_sends_rules_uids_and_reports_counts() {
    let planned = plan_rules(&hosts_and_whiteout()).unwrap();
    let mut kernel = RecordingKernel::default();

    let stats = apply_rules(&mut kernel, &planned, &[1000]).unwrap();

    assert_eq!(stats.injected, 1);
    assert_eq!(stats.whiteouts, 1);
    assert_eq!(stats.mounted_module_ids, vec!["m".to_owned()]);
    assert_eq!(
        stats.active_targets,
        vec![
            "/system/etc/hidden.xml".to_owned(),
            "/system/etc/hosts".to_owned()
        ]
    );
    assert_eq!(kernel.applied, 2);
    assert_eq!(kernel.removed, 0);
    assert_eq!(kernel.uids_added, vec![1000]);
    assert_eq!(
        planned.rules,
        vec![
            EncodedRule {
                flags: FLAG_WHITEOUT,
                virtual_path: b"/system/etc/hidden.xml".to_vec(),
                real_path: Vec::new(),
            },
            EncodedRule {
                flags: 0,
                virtual_path: b"/system/etc/hosts".to_vec(),
                real_path: b"/data/adb/modules/m/system/etc/hosts".to_vec(),
            },
        ]
    );
}

#[test]
fn planned_rules_are_returned_for_rollback() {
    let planned = plan_rules(&plan_for(&[(
        "system/etc/hosts",
        NodeFileType::RegularFile,
    )]))
    .unwrap();
    let mut kernel = RecordingKernel::default();

    apply_rules(&mut kernel, &planned, &[]).unwrap();

    assert_eq!(planned.rules.len(), 1);
    assert_eq!(planned.rules[0].virtual_path, b"/system/etc/hosts");
    kernel.remove_rules(&planned.rules).unwrap();
    assert_eq!(kernel.removed, 1);
    assert_eq!(kernel.applied, 1);
}

#[test]
fn apply_rules_skips_uid_call_when_no_isolated_uids() {
    let planned = plan_rules(&plan_for(&[(
        "system/etc/hosts",
        NodeFileType::RegularFile,
    )]))
    .unwrap();
    let mut kernel = RecordingKernel::default();

    apply_rules(&mut kernel, &planned, &[]).unwrap();

    assert!(kernel.uids_added.is_empty());
}

/// A uid is sent once: the kernel answers -EEXIST for one already isolated, which a
/// duplicated config entry would otherwise turn into a failed batch.
#[test]
fn apply_rules_deduplicates_isolated_uids() {
    let planned = plan_rules(&plan_for(&[(
        "system/etc/hosts",
        NodeFileType::RegularFile,
    )]))
    .unwrap();
    let mut kernel = RecordingKernel::default();

    apply_rules(&mut kernel, &planned, &[1000, 1010, 1000]).unwrap();

    assert_eq!(kernel.uids_added, vec![1000, 1010]);
}

/// The batch is not atomic, so the applied prefix must be deleted after a failure.
/// Non-strict VFS is optional and must not take Overlay / Magic down with it.
#[test]
fn non_strict_apply_failure_removes_the_prefix_and_degrades() {
    let planned = plan_rules(&hosts_and_whiteout()).unwrap();
    let mut kernel = RecordingKernel {
        fail_apply: true,
        ..RecordingKernel::default()
    };

    let stats = apply_rules_with_policy(&mut kernel, &planned, &[], false).unwrap();

    assert_eq!(stats, None);
    assert_eq!(kernel.applied, 2);
    assert_eq!(kernel.removed, 2);
}

#[test]
fn strict_apply_failure_still_removes_the_prefix_and_reports_the_error() {
    let planned = plan_rules(&plan_for(&[(
        "system/etc/hosts",
        NodeFileType::RegularFile,
    )]))
    .unwrap();
    let mut kernel = RecordingKernel {
        fail_apply: true,
        ..RecordingKernel::default()
    };

    let err = apply_rules_with_policy(&mut kernel, &planned, &[], true).unwrap_err();

    assert!(matches!(err, Error::VfsProtocol { .. }));
    assert_eq!(kernel.removed, 1);
}
