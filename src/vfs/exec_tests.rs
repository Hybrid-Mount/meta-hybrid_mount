// SPDX-License-Identifier: GPL-3.0-only

use super::*;
use crate::config::Mode;
use crate::errors::Result;
use crate::module_id::ModuleId;
use crate::mount_tree::{MountSource, MountTree, NodeFileType};
use crate::plan::MountPlan;
use crate::vfs::backend::VfsKernel;
use crate::vfs::protocol::{EncodedRule, FLAG_WHITEOUT};
use std::path::PathBuf;

#[derive(Default)]
struct RecordingKernel {
    applied: usize,
    removed: usize,
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

#[test]
fn apply_rules_sends_rules_uids_and_reports_counts() {
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

    let planned = plan_rules(&plan).unwrap();
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

    let planned = plan_rules(&plan).unwrap();
    apply_rules(&mut kernel, &planned, &[]).unwrap();

    assert_eq!(planned.rules.len(), 1);
    assert_eq!(planned.rules[0].virtual_path, b"/system/etc/hosts");
    kernel.remove_rules(&planned.rules).unwrap();
    assert_eq!(kernel.removed, 1);
    assert_eq!(kernel.applied, 1);
}

#[test]
fn apply_rules_skips_uid_call_when_no_isolated_uids() {
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

    let planned = plan_rules(&plan).unwrap();
    apply_rules(&mut kernel, &planned, &[]).unwrap();

    assert!(kernel.uids_added.is_empty());
}

/// 重复 UID 不得让下发失败。内核的 ADD_UID 对已存在的 UID 回 -EEXIST，而配置里
/// 写两次同一 UID（或同一次启动内第二次运行流水线，UID 表仍在）都会命中它；
/// `ensure_status` 把任何负值当硬错误，会把整条挂载流水线拖进回滚。
/// 去重后只应下发一次。
#[test]
fn apply_rules_deduplicates_isolated_uids() {
    #[derive(Default)]
    struct UidRecordingKernel {
        uids_added: Vec<u32>,
    }

    impl VfsKernel for UidRecordingKernel {
        fn version(&mut self) -> Result<String> {
            Ok("hm1".to_owned())
        }

        fn apply_rules(&mut self, _rules: &[EncodedRule]) -> Result<()> {
            Ok(())
        }

        fn add_uids(&mut self, uids: &[u32]) -> Result<()> {
            self.uids_added = uids.to_vec();
            Ok(())
        }

        fn remove_rules(&mut self, _rules: &[EncodedRule]) -> Result<()> {
            Ok(())
        }
    }

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
    let mut kernel = UidRecordingKernel::default();

    let planned = plan_rules(&plan).unwrap();
    apply_rules(&mut kernel, &planned, &[1000, 1010, 1000]).unwrap();

    assert_eq!(kernel.uids_added, vec![1000, 1010]);
}

/// 下发中途失败时，已生效的前缀必须被删除；非 strict 模式下 VFS 自身失败不应拖垮
/// 已经成功的 Overlay / Magic 挂载，因此降级为「本次不使用 VFS」。
#[test]
fn non_strict_apply_failure_removes_the_prefix_and_degrades() {
    #[derive(Default)]
    struct FailingKernel {
        applied: usize,
        removed: usize,
    }

    impl VfsKernel for FailingKernel {
        fn version(&mut self) -> Result<String> {
            Ok("hm1".to_owned())
        }

        fn apply_rules(&mut self, rules: &[EncodedRule]) -> Result<()> {
            self.applied += rules.len();
            Err(crate::errors::Error::VfsProtocol {
                detail: "mid-batch failure".to_owned(),
            })
        }

        fn add_uids(&mut self, _uids: &[u32]) -> Result<()> {
            Ok(())
        }

        fn remove_rules(&mut self, rules: &[EncodedRule]) -> Result<()> {
            self.removed += rules.len();
            Ok(())
        }
    }

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
    let planned = plan_rules(&plan).unwrap();

    let mut kernel = FailingKernel::default();
    let stats = apply_rules_with_policy(&mut kernel, &planned, &[], false).unwrap();

    assert_eq!(stats, None);
    assert_eq!(kernel.applied, 1);
    assert_eq!(kernel.removed, 1);
}

/// strict 模式保留失败：VFS 是显式声明的必需后端，调用方要拿到错误。
#[test]
fn strict_apply_failure_still_removes_the_prefix_and_reports_the_error() {
    #[derive(Default)]
    struct FailingKernel {
        removed: usize,
    }

    impl VfsKernel for FailingKernel {
        fn version(&mut self) -> Result<String> {
            Ok("hm1".to_owned())
        }

        fn apply_rules(&mut self, _rules: &[EncodedRule]) -> Result<()> {
            Err(crate::errors::Error::VfsProtocol {
                detail: "mid-batch failure".to_owned(),
            })
        }

        fn add_uids(&mut self, _uids: &[u32]) -> Result<()> {
            Ok(())
        }

        fn remove_rules(&mut self, rules: &[EncodedRule]) -> Result<()> {
            self.removed += rules.len();
            Ok(())
        }
    }

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
    let planned = plan_rules(&plan).unwrap();

    let mut kernel = FailingKernel::default();
    let err = apply_rules_with_policy(&mut kernel, &planned, &[], true).unwrap_err();

    assert!(matches!(err, crate::errors::Error::VfsProtocol { .. }));
    assert_eq!(kernel.removed, 1);
}

/// 下发中途失败的场景：批次在调用内核之前就已完整构建，因此回滚仍能拿到全部
/// 规则（已生效的前缀必须删除，未生效的由 ENOENT 容忍）。
#[test]
fn plan_rules_holds_full_batch_before_any_kernel_call() {
    #[derive(Default)]
    struct FailingKernel {
        applied: usize,
        rolled_back: usize,
    }

    impl VfsKernel for FailingKernel {
        fn version(&mut self) -> Result<String> {
            Ok("20".to_owned())
        }

        fn apply_rules(&mut self, rules: &[EncodedRule]) -> Result<()> {
            self.applied += rules.len();
            Err(crate::errors::Error::VfsProtocol {
                detail: "mid-batch failure".to_owned(),
            })
        }

        fn add_uids(&mut self, _uids: &[u32]) -> Result<()> {
            Ok(())
        }

        fn remove_rules(&mut self, rules: &[EncodedRule]) -> Result<()> {
            self.rolled_back += rules.len();
            Ok(())
        }
    }

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

    let planned = plan_rules(&plan).unwrap();
    assert_eq!(planned.rules.len(), 2);

    let mut kernel = FailingKernel::default();
    assert!(apply_rules(&mut kernel, &planned, &[]).is_err());
    assert_eq!(kernel.applied, 2);

    kernel.remove_rules(&planned.rules).unwrap();
    assert_eq!(kernel.rolled_back, 2);
}
