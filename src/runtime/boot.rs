// SPDX-License-Identifier: GPL-3.0-only
//! Boot and soft-reboot ownership. Never infer ownership from a shared mount source.

use super::{
    device,
    ledger::{self, Ledger},
    lifecycle, mounts,
    rules::{self, RuleKernel, SavedRule},
};
use crate::errors::{Error, Result};
use crate::plan::MountPlan;
use crate::state::RunState;
use crate::sys::mountinfo::{self, MountEntry};
use std::fs;

pub struct BootSession {
    baseline: Vec<MountEntry>,
    original_uids: Vec<u32>,
}

pub fn already_applied() -> Result<bool> {
    Ok(device::load()?.phase == "ready")
}

pub fn start() -> Result<BootSession> {
    let mut saved = device::load()?;
    saved.require_clean()?;
    let baseline = mountinfo::mount_entries()?;
    reject_legacy(&baseline)?;
    let original_uids = check_untracked_vfs()?;
    saved.generation += 1;
    saved.phase = "applying".into();
    saved.error = None;
    ledger::save(&saved)?;
    Ok(BootSession {
        baseline,
        original_uids,
    })
}

pub fn planned_rules(plan: &MountPlan) -> Result<Vec<SavedRule>> {
    crate::vfs::rule::build_vfs_rules(&plan.tree)
        .iter()
        .map(|rule| {
            let encoded = crate::vfs::protocol::encode_rule(rule)?;
            Ok(SavedRule {
                module_id: rule.module_id.to_string(),
                virtual_path: String::from_utf8(encoded.virtual_path)
                    .map_err(|e| Error::msg(e.to_string()))?,
                real_path: String::from_utf8(encoded.real_path)
                    .map_err(|e| Error::msg(e.to_string()))?,
                flags: encoded.flags,
                uid: 0,
            })
        })
        .collect()
}

pub fn stage_plan(plan: &MountPlan, requested_uids: &[u32]) -> Result<()> {
    let mut saved = device::load()?;
    saved.pending_rules = planned_rules(plan)?;
    saved.isolated_uids = requested_uids.to_vec();
    saved.non_vfs_modules = plan
        .magic_module_ids
        .iter()
        .chain(&plan.overlay_module_ids)
        .map(ToString::to_string)
        .collect();
    ledger::save(&saved)
}

pub fn finish(session: BootSession, successful: bool, effect_targets: &[String]) -> Result<()> {
    let mut saved = device::load()?;
    let state = RunState::load_or_default();
    let targets = crate::pipeline::runtime_mount_targets(&state, effect_targets);
    saved.mounts = mounts::capture(&targets, &session.baseline)?;
    saved.ksu_unmounts = Some(crate::utils::ksu::committed_unmounts());
    let mut verification = Ok(());
    if !saved.pending_rules.is_empty() || crate::vfs::available() {
        let mut kernel = device::Kernel::inspect()?;
        let current = kernel.list()?;
        verification =
            lifecycle::confirm_boot_rules(&saved.pending_rules, &state.vfs_active_mounts, &current);
        saved.rules = saved
            .pending_rules
            .iter()
            .filter(|expected| current.iter().any(|r| rules::semantic_equal(expected, r)))
            .cloned()
            .collect();
        saved.isolated_uids = lifecycle::owned_uids(
            &saved.isolated_uids,
            &session.original_uids,
            &kernel.uids()?,
        );
    } else {
        saved.isolated_uids.clear();
    }
    saved.modules = match fs::read(crate::defs::SCAN_RET_PATH) {
        Ok(bytes) => serde_json::from_slice(&bytes)?,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(err) => return Err(err.into()),
    };
    if verification.is_ok() {
        saved.pending_rules.clear();
    }
    saved.phase = if successful && verification.is_ok() {
        "ready"
    } else {
        "error"
    }
    .into();
    saved.error = verification
        .as_ref()
        .err()
        .map(ToString::to_string)
        .or_else(|| {
            (!successful).then(|| "boot pipeline failed; inspect status before recovery".into())
        });
    ledger::save(&saved)?;
    verification
}

pub fn cleanup() -> Result<()> {
    device::enter_init_namespace()?;
    let _lock = ledger::OperationLock::acquire()?;
    let mut saved = device::load()?;
    if saved.phase == "clean" {
        saved.require_clean()?;
        if saved.generation == 0 {
            reject_legacy(&mountinfo::mount_entries()?)?;
        }
        check_untracked_vfs()?;
        return super::hot::refresh_snapshots(&saved);
    }
    // Interrupted application may have installed mounts before their IDs were committed.
    if saved.phase == "applying" || saved.phase == "error" {
        return Err(Error::msg(
            "incomplete runtime operation; full reboot required, refusing to guess ownership",
        ));
    }
    saved.owned_unmounts()?;
    mounts::validate(&saved.mounts)?;
    saved.phase = "cleaning".into();
    ledger::save(&saved)?;
    let result = cleanup_owned(&mut saved);
    if let Err(err) = result {
        saved.error = Some(err.to_string());
        ledger::save(&saved)?;
        return Err(err);
    }
    saved.phase = "clean".into();
    saved.error = None;
    saved.non_vfs_modules.clear();
    saved.pending_rules.clear();
    saved.modules.iter_mut().for_each(|m| m.is_mounted = false);
    ledger::save(&saved)?;
    super::hot::refresh_snapshots(&saved)?;
    Ok(())
}

fn cleanup_owned(saved: &mut Ledger) -> Result<()> {
    if !saved.rules.is_empty() || !saved.isolated_uids.is_empty() {
        let mut kernel = device::Kernel::open()?;
        let current = kernel.list()?;
        // Missing records may have been removed by an earlier cleanup attempt. Drift is refused.
        let mut remaining = Vec::new();
        for expected in &saved.rules {
            if let Some(actual) = current
                .iter()
                .find(|r| r.virtual_path == expected.virtual_path && r.uid == expected.uid)
            {
                if !rules::semantic_equal(expected, actual) {
                    return Err(Error::msg(format!(
                        "owned VFS rule changed externally: {}",
                        expected.virtual_path
                    )));
                }
                remaining.push(expected.clone());
            }
        }
        let _guard = crate::pipeline::VfsBootGuard::arm()?;
        rules::reconcile(&mut kernel, &remaining, &[])?;
        kernel.remove_uids(&saved.isolated_uids)?;
        saved.rules.clear();
        saved.isolated_uids.clear();
        ledger::save(saved)?;
    }
    saved.release_mount_resources(mounts::cleanup, crate::utils::ksu::release_unmounts)?;
    ledger::save(saved)?;
    // The boot semaphore is only a deduplication marker, not the operation mutex.
    match fs::remove_dir("/dev/hybrid_mount_single_instance") {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    Ok(())
}

fn reject_legacy(current: &[MountEntry]) -> Result<()> {
    let entries = current
        .iter()
        .map(|m| {
            (
                m.mount_point.display().to_string(),
                m.fs_type.clone(),
                m.mount_source.clone(),
            )
        })
        .collect::<Vec<_>>();
    lifecycle::reject_legacy_mounts(&RunState::load_or_default(), &entries)
}

fn check_untracked_vfs() -> Result<Vec<u32>> {
    if !crate::vfs::available() {
        return Ok(Vec::new());
    }
    let mut kernel = device::Kernel::inspect()?;
    if kernel.list()?.iter().any(|r| !rules::derived_parent(r)) {
        return Err(Error::msg(
            "untracked VFS rules remain; refuse a full pipeline rebuild",
        ));
    }
    kernel.uids()
}
