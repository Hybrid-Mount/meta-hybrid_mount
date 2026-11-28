// SPDX-License-Identifier: GPL-3.0-only

use super::{
    boot, device,
    ledger::{self, Ledger, OperationLock},
    policy, rules, transaction,
};
use crate::{
    defs,
    errors::{Error, Result},
    framework::config::Config,
    framework::scan,
    framework::state::{self, RunState},
    mount::model::ModuleId,
};
use serde::Serialize;
use std::{collections::BTreeSet, path::Path};

#[derive(Serialize)]
struct ModuleStatus {
    id: String,
    active: bool,
    eligible: bool,
    reload_reason: Option<String>,
    reason: Option<String>,
}
#[derive(Serialize)]
pub(crate) struct Status {
    supported: bool,
    reason: Option<String>,
    generation: u64,
    modules: Vec<ModuleStatus>,
}

fn guard() -> Result<()> {
    match crate::mount::vfs::boot_guard::inspect()? {
        // A marker of another build, or the legacy marker without identity, is retired by the
        // mutation itself; only a marker this build wrote still refuses the operation.
        crate::mount::vfs::boot_guard::VfsGuardVerdict::Absent
        | crate::mount::vfs::boot_guard::VfsGuardVerdict::Foreign { .. } => Ok(()),
        crate::mount::vfs::boot_guard::VfsGuardVerdict::Own(record) => Err(Error::msg(format!(
            "VFS crash guard is present; hot operations will not bypass it ({})",
            record.describe()
        ))),
    }
}

fn config_and_modules(saved: &Ledger) -> Result<(Config, Vec<scan::ModuleRecord>)> {
    let config = Config::load_for_boot(Path::new(defs::CONFIG_PATH))?;
    let modules = scan::list_modules_with(
        &config.moduledir,
        &config.scan_partition_names(Path::new("/")),
        |id, _| Ok(saved.lua_edits.get(id).cloned().unwrap_or_default()),
    )?;
    Ok((config, modules))
}

fn status() -> Result<Status> {
    let saved = device::load()?;
    saved.require_ready()?;
    guard()?;
    let mut kernel = device::Kernel::open(&saved)?;
    rules::verify_owned(&mut kernel, &saved.rules)?;
    super::mounts::validate(&saved.mounts)?;
    let (config, modules) = config_and_modules(&saved)?;
    let layout = crate::framework::scan::partitions::detect(&config)?;
    let mut remaining: BTreeSet<_> = saved
        .rules
        .iter()
        .map(|r| r.module_id.as_str())
        .filter(|id| *id != rules::LUA_OWNER)
        .collect();
    let mut rows = Vec::new();
    for module in modules {
        let active = remaining.remove(module.id.as_str());
        let plan_error = policy::plan_hot_module(&module, &config, &layout, &saved)
            .err()
            .map(|e| e.to_string());
        let can_unload = active && !saved.non_vfs_modules.contains(module.id.as_str());
        rows.push(ModuleStatus {
            id: module.id.to_string(),
            active,
            eligible: can_unload || plan_error.is_none(),
            reload_reason: if active { plan_error.clone() } else { None },
            reason: if can_unload { None } else { plan_error },
        });
    }
    // Preserve the ledger's order for modules removed since boot.
    for rule in &saved.rules {
        if remaining.remove(rule.module_id.as_str()) {
            let mixed = saved.non_vfs_modules.contains(&rule.module_id);
            rows.push(ModuleStatus {
                id: rule.module_id.clone(),
                active: true,
                eligible: !mixed,
                reload_reason: Some("installed module not found".into()),
                reason: mixed
                    .then(|| "module has active Magic/Overlay ownership; reboot required".into()),
            });
        }
    }
    Ok(Status {
        supported: true,
        reason: None,
        generation: saved.generation,
        modules: rows,
    })
}

pub(crate) fn status_report() -> Status {
    (|| {
        crate::platform::namespace::enter_init_namespace()?;
        let _lock = OperationLock::acquire()?;
        status()
    })()
    .unwrap_or_else(|error| Status {
        supported: false,
        reason: Some(error.to_string()),
        generation: 0,
        modules: Vec::new(),
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModuleAction {
    Load,
    Reload,
    Unload,
}

pub(crate) fn apply_module(
    action: ModuleAction,
    module_id: &ModuleId,
) -> Result<serde_json::Value> {
    crate::platform::namespace::enter_init_namespace()?;
    let _lock = OperationLock::acquire()?;
    let generation = apply(action, module_id.as_str())?;
    let saved = device::load()?;
    let errors: Vec<_> = saved
        .modules
        .iter()
        .flat_map(|module| &module.partition_errors)
        .collect();
    let operations: Vec<_> = saved
        .modules
        .iter()
        .flat_map(|module| &module.operation_errors)
        .collect();
    Ok(serde_json::json!({
        "ok": true,
        "generation": generation,
        "degraded": !errors.is_empty() || !operations.is_empty(),
        "partition_errors": errors,
        "operation_errors": operations,
    }))
}

fn apply(action: ModuleAction, module_id: &str) -> Result<u64> {
    let mut saved = device::load()?;
    saved.require_ready()?;
    guard()?;
    let mut kernel = device::Kernel::open(&saved)?;
    saved.vfs_provider = Some(kernel.0.provider().as_str().into());
    rules::verify_owned(&mut kernel, &saved.rules)?;
    super::mounts::validate(&saved.mounts)?;
    if saved.non_vfs_modules.contains(module_id) {
        return Err(Error::msg(
            "module has active Magic/Overlay ownership; reboot required",
        ));
    }
    let before = saved
        .rules
        .iter()
        .filter(|r| r.module_id == module_id)
        .cloned()
        .collect::<Vec<_>>();
    if action == ModuleAction::Load && !before.is_empty() {
        return Err(Error::msg("module is already active; use reload"));
    }
    if action != ModuleAction::Load && before.is_empty() {
        return Err(Error::msg("module has no owned active VFS rules"));
    }
    let mut updated_module = None;
    let mut requested_uids = Vec::new();
    let after = if action == ModuleAction::Unload {
        Vec::new()
    } else {
        let (config, mut modules) = config_and_modules(&saved)?;
        let index = modules
            .iter()
            .position(|m| m.id.as_str() == module_id)
            .ok_or_else(|| Error::msg("installed module not found"))?;
        // Validate the complete configured plan as well as current runtime ownership.
        let disabled = modules[index].disabled;
        modules[index].disabled = false;
        let layout = crate::framework::scan::partitions::detect(&config)?;
        let full_plan = crate::framework::plan::build_plan(&crate::framework::plan::PlanInput {
            modules: &modules,
            config: &config,
            layout: &layout,
            vfs_available: true,
        })?;
        modules[index].disabled = disabled;
        let module = &modules[index];
        let plan = policy::plan_hot_module(module, &config, &layout, &saved)?;
        requested_uids = config.vfs_isolate_uids.clone();
        updated_module = state::app_modules(
            std::slice::from_ref(module),
            &config,
            &plan,
            &[],
            &BTreeSet::from([module_id.to_owned()]),
        )
        .into_iter()
        .next();
        if saved
            .lua_edits
            .values()
            .any(|edits| !edits.removed.is_empty())
        {
            let visible: Vec<_> = boot::planned_rules(&full_plan)?
                .into_iter()
                .filter(|rule| rule.module_id == module_id)
                .collect();
            if visible.is_empty() {
                return Err(Error::msg(
                    "module has no VFS rules in the merged mount plan",
                ));
            }
            visible
        } else {
            boot::planned_rules(&plan)?
        }
    };
    // Validation errors must leave the ready ledger untouched. In particular a
    // foreign new target is not an introduced rule requiring rollback.
    rules::preflight(&mut kernel, &before, &after)?;
    let _guard = crate::mount::vfs::boot_guard::arm_for_mutation(
        crate::mount::vfs::boot_guard::VfsMutationSource::Runtime,
    )?;
    transaction::reconcile(
        &mut kernel,
        &mut saved,
        &before,
        &after,
        &requested_uids,
        ledger::save,
    )?;
    saved.generation += 1;
    if let Some(module) = updated_module {
        saved.modules.retain(|m| m.id.as_str() != module_id);
        saved.modules.push(module);
    }
    for module in &mut saved.modules {
        if !saved.non_vfs_modules.contains(module.id.as_str()) {
            module.is_mounted = saved
                .rules
                .iter()
                .any(|r| r.module_id == module.id.as_str());
        }
    }
    // Ownership is complete; a snapshot failure can still be cleaned up safely.
    saved.phase = "syncing".into();
    ledger::save(&saved)?;
    refresh_snapshots(&saved)?;
    saved.phase = "ready".into();
    ledger::save(&saved)?;
    Ok(saved.generation)
}

pub fn refresh_snapshots(saved: &Ledger) -> Result<()> {
    let mut state = RunState::load_or_default();
    if saved.phase == "clean" {
        state.overlay_modules.clear();
        state.magic_modules.clear();
        state.overlay_active_mounts.clear();
        state.magic_active_mounts.clear();
        state.mount_stats = Default::default();
        state.mode_stats = Default::default();
        state.mount_point.clear();
        state.storage_mode = defs::NO_STORAGE_MODE.into();
        state.rollback_status = Some("clean".into());
    }
    state.vfs_modules = saved
        .rules
        .iter()
        .filter(|rule| rule.module_id != rules::LUA_OWNER)
        .map(|r| r.module_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    state.vfs_active_mounts = saved
        .rules
        .iter()
        .map(|r| r.virtual_path.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    state.mode_stats.vfs = state.vfs_modules.len();
    state.vfs_provider = (!saved.rules.is_empty())
        .then(|| saved.vfs_provider.clone().unwrap_or_else(|| "hm".into()));
    state.active_mounts = state
        .overlay_active_mounts
        .iter()
        .chain(&state.magic_active_mounts)
        .chain(&state.vfs_active_mounts)
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    state.confirmed_active_mounts = state.active_mounts.clone();
    state.partition_errors = saved
        .modules
        .iter()
        .flat_map(|m| m.partition_errors.iter().cloned())
        .collect();
    state.operation_errors = saved
        .modules
        .iter()
        .flat_map(|m| m.operation_errors.iter().cloned())
        .collect();
    state.degraded = !state.partition_errors.is_empty() || !state.operation_errors.is_empty();
    state
        .vfs_error_modules
        .retain(|m| !state.vfs_modules.contains(m));
    if state.vfs_error_modules.is_empty() {
        state.vfs_error = None;
    }
    state.save()?;
    state::write_scan_ret(&saved.modules)?;
    crate::framework::state::description::update_description(
        &state.storage_mode,
        state.mode_stats.overlayfs,
        state.mode_stats.magicmount,
        state.mode_stats.vfs,
    );
    Ok(())
}
