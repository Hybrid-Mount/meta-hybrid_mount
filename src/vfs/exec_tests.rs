// SPDX-License-Identifier: GPL-3.0-only

use super::*;
use crate::config::Mode;
use crate::errors::Result;
use crate::module_id::ModuleId;
use crate::mount_tree::{MountSource, MountTree, NodeFileType};
use crate::plan::MountPlan;
use crate::vfs::backend::VfsKernel;
use crate::vfs::protocol::EncodedRule;
use std::path::PathBuf;

#[derive(Default)]
struct RecordingKernel {
    applied: usize,
    uids_added: Vec<u32>,
}

impl VfsKernel for RecordingKernel {
    fn version(&mut self) -> Result<String> {
        Ok("20".to_owned())
    }

    fn apply_rules(&mut self, rules: &[EncodedRule]) -> Result<()> {
        self.applied += rules.len();
        Ok(())
    }

    fn add_uids(&mut self, uids: &[u32]) -> Result<()> {
        self.uids_added = uids.to_vec();
        Ok(())
    }

    fn clear_rules(&mut self) -> Result<()> {
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

#[test]
fn apply_plan_sends_rules_uids_and_reports_counts() {
    let mut tree = MountTree::default();
    tree.insert(
        "/system/etc/hosts",
        source("m", "system/etc/hosts", NodeFileType::RegularFile),
    );
    tree.insert(
        "/system/etc/hidden.xml",
        source("m", "system/etc/hidden.xml", NodeFileType::Whiteout),
    );

    let plan = MountPlan {
        tree,
        vfs_module_ids: vec![ModuleId::try_from("m").unwrap()],
        ..MountPlan::default()
    };
    let mut kernel = RecordingKernel::default();

    let stats = apply_plan(&mut kernel, &plan, &[1000]).unwrap();

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
    assert_eq!(kernel.uids_added, vec![1000]);
}

#[test]
fn apply_plan_skips_uid_call_when_no_isolated_uids() {
    let mut tree = MountTree::default();
    tree.insert(
        "/system/etc/hosts",
        source("m", "system/etc/hosts", NodeFileType::RegularFile),
    );
    let plan = MountPlan {
        tree,
        vfs_module_ids: vec![ModuleId::try_from("m").unwrap()],
        ..MountPlan::default()
    };
    let mut kernel = RecordingKernel::default();

    apply_plan(&mut kernel, &plan, &[]).unwrap();

    assert!(kernel.uids_added.is_empty());
}
