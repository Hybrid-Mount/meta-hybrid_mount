// SPDX-License-Identifier: GPL-3.0-only

use super::{
    boot, device,
    ledger::{self, Ledger, OperationLock},
    policy, rules, transaction,
};
use crate::{
    config::Config,
    defs,
    errors::{Error, Result},
    module_id::ModuleId,
    scanner,
    state::{self, RunState},
};
use serde::Serialize;
use std::{collections::BTreeSet, path::Path};

#[derive(Serialize)]
struct ModuleStatus {
    id: String,
    active: bool,
    eligible: bool,
    reason: Option<String>,
}
#[derive(Serialize)]
struct Status {
    supported: bool,
    reason: Option<String>,
    generation: u64,
    modules: Vec<ModuleStatus>,
}

fn guard() -> Result<()> {
    if Path::new(defs::VFS_BOOT_GUARD_PATH).exists() {
        return Err(Error::msg(
            "VFS crash guard is present; hot operations will not bypass it",
        ));
    }
    Ok(())
}

fn config_and_modules() -> Result<(Config, Vec<scanner::ModuleRecord>)> {
    let config = Config::load_for_boot(Path::new(defs::CONFIG_PATH))?;
    let modules = scanner::list_modules(
        &config.moduledir,
        &crate::pipeline::managed_partition_names(),
    )?;
    Ok((config, modules))
}

fn status() -> Result<Status> {
    let saved = device::load()?;
    saved.require_ready()?;
    guard()?;
    let mut kernel = device::Kernel::open()?;
    rules::verify_owned(&mut kernel, &saved.rules)?;
    let (config, modules) = config_and_modules()?;
    let promoted = crate::pipeline::detect_promoted_partitions();
    let mut rows = Vec::new();
    for module in modules {
        let active = saved
            .rules
            .iter()
            .any(|r| r.module_id == module.id.as_str());
        // Existing pure-VFS ownership can always be unloaded, even if its files/config changed.
        let reason = if active && !saved.non_vfs_modules.contains(module.id.as_str()) {
            None
        } else {
            policy::plan_hot_module(&module, &config, &promoted, &saved)
                .err()
                .map(|e| e.to_string())
        };
        rows.push(ModuleStatus {
            id: module.id.to_string(),
            active,
            eligible: reason.is_none(),
            reason,
        });
    }
    for rule in &saved.rules {
        if !rows.iter().any(|m| m.id == rule.module_id) {
            rows.push(ModuleStatus {
                id: rule.module_id.clone(),
                active: true,
                eligible: !saved.non_vfs_modules.contains(&rule.module_id),
                reason: None,
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

pub fn handle(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("status") if args.len() == 1 => {
            let report = (|| {
                device::enter_init_namespace()?;
                let _lock = OperationLock::acquire()?;
                status()
            })()
            .unwrap_or_else(|e| Status {
                supported: false,
                reason: Some(e.to_string()),
                generation: 0,
                modules: Vec::new(),
            });
            println!("{}", serde_json::to_string(&report)?);
            Ok(())
        }
        Some("prepare-reboot") if args.len() == 1 => boot::cleanup(),
        Some(action @ ("load" | "reload" | "unload")) if args.len() == 2 => {
            let module_id = ModuleId::try_from(args[1].as_str())?;
            device::enter_init_namespace()?;
            let _lock = OperationLock::acquire()?;
            let generation = apply(action, module_id.as_str())?;
            println!("{}", serde_json::json!({"ok":true,"generation":generation}));
            Ok(())
        }
        _ => Err(Error::msg(
            "usage: hybrid-mount runtime status|prepare-reboot|load ID|unload ID|reload ID",
        )),
    }
}

fn apply(action: &str, module_id: &str) -> Result<u64> {
    let mut saved = device::load()?;
    saved.require_ready()?;
    guard()?;
    let mut kernel = device::Kernel::open()?;
    rules::verify_owned(&mut kernel, &saved.rules)?;
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
    if action == "load" && !before.is_empty() {
        return Err(Error::msg("module is already active; use reload"));
    }
    if action != "load" && before.is_empty() {
        return Err(Error::msg("module has no owned active VFS rules"));
    }
    let mut updated_module = None;
    let mut requested_uids = Vec::new();
    let after = if action == "unload" {
        Vec::new()
    } else {
        let (config, modules) = config_and_modules()?;
        let module = modules
            .iter()
            .find(|m| m.id.as_str() == module_id)
            .ok_or_else(|| Error::msg("installed module not found"))?;
        // Validate the complete configured plan as well as current runtime ownership.
        let mut configured = modules.clone();
        if let Some(requested) = configured.iter_mut().find(|m| m.id.as_str() == module_id) {
            requested.disabled = false;
        }
        let promoted = crate::pipeline::detect_promoted_partitions();
        crate::plan::build_plan(&crate::plan::PlanInput {
            modules: &configured,
            config: &config,
            promoted_partitions: &promoted,
            vfs_available: true,
        })?;
        let plan = policy::plan_hot_module(module, &config, &promoted, &saved)?;
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
        boot::planned_rules(&plan)?
    };
    // Validation errors must leave the ready ledger untouched. In particular a
    // foreign new target is not an introduced rule requiring rollback.
    rules::preflight(&mut kernel, &before, &after)?;
    let _guard = crate::pipeline::VfsBootGuard::arm()?;
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
    state.vfs_provider = (!saved.rules.is_empty()).then(|| "hm".into());
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
    state
        .vfs_error_modules
        .retain(|m| !state.vfs_modules.contains(m));
    if state.vfs_error_modules.is_empty() {
        state.vfs_error = None;
    }
    state.save()?;
    state::write_scan_ret(&saved.modules)?;
    crate::module_status::update_description(
        &state.storage_mode,
        state.mode_stats.overlayfs,
        state.mode_stats.magicmount,
        state.mode_stats.vfs,
    );
    Ok(())
}
