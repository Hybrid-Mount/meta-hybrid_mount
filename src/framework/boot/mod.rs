// SPDX-License-Identifier: GPL-3.0-only

//! The argument-free boot pipeline: read config, scan read-only, plan, run OverlayFS,
//! run Magic Mount, commit the KSU try-umount list, then write scan.ret and run/state.json.
//!
//! Mounts and shallow staging write only inside the runtime directory; module sources stay read-only.

pub(crate) mod hook;

#[cfg(any(target_os = "linux", target_os = "android"))]
mod custom;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[cfg(any(target_os = "linux", target_os = "android"))]
use rustix::mount::{
    MountFlags, MountPropagationFlags, UnmountFlags, mount, mount_change, unmount,
};
#[cfg(any(target_os = "linux", target_os = "android"))]
use std::fs;

use crate::defs;
use crate::errors::{Error, Result};
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::framework::config::{Config, OverlayMode};
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::framework::extension::{Extensions, HookPhase, MountKind};
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::framework::plan::{PlanInput, build_plan};
use crate::framework::scan::ModuleRecord;
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::framework::scan::list_modules_with;
use crate::framework::state::MountStatistics;
#[cfg(any(target_os = "linux", target_os = "android", test))]
use crate::framework::state::RunState;
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::framework::state::{app_modules, mounted_module_ids_for_snapshot, write_scan_ret};
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::mount::magic_mount::exec;
#[cfg(unix)]
use crate::mount::model::MountPlan;
#[cfg(unix)]
use crate::mount::overlayfs::layers::OverlayExecutionPlan;
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::mount::overlayfs::layers::{
    OverlayPlans, build_overlay_execution_plan, mount_layers,
    normalize_direct_overlay_layer_metadata,
};
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::mount::vfs::backend::{KeyringKernel, VfsKernel};
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::mount::vfs::exec::{VfsApplied, VfsExecStats};
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::mount::vfs::sys::KeyringChannel;
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::platform;
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::platform::effective_mount_source;
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::platform::sys::mountinfo::describe_path_mount;
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::timing::PhaseTimer;
#[cfg(any(target_os = "linux", target_os = "android"))]
use std::cell::RefCell;
#[cfg(any(target_os = "linux", target_os = "android"))]
use std::rc::Rc;

/// The single entry point for the argument-free boot pipeline.
pub fn run_mount_pipeline() -> Result<()> {
    run_pipeline(false)
}

pub fn run_boot_pipeline() -> Result<()> {
    run_pipeline(true)
}

fn run_pipeline(boot_hook: bool) -> Result<()> {
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        let _ = boot_hook;
        Err(Error::msg(
            "mount pipeline is only supported on linux/android",
        ))
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        crate::platform::namespace::enter_init_namespace()?;
        let _operation = crate::framework::runtime::ledger::OperationLock::acquire()?;
        if boot_hook && crate::framework::runtime::boot::already_applied()? {
            log::info!("boot generation already applied; skipping duplicate mount hook");
            return Ok(());
        }
        let session = crate::framework::runtime::boot::start()?;
        let mut mounted = MountedTargets::default();
        let outcome = run_mount_pipeline_impl(&mut mounted, &session.baseline);
        let recorded =
            crate::framework::runtime::boot::finish(session, outcome.is_ok(), &mounted.paths);
        match (outcome, recorded) {
            (Err(error), _) => Err(error),
            (Ok(()), result) => result,
        }
    }
}

/// Summarises the execution counters into state statistics (pure, so it tests across platforms).
pub fn pipeline_stats(
    overlay_dir_mounts: usize,
    shallow_overlay_mounts: usize,
    magic_files: usize,
    magic_symlinks: usize,
    ignored_entries: usize,
    magic_dirs: usize,
) -> MountStatistics {
    let successful =
        overlay_dir_mounts + shallow_overlay_mounts + magic_files + magic_symlinks + magic_dirs;

    MountStatistics {
        total_mounts: successful,
        successful_mounts: successful,
        failed_mounts: 0,
        files_mounted: magic_files,
        symlinks_created: magic_symlinks,
        overlayfs_mounts: overlay_dir_mounts + shallow_overlay_mounts,
        ignored_entries,
        magic_dirs,
    }
}

/// Merge successful targets from every backend into the stable WebUI contract.
///
/// VFS targets are injection points rather than kernel mounts, but the WebUI counts
/// them as active mount points, so they join the same sorted, deduplicated list.
pub fn merge_active_mounts(overlay: &[String], magic: &[String], vfs: &[String]) -> Vec<String> {
    overlay
        .iter()
        .chain(magic)
        .chain(vfs)
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Shallow directory planning for file-level overlay rules: one layer directory per source file.
pub fn staged_overlay_path(
    source: &Path,
    modules: &[ModuleRecord],
    storage_root: &Path,
) -> Result<PathBuf> {
    let module = modules
        .iter()
        .filter(|module| source.starts_with(&module.source_path))
        .max_by_key(|module| module.source_path.components().count())
        .ok_or_else(|| {
            Error::msg(format!(
                "overlay source is outside every scanned module: {}",
                source.display()
            ))
        })?;
    let relative = source.strip_prefix(&module.source_path).map_err(|err| {
        Error::msg(format!(
            "strip module prefix {} from {}: {err}",
            module.source_path.display(),
            source.display()
        ))
    })?;
    Ok(storage_root.join(module.id.as_str()).join(relative))
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[derive(Debug, Default)]
struct MountedTargets {
    paths: Vec<String>,
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn register_mounted_target(
    transaction: &mut crate::platform::sys::transaction::MountTransaction<'_>,
    mounted: &mut MountedTargets,
    target: &str,
) {
    let path = PathBuf::from(target);
    transaction.register_rollback_only(format!("mount:{target}"), move || {
        crate::platform::sys::mount::rollback_mount_target(&path)
    });
    mounted.paths.push(target.to_owned());
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[derive(Debug)]
struct MagicStagingGuard {
    path: PathBuf,
    cleanup: bool,
}

#[cfg(any(target_os = "linux", target_os = "android"))]
impl MagicStagingGuard {
    fn mount(path: PathBuf, mount_source: &str) -> Result<Self> {
        if let Err(err) = prepare_tmp_root(&path, mount_source) {
            let _ = fs::remove_dir_all(&path);
            return Err(err);
        }
        Ok(Self {
            path,
            cleanup: true,
        })
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn cleanup(mut self) -> Result<()> {
        self.cleanup_now(true)
    }

    fn cleanup_now(&mut self, strict: bool) -> Result<()> {
        if !self.cleanup {
            return Ok(());
        }

        if strict {
            cleanup_tmp_root(&self.path)?;
        } else {
            cleanup_tmp_root_best_effort(&self.path)?;
        }
        if crate::platform::sys::faults::should_fail_staging_remove() {
            return Err(Error::msg(format!(
                "injected magic staging remove failure: path={}",
                self.path.display()
            )));
        }
        match fs::remove_dir_all(&self.path) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                return Err(Error::msg(format!(
                    "remove magic staging directory {}: {err}",
                    self.path.display()
                )));
            }
        }
        self.cleanup = false;
        Ok(())
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
impl Drop for MagicStagingGuard {
    fn drop(&mut self) {
        if let Err(err) = self.cleanup_now(false) {
            log::warn!(
                "magic staging cleanup failed: path={}, error={err}",
                self.path.display()
            );
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn log_backend_capabilities() {
    match crate::mount::overlayfs::utils::is_overlay_supported() {
        Ok(supported) => log::info!("capability: overlayfs_supported={supported}"),
        Err(err) => log::warn!("capability probe failed: overlayfs_supported, error={err}"),
    }
    match crate::platform::sys::fs::is_overlay_xattr_supported() {
        Ok(supported) => log::info!("capability: tmpfs_xattr_supported={supported}"),
        Err(err) => log::warn!("capability probe failed: tmpfs_xattr, error={err}"),
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn prepare_overlay_storage(
    modules: &[ModuleRecord],
    plan: &mut MountPlan,
    execution_plan: &mut OverlayExecutionPlan,
    storage_root: &Path,
) -> Result<()> {
    log::info!(
        "overlay staging start: root={}, modules={}",
        storage_root.display(),
        plan.overlay_module_ids.len()
    );

    let stats = crate::mount::overlayfs::staging::stage_overlay_tree(&plan.tree, storage_root)
        .map_err(|err| Error::msg(format!("stage shared overlay tree: {err}")))?;
    log::info!(
        "overlay staging tree complete: dirs={}, files={}, symlinks={}, whiteouts={}, opaque={}, bytes={}, destination_mount={}",
        stats.directories,
        stats.files,
        stats.symlinks,
        stats.special_entries,
        stats.opaque_directories,
        stats.bytes,
        describe_path_mount(storage_root)
    );

    for op in &mut plan.overlay_ops {
        for lowerdir in &mut op.lowerdirs {
            *lowerdir = staged_overlay_path(lowerdir, modules, storage_root)?;
        }
    }
    for sources in plan.overlay_files.values_mut() {
        for source in sources {
            *source = staged_overlay_path(source, modules, storage_root)?;
        }
    }
    for sources in execution_plan.1.values_mut() {
        for source in sources {
            source.source = staged_overlay_path(&source.source, modules, storage_root)?;
        }
    }

    let normalized_layers = normalize_direct_overlay_layer_metadata(plan)?;

    log::info!(
        "overlay staging complete: root={}, operations={}, shallow_targets={}, normalized_layer_roots={}",
        storage_root.display(),
        plan.overlay_ops.len(),
        plan.overlay_files.len(),
        normalized_layers
    );
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
type MountExecutionResult = (
    usize,
    usize,
    Vec<String>,
    exec::MagicMountStats,
    crate::mount::vfs::exec::VfsExecStats,
);

#[cfg(any(target_os = "linux", target_os = "android"))]
fn execute_mount_phases(
    config: &Config,
    modules: &[ModuleRecord],
    plan: &mut MountPlan,
    mount_source: &str,
    state: &mut RunState,
    transaction: &mut crate::platform::sys::transaction::MountTransaction<'_>,
    mounted: &mut MountedTargets,
) -> Result<MountExecutionResult> {
    let needs_overlay_storage = !plan.overlay_ops.is_empty() || !plan.overlay_files.is_empty();
    let needs_runtime_temp = needs_overlay_storage || !plan.magic_module_ids.is_empty();
    let mut overlay_execution_plan = build_overlay_execution_plan(plan)?;
    let storage_sizing_paths = overlay_storage_sizing_paths(modules, plan, &overlay_execution_plan);
    let shallow_copy_count = overlay_execution_plan
        .1
        .values()
        .map(Vec::len)
        .sum::<usize>();
    log::info!(
        "overlay storage sizing inputs: overlay_module_roots={}, shallow_copy_roots={}, total_paths={}",
        plan.overlay_module_ids.len(),
        shallow_copy_count,
        storage_sizing_paths.len()
    );

    let mut runtime_temp = needs_runtime_temp
        .then(crate::platform::sys::temp::RuntimeTempDir::create)
        .transpose()?;
    let transient_root = runtime_temp
        .as_ref()
        .map(|session| session.path().to_path_buf());
    let retain_runtime = config.disable_umount && needs_overlay_storage;
    if retain_runtime && let Some(session) = runtime_temp.as_mut() {
        session.keep();
    }
    if let Some(session) = runtime_temp.take() {
        if retain_runtime {
            transaction
                .register_retainable("runtime_temp", move || session.cleanup_unconditional());
        } else {
            transaction.register("runtime_temp", move || session.cleanup_unconditional());
        }
    }

    let storage_phase = PhaseTimer::start("storage");
    let storage_root = if needs_overlay_storage {
        let session_root = transient_root
            .as_deref()
            .ok_or_else(|| Error::msg("overlay storage requires a runtime temporary session"))?;
        let mount_base = crate::platform::sys::temp::create_random_dir(session_root)?;
        let force_ext4 = matches!(config.overlay_mode, OverlayMode::Ext4);
        let handle = crate::mount::storage::setup(
            &mount_base,
            &storage_sizing_paths,
            force_ext4,
            mount_source,
            config.disable_umount,
        )
        .map_err(|err| {
            Error::msg(format!(
                "initialize overlay storage: requested_mode={}, mount_point={}: {err}",
                config.overlay_mode.as_str(),
                mount_base.display()
            ))
        })?;
        let storage_mode = handle.mode().as_str().to_owned();
        let storage_root = handle.mount_point().to_path_buf();
        transaction.register_retainable("overlay_storage", move || {
            crate::mount::storage::teardown(&handle)
        });
        state.storage_mode = storage_mode;
        state.mount_point = storage_root.clone();
        state.save()?;
        log::info!(
            "storage state saved: requested_mode={}, actual_mode={}, mount_point={}",
            config.overlay_mode.as_str(),
            state.storage_mode,
            state.mount_point.display()
        );
        prepare_overlay_storage(modules, plan, &mut overlay_execution_plan, &storage_root)?;
        Some(storage_root)
    } else {
        // No overlay staging was created, so TMPFS/EXT4 never started. Record the
        // explicit sentinel instead of leaving the string empty: the module description
        // and the WebUI must not claim a storage backend that never ran (VFS-only boot).
        log::info!("overlay storage skipped: reason=no_overlay_operations");
        state.storage_mode = defs::NO_STORAGE_MODE.to_owned();
        None
    };
    storage_phase.finish();

    let magic_work_dir = if plan.magic_module_ids.is_empty() {
        log::info!("magic staging skipped: reason=no_magic_modules");
        None
    } else {
        let session_root = transient_root
            .as_deref()
            .ok_or_else(|| Error::msg("magic mount requires a runtime temporary session"))?;
        let staging_path = crate::platform::sys::temp::create_random_dir(session_root)?;
        let staging = MagicStagingGuard::mount(staging_path.clone(), mount_source)?;
        let staging_path = staging.path().to_path_buf();
        transaction.register("magic_staging", move || staging.cleanup());
        Some(crate::platform::sys::temp::create_random_dir(
            &staging_path,
        )?)
    };

    let overlay_phase = PhaseTimer::start("overlay");
    let (overlay_dir_mounts, shallow_overlay_mounts, active_mounts) = mount_overlay_phase(
        OverlayPlans {
            mount: plan,
            execution: &overlay_execution_plan,
        },
        config,
        storage_root.as_deref(),
        transient_root.as_deref(),
        mount_source,
        transaction,
        mounted,
    )?;
    overlay_phase.finish();

    let magic_phase = PhaseTimer::start("magic");
    let magic_stats = mount_magic_phase(
        config,
        plan,
        mount_source,
        magic_work_dir.as_deref(),
        transaction,
        mounted,
    )?;
    magic_phase.finish();

    let vfs_phase = PhaseTimer::start("vfs");
    let use_nomount = state.vfs_provider.as_deref() == Some("nomount");
    let vfs_stats = apply_vfs_phase(config, plan, state, use_nomount, transaction)?;
    vfs_phase.finish();

    Ok((
        overlay_dir_mounts,
        shallow_overlay_mounts,
        active_mounts,
        magic_stats,
        vfs_stats,
    ))
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn mountinfo_mismatches(
    baseline: &crate::platform::sys::mountinfo::MountSnapshot,
    mounted: &MountedTargets,
) -> Result<(Vec<String>, Vec<String>)> {
    let current = crate::platform::sys::mountinfo::MountSnapshot::read()?;
    let mut leftover = Vec::new();
    let mut missing = Vec::new();

    for target in &mounted.paths {
        let root = Path::new(target);
        let before = baseline.subtree_ids(root);
        let after = current.subtree_ids(root);

        for (path, ids) in &after {
            if before.get(path) != Some(ids) {
                leftover.push(path.display().to_string());
            }
        }
        for path in before.keys() {
            if !after.contains_key(path) {
                missing.push(path.display().to_string());
            }
        }
    }

    leftover.sort();
    leftover.dedup();
    missing.sort();
    missing.dedup();
    Ok((leftover, missing))
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[derive(Debug, Default)]
struct RollbackSummary {
    status: String,
    leftover_targets: Vec<String>,
}

#[cfg(any(target_os = "linux", target_os = "android"))]
impl RollbackSummary {
    fn clean() -> Self {
        Self {
            status: "clean".to_owned(),
            leftover_targets: Vec::new(),
        }
    }

    fn unverified() -> Self {
        Self {
            status: "unverified".to_owned(),
            leftover_targets: Vec::new(),
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn rollback_mount_pipeline(
    transaction: crate::platform::sys::transaction::MountTransaction<'_>,
    mounted: &MountedTargets,
    baseline: &crate::platform::sys::mountinfo::MountSnapshot,
) -> RollbackSummary {
    let rollback_phase = PhaseTimer::start("rollback");
    let report = transaction.rollback();
    for failure in &report.failures {
        log::error!(
            "rollback action failed: action={}, error={}",
            failure.label,
            failure.error
        );
    }

    let summary = match mountinfo_mismatches(baseline, mounted) {
        Ok((leftover, missing)) => {
            for target in &leftover {
                log::error!("rollback left an uncleaned mount target: {target}");
            }
            for target in &missing {
                log::error!("rollback removed a pre-existing mount target: {target}");
            }
            if report.failures.is_empty() && leftover.is_empty() && missing.is_empty() {
                RollbackSummary::clean()
            } else {
                RollbackSummary {
                    status: "incomplete".to_owned(),
                    leftover_targets: leftover,
                }
            }
        }
        Err(err) => {
            log::error!("rollback verification unavailable: {err}");
            RollbackSummary::unverified()
        }
    };
    rollback_phase.finish();
    summary
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn persist_mount_failure_state(
    state: &mut RunState,
    failed_stage: &str,
    failure_reason: &str,
    rollback: &RollbackSummary,
) {
    state.mount_point = PathBuf::new();
    state.active_mounts.clear();
    state.overlay_active_mounts.clear();
    state.magic_active_mounts.clear();
    state.vfs_active_mounts.clear();
    state.confirmed_active_mounts.clear();
    state.mount_stats = MountStatistics {
        total_mounts: 1,
        failed_mounts: 1,
        ..MountStatistics::default()
    };
    state.failed_stage = Some(failed_stage.to_owned());
    state.failure_reason = Some(failure_reason.to_owned());
    state.rollback_status = Some(rollback.status.clone());
    state.leftover_mount_targets = rollback.leftover_targets.clone();
    if let Err(state_err) = state.save() {
        log::warn!("failed to persist mount failure statistics: {state_err}");
    }
}

/// After a rollback `scan.ret` must report everything as unmounted, or the WebUI would
/// keep showing mounts that succeeded before the rollback.
#[cfg(any(target_os = "linux", target_os = "android"))]
fn persist_unmounted_module_snapshot(modules: &[ModuleRecord], config: &Config, plan: &MountPlan) {
    let mount_errors = crate::framework::state::collect_mount_error_modules(&config.moduledir);
    let snapshot = app_modules(modules, config, plan, &mount_errors, &BTreeSet::new());
    if let Err(err) = write_scan_ret(&snapshot) {
        log::error!("failed to restore unmounted module snapshot: {err}");
    }
}

/// Records a boot failure after the mounts have been rolled back, then hands the error back to the
/// caller.
///
/// Every failure site in the mount pipeline persists the same three things in the same order, so
/// the summary the WebUI reads can never drift away from the state file or the module snapshot.
#[cfg(any(target_os = "linux", target_os = "android"))]
fn record_mount_failure(
    state: &mut RunState,
    modules: &[ModuleRecord],
    config: &Config,
    plan: &MountPlan,
    stage: &str,
    err: Error,
    rollback: &RollbackSummary,
) -> Error {
    persist_mount_failure_state(state, stage, &err.to_string(), rollback);
    persist_unmounted_module_snapshot(modules, config, plan);
    err
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn confirmed_mount_targets(
    targets: &[String],
    snapshot: &crate::platform::sys::mountinfo::MountSnapshot,
) -> Vec<String> {
    targets
        .iter()
        .filter(|target| snapshot.contains(Path::new(target)))
        .cloned()
        .collect()
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn log_phase_failure<T>(phase: &'static str, result: Result<T>) -> Result<T> {
    if let Err(err) = &result {
        log::error!(
            "phase={phase} failed class={}: {err}",
            err.classify().label()
        );
    }
    result
}

#[cfg(any(target_os = "linux", target_os = "android", test))]
fn persist_startup_failure_state_to(
    stage: &str,
    error: &Error,
    state_path: &Path,
    scan_ret_path: &Path,
) {
    let state = RunState::from_startup_failure(stage, error.to_string());
    if let Err(state_err) = state.save_to(state_path) {
        log::error!("failed to persist {stage} startup failure state: {state_err}");
    }
    if let Err(snapshot_err) = crate::framework::state::write_scan_ret_to(&[], scan_ret_path) {
        log::error!("failed to clear module snapshot after {stage} failure: {snapshot_err}");
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn persist_startup_failure_state(stage: &str, error: &Error) {
    persist_startup_failure_state_to(
        stage,
        error,
        Path::new(defs::STATE_PATH),
        Path::new(defs::SCAN_RET_PATH),
    );
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn startup_phase<T>(stage: &'static str, result: Result<T>) -> Result<T> {
    match log_phase_failure(stage, result) {
        Ok(value) => Ok(value),
        Err(err) => {
            persist_startup_failure_state(stage, &err);
            Err(err)
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn run_mount_pipeline_impl(
    mounted: &mut MountedTargets,
    baseline_mounts: &[crate::platform::sys::mountinfo::MountEntry],
) -> Result<()> {
    let startup = PhaseTimer::start("startup");
    platform::ksu::init();

    // Config and scan.ret share the persisted directory; state.json lives in the run subdirectory.
    for path in [defs::CONFIG_PATH, defs::STATE_PATH] {
        if let Some(directory) = Path::new(path).parent()
            && let Err(err) = crate::platform::sys::fs::cleanup_stale_atomic_temp_files(directory)
        {
            log::warn!("failed to cleanup stale temp files: {err}");
        }
    }

    startup.finish();

    let config_phase = PhaseTimer::start("config");
    let config = startup_phase(
        "config",
        Config::load_for_boot(Path::new(defs::CONFIG_PATH)),
    )?;
    let ksu_active = platform::ksu::is_active();
    let mount_source = effective_mount_source(ksu_active);
    log::info!(
        "config info: {}",
        startup_phase("config", config.to_toml())?
    );
    log::info!(
        "runtime: pid={}, effective_mount_source={}, ksu_ioctl_active={}",
        std::process::id(),
        mount_source,
        ksu_active
    );
    log_backend_capabilities();
    config_phase.finish();

    let scan_phase = PhaseTimer::start("scan");
    let layout = startup_phase(
        "partition_layout",
        crate::framework::scan::partitions::detect_with_mounts(&config, baseline_mounts),
    )?;
    let managed_partitions = layout.scan_names();
    let extensions = startup_phase(
        "lua",
        Extensions::load(Path::new(defs::LUA_DIR), &config.moduledir),
    )?;
    let before = startup_phase("lua_before", extensions.commands(HookPhase::Before))?;
    let modules = startup_phase(
        "scan",
        list_modules_with(&config.moduledir, &managed_partitions, |id, enabled| {
            if !enabled || config.is_module_blacklisted(id.as_str()) {
                Ok(Default::default())
            } else {
                extensions.module_edits(id, &layout)
            }
        }),
    )?;
    log::info!("scanned modules: {}", modules.len());
    for module in &modules {
        let entry_roots = module
            .entries
            .iter()
            .filter_map(|entry| entry.relative.split('/').next())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
            .join(",");
        log::debug!(
            "module scan: id={}, enabled={}, skip_mount={}, mountable={}, entries={}, roots={}, source={}, source_mount={}",
            module.id,
            !module.disabled,
            module.skip_mount,
            module.mountable(),
            module.entries.len(),
            entry_roots,
            module.source_path.display(),
            describe_path_mount(&module.source_path)
        );
    }
    scan_phase.finish();

    // Detect a foreign provider before considering insmod. This has to finish before planning:
    // a plan built while the module is unloaded carries no VFS work, while loading next to a
    // foreign provider would let two inode-hooking implementations coexist.
    let nomount_probe = crate::mount::vfs::guard::detect_foreign_nomount();
    let wants_vfs = config.wants_vfs()
        || before.iter().any(|mount| mount.kind == MountKind::Vfs)
        || modules.iter().any(|module| {
            module
                .edits
                .modes
                .values()
                .any(|mode| *mode == crate::mount::model::Mode::Vfs)
        });
    let vfs_available = if nomount_probe.present {
        crate::mount::vfs::available()
    } else if nomount_probe.blocked() {
        crate::mount::vfs::ensure_loaded_for_plan_with_guard(
            wants_vfs,
            true,
            crate::mount::vfs::available,
            crate::mount::vfs::lkm::load_hm_vfs,
        )
    } else {
        crate::mount::vfs::ensure_loaded_for_plan(
            wants_vfs,
            crate::mount::vfs::available,
            crate::mount::vfs::lkm::load_hm_vfs,
        )
    };
    log::info!(
        "vfs provider probe: wants_vfs={}, available={}, foreign_nomount={}, strict={}",
        wants_vfs,
        vfs_available,
        nomount_probe.present,
        config.vfs_strict
    );

    // `vfs_strict` promises that an unavailable VFS fails the boot, and this is the only point
    // where that is still decidable: the plan below rewrites every `vfs` rule to `ignore`, so
    // `apply_vfs_phase` sees an empty module set and returns before its own strict checks. Without
    // this the option would be silently ineffective for exactly the case it exists to catch.
    if crate::mount::vfs::unavailable_is_fatal(wants_vfs, config.vfs_strict, vfs_available) {
        let reason = if nomount_probe.present {
            "NoMount is present but no compatible, conflict-free VFS provider is available"
                .to_owned()
        } else if let Some(error) = &nomount_probe.error {
            format!("VFS provider guard failed closed: {error}")
        } else {
            "no supported VFS kernel provider and vfs_strict is enabled".to_owned()
        };
        return startup_phase("vfs", Err(Error::VfsUnavailable { reason }));
    }

    let plan_phase = PhaseTimer::start("plan");
    let mut plan = startup_phase(
        "plan",
        build_plan(&PlanInput {
            modules: &modules,
            config: &config,
            layout: &layout,
            vfs_available,
        }),
    )?;
    for error in &plan.partition_errors {
        log::error!(
            "partition skipped: {}: {}; modules={:?}",
            error.partition,
            error.reason,
            error.modules
        );
    }
    for error in &plan.operation_errors {
        log::error!(
            "operation skipped: {}:{}: {}",
            error.module_id,
            error.logical_path,
            error.reason
        );
    }
    log::info!(
        "plan: overlay_ops={}, overlay_modules={}, magic_modules={}, vfs_modules={}",
        plan.overlay_ops.len(),
        plan.overlay_module_ids.len(),
        plan.magic_module_ids.len(),
        plan.vfs_module_ids.len()
    );
    log::info!(
        "plan metrics: modules={}, nodes={}, overlay_ops={}, shallow_targets={}, overlay_modules={}, magic_modules={}, vfs_modules={}",
        modules.len(),
        plan.tree.node_count(),
        plan.overlay_ops.len(),
        plan.overlay_files.len(),
        plan.overlay_module_ids.len(),
        plan.magic_module_ids.len(),
        plan.vfs_module_ids.len()
    );
    for (index, op) in plan.overlay_ops.iter().enumerate() {
        log::debug!(
            "plan overlay operation: index={}, partition={}, target={}, layers={}",
            index,
            op.partition,
            op.target,
            op.lowerdirs.len()
        );
        for (layer_index, lowerdir) in op.lowerdirs.iter().enumerate() {
            log::debug!(
                "plan overlay lowerdir: operation={}, layer={}, path={}, exists={}, mount={}",
                index,
                layer_index,
                lowerdir.display(),
                lowerdir.exists(),
                describe_path_mount(lowerdir)
            );
        }
    }
    plan_phase.finish();
    crate::framework::runtime::boot::stage_plan(
        &plan,
        &config.vfs_isolate_uids,
        nomount_probe.present,
        &modules,
    )?;

    // `modules` is a boot-time snapshot, not a proof that every mount already
    // succeeded.  Persist it before entering the fallible mount phases so the
    // WebUI can still show the scanned modules and their planned modes when a
    // device rejects one overlay operation.
    let state_phase = PhaseTimer::start("state");
    let initial_mount_errors =
        crate::framework::state::collect_mount_error_modules(&config.moduledir);
    let initial_app_modules = app_modules(
        &modules,
        &config,
        &plan,
        &initial_mount_errors,
        &BTreeSet::new(),
    );
    startup_phase("state", write_scan_ret(&initial_app_modules))?;
    log::info!(
        "module snapshot saved: modules={}",
        initial_app_modules.len()
    );

    // State is a boot snapshot, not a daemon-owned live feed.  Save the plan
    // before any fallible mount operation so `status` and the WebUI can still
    // report the selected backends when the device rejects a later mount.
    let mut state = RunState::from_plan(&config, &modules, &plan, initial_mount_errors);
    if nomount_probe.present && vfs_available {
        state.vfs_provider = Some("nomount".into());
    }
    if wants_vfs && !vfs_available {
        state.vfs_error = Some(nomount_probe.error.as_ref().map_or_else(
            || "VFS provider unavailable; configured VFS rules were skipped".to_owned(),
            |error| format!("VFS provider guard failed closed: {error}"),
        ));
        state.vfs_error_modules = plan.vfs_module_id_strings();
    }
    startup_phase("state", state.save())?;
    state_phase.finish();
    log::info!(
        "planned state saved: overlay_modules={}, magic_modules={}",
        state.overlay_modules.len(),
        state.magic_modules.len()
    );

    let baseline = startup_phase(
        "baseline",
        crate::platform::sys::mountinfo::MountSnapshot::read(),
    )?;
    let mut transaction = crate::platform::sys::transaction::MountTransaction::new();
    transaction.register_rollback_only("ksu_try_umount_list", || {
        crate::platform::ksu::clear_unmount_list()
    });
    let mut hooks = custom::HookEffects {
        binds: Vec::new(),
        overlays: Vec::new(),
        vfs: Vec::new(),
        mount_source,
        provider: vfs_available.then_some(if nomount_probe.present {
            "nomount"
        } else {
            "hm"
        }),
    };
    let outcome = (|| {
        hooks.apply(before, &config, &mut transaction, mounted)?;
        let result = execute_mount_phases(
            &config,
            &modules,
            &mut plan,
            mount_source,
            &mut state,
            &mut transaction,
            mounted,
        )?;
        hooks.apply(
            extensions.commands(HookPhase::After)?,
            &config,
            &mut transaction,
            mounted,
        )?;
        crate::platform::ksu::commit_unmount_list()?;
        Ok(result)
    })();
    let (
        overlay_dir_mounts,
        shallow_overlay_mounts,
        mut active_mounts,
        mut magic_stats,
        mut vfs_stats,
    ) = match outcome {
        Ok(result) => result,
        Err(err) => {
            log::error!(
                "mount execution failed: phase=mount_execution, error={err}, overlay_modules={}, magic_modules={}",
                plan.overlay_module_ids.join(","),
                plan.magic_module_ids.join(",")
            );
            let rollback = rollback_mount_pipeline(transaction, mounted, &baseline);
            return Err(record_mount_failure(
                &mut state,
                &modules,
                &config,
                &plan,
                "mount_execution",
                err,
                &rollback,
            ));
        }
    };

    // `is_mounted` comes from mountinfo-confirmed targets, never from the plan.
    // Executor attempt counts stay in `mount_stats` and are never copied into
    // `active_mounts` as proof of still being mounted.
    let final_mountinfo = match crate::platform::sys::mountinfo::MountSnapshot::read() {
        Ok(snapshot) => snapshot,
        Err(err) => {
            log::error!("phase=mountinfo_confirm failed: {err}");
            let rollback = rollback_mount_pipeline(transaction, mounted, &baseline);
            return Err(record_mount_failure(
                &mut state,
                &modules,
                &config,
                &plan,
                "mountinfo_confirm",
                err,
                &rollback,
            ));
        }
    };
    active_mounts.extend(hooks.overlays.iter().cloned());
    magic_stats
        .active_mounts
        .extend(hooks.binds.iter().cloned());
    if !hooks.vfs.is_empty() {
        vfs_stats.active_targets.extend(hooks.vfs);
        vfs_stats.active_targets.sort();
        vfs_stats.active_targets.dedup();
        state.vfs_provider = hooks.provider.map(str::to_owned);
    }
    let confirmed_overlay_targets = confirmed_mount_targets(&active_mounts, &final_mountinfo);
    let confirmed_magic_targets =
        confirmed_mount_targets(&magic_stats.active_mounts, &final_mountinfo);
    // VFS injection points are not kernel mounts, so mountinfo can never confirm them.
    // They are confirmed by the provider read-back in `apply_vfs_phase`, which drops a
    // batch whose rules did not survive, so the stats targets are already trustworthy.
    let confirmed_vfs_targets = vfs_stats.active_targets.clone();
    let confirmed_active_mounts = merge_active_mounts(
        &confirmed_overlay_targets,
        &confirmed_magic_targets,
        &confirmed_vfs_targets,
    );
    let attempted_active_mounts = merge_active_mounts(
        &active_mounts,
        &magic_stats.active_mounts,
        &vfs_stats.active_targets,
    );
    if confirmed_active_mounts != attempted_active_mounts {
        log::warn!(
            "executor targets not fully confirmed by mountinfo: executed={}, confirmed={}",
            attempted_active_mounts.join(","),
            confirmed_active_mounts.join(",")
        );
    }

    let mount_error_modules =
        crate::framework::state::collect_mount_error_modules(&config.moduledir);
    let mut mounted_module_ids = mounted_module_ids_for_snapshot(
        &modules,
        &plan,
        &confirmed_overlay_targets,
        &confirmed_magic_targets,
    );
    // Symlink/whiteout-only magic modules have successful execution results
    // but no mount target; include them from the executor stats.
    mounted_module_ids.extend(magic_stats.mounted_module_ids.iter().cloned());
    mounted_module_ids.extend(vfs_stats.mounted_module_ids.iter().cloned());
    let app_modules = app_modules(
        &modules,
        &config,
        &plan,
        &mount_error_modules,
        &mounted_module_ids,
    );
    if let Err(err) = write_scan_ret(&app_modules) {
        log::error!("phase=module_snapshot_save failed, rolling back mounts: {err}");
        let rollback = rollback_mount_pipeline(transaction, mounted, &baseline);
        return Err(record_mount_failure(
            &mut state,
            &modules,
            &config,
            &plan,
            "module_snapshot_save",
            err,
            &rollback,
        ));
    }

    let mount_error_reasons = mount_error_modules
        .iter()
        .map(|module| (module.clone(), crate::defs::MOUNT_ERROR_REASON.to_owned()))
        .collect();

    let state_phase = PhaseTimer::start("state");
    state.active_mounts = confirmed_active_mounts.clone();
    state.overlay_active_mounts = confirmed_overlay_targets;
    state.magic_active_mounts = confirmed_magic_targets;
    state.vfs_active_mounts = vfs_stats.active_targets.clone();
    state.confirmed_active_mounts = confirmed_active_mounts;
    state.mount_stats = pipeline_stats(
        overlay_dir_mounts + hooks.overlays.len(),
        shallow_overlay_mounts,
        magic_stats.mounted_files as usize,
        magic_stats.mounted_symlinks as usize,
        magic_stats.ignored_files as usize,
        magic_stats.mounted_dirs as usize,
    );
    state.mount_stats.total_mounts += hooks.binds.len();
    state.mount_stats.successful_mounts += hooks.binds.len();
    state.mode_stats.vfs = vfs_stats.mounted_module_ids.len();
    if let Some(failure) = &vfs_stats.failure {
        state.vfs_error = Some(failure.clone());
        state.vfs_error_modules = plan.vfs_module_id_strings();
    }
    state.mount_error_modules = mount_error_modules;
    state.mount_error_reasons = mount_error_reasons;
    state.failed_stage = None;
    state.rollback_status = Some("pending_commit".to_owned());
    state.leftover_mount_targets.clear();
    if !config.disable_umount {
        state.mount_point = PathBuf::new();
    }
    if let Err(err) = state.save() {
        log::error!("phase=state_save failed, rolling back mounts: {err}");
        let rollback = rollback_mount_pipeline(transaction, mounted, &baseline);
        return Err(record_mount_failure(
            &mut state,
            &modules,
            &config,
            &plan,
            "state_save",
            err,
            &rollback,
        ));
    }
    state_phase.finish();

    let cleanup_phase = PhaseTimer::start("cleanup");
    if let Err(err) = transaction.commit(config.disable_umount) {
        cleanup_phase.abort();
        log::error!("phase=mount_transaction_commit failed: {err}");
        let rollback = match mountinfo_mismatches(&baseline, mounted) {
            Ok((leftover, missing)) => {
                for target in &leftover {
                    log::error!("post-commit leftover mount target: {target}");
                }
                for target in &missing {
                    log::error!("post-commit missing pre-existing mount target: {target}");
                }
                RollbackSummary {
                    status: "incomplete".to_owned(),
                    leftover_targets: leftover,
                }
            }
            Err(verify_err) => {
                log::error!("post-commit rollback verification unavailable: {verify_err}");
                RollbackSummary::unverified()
            }
        };
        return Err(record_mount_failure(
            &mut state,
            &modules,
            &config,
            &plan,
            "mount_transaction_commit",
            err,
            &rollback,
        ));
    }
    cleanup_phase.finish();

    let post_commit_state_phase = PhaseTimer::start("state");
    state.rollback_status = Some("committed".to_owned());
    state.failed_stage = None;
    state.leftover_mount_targets.clear();
    match state.save() {
        Ok(()) => {
            post_commit_state_phase.finish();
        }
        Err(err) => {
            log::error!("post-commit state save failed, mounts remain active: {err}");
            post_commit_state_phase.abort();
        }
    }

    crate::framework::state::description::update_description(
        &state.storage_mode,
        plan.overlay_module_ids.len(),
        plan.magic_module_ids.len(),
        vfs_stats.mounted_module_ids.len(),
    );

    log::info!("mount pipeline completed");
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn prepare_tmp_root(tmp_root: &Path, mount_source: &str) -> Result<()> {
    log::info!(
        "magic staging mount start: source={}, target={}, target_before={}",
        mount_source,
        tmp_root.display(),
        describe_path_mount(tmp_root)
    );
    platform::ensure_dir_exists(tmp_root)?;
    mount(mount_source, tmp_root, "tmpfs", MountFlags::empty(), None).map_err(|err| {
        Error::msg(format!(
            "mount tmpfs {mount_source} at {}: {err}",
            tmp_root.display()
        ))
    })?;
    if let Err(err) = mount_change(
        tmp_root,
        MountPropagationFlags::PRIVATE | MountPropagationFlags::REC,
    ) {
        let _ = unmount(tmp_root, UnmountFlags::DETACH);
        return Err(Error::msg(format!(
            "make magic staging private at {}: {err}",
            tmp_root.display()
        )));
    }
    log::info!(
        "magic staging mount complete: target={}, target_after={}",
        tmp_root.display(),
        describe_path_mount(tmp_root)
    );
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn cleanup_tmp_root(tmp_root: &Path) -> Result<()> {
    if crate::platform::sys::mount::is_mounted(tmp_root)? {
        unmount(tmp_root, UnmountFlags::DETACH).map_err(|err| {
            Error::msg(format!(
                "detach magic staging mount {}: {err}",
                tmp_root.display()
            ))
        })?;
        log::info!("magic staging unmounted: target={}", tmp_root.display());
    }
    if crate::platform::sys::mount::is_mounted(tmp_root)? {
        return Err(Error::msg(format!(
            "magic staging mount still mounted after detach: target={}",
            tmp_root.display()
        )));
    }
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn cleanup_tmp_root_best_effort(tmp_root: &Path) -> Result<()> {
    if crate::platform::sys::mount::is_mounted_best_effort(tmp_root) {
        unmount(tmp_root, UnmountFlags::DETACH).map_err(|err| {
            Error::msg(format!(
                "detach magic staging mount {}: {err}",
                tmp_root.display()
            ))
        })?;
        log::info!("magic staging unmounted: target={}", tmp_root.display());
    }
    Ok(())
}

/// Size the initial prepared tree once per participating module and every
/// shallow source once more. Duplicate paths are intentional because each one
/// represents another materialization inside the same staging filesystem.
#[cfg(unix)]
fn overlay_storage_sizing_paths(
    modules: &[ModuleRecord],
    plan: &MountPlan,
    execution_plan: &OverlayExecutionPlan,
) -> Vec<PathBuf> {
    let selected: Vec<_> = modules
        .iter()
        .filter(|module| plan.overlay_module_ids.contains(&module.id))
        .collect();
    let mut paths: Vec<_> = selected
        .iter()
        .map(|module| module.source_path.clone())
        .collect();
    let shallow: Vec<_> = execution_plan
        .1
        .values()
        .flatten()
        .map(|source| &source.source)
        .collect();
    paths.extend(shallow.iter().map(|source| (**source).clone()));
    for module in selected {
        for (logical, source) in &module.edits.replacements {
            if module.edits.excludes(logical) {
                continue;
            }
            let Ok((_, target)) = plan.partition_layout.map_target(&logical.to_string_lossy())
            else {
                continue;
            };
            if !plan
                .tree
                .find(&target.to_string_lossy())
                .is_some_and(|node| node.has_backend(crate::mount::model::Mode::Overlay))
            {
                continue;
            }
            paths.push(source.clone());
            let layer = module.source_path.join(logical);
            for staged in &shallow {
                if let Ok(suffix) = staged.strip_prefix(&layer) {
                    paths.push(if suffix.as_os_str().is_empty() {
                        source.clone()
                    } else {
                        source.join(suffix)
                    });
                } else if layer.starts_with(staged) {
                    paths.push(source.clone());
                }
            }
        }
    }
    paths
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn mount_overlay_phase(
    plans: OverlayPlans<'_>,
    config: &Config,
    storage_root: Option<&Path>,
    transient_root: Option<&Path>,
    effective_mount_source: &str,
    transaction: &mut crate::platform::sys::transaction::MountTransaction<'_>,
    mounted: &mut MountedTargets,
) -> Result<(usize, usize, Vec<String>)> {
    use crate::mount::overlayfs::exec::MountEffect;

    mount_layers(
        plans,
        !config.disable_umount,
        storage_root,
        transient_root,
        effective_mount_source,
        &mut |effect: MountEffect| match effect {
            MountEffect::Target(target) => register_mounted_target(transaction, mounted, &target),
            MountEffect::Staging(path) => transaction
                .register("intermediate_overlay_staging", move || {
                    crate::mount::overlayfs::exec::cleanup_staging_mount(path)
                }),
        },
    )
}

/// Rollback closure: deletes the full batch registered for this run.
///
/// Registering before applying means an applied prefix is undone too; rules that never
/// took effect are tolerated as ENOENT. Uses DEL_RULE rather than CLEAR_RULES so it
/// cannot remove rules from other sources.
#[cfg(any(target_os = "linux", target_os = "android"))]
fn rollback_vfs_rules(
    shared: Rc<RefCell<KeyringKernel>>,
    applied: Rc<RefCell<VfsApplied>>,
) -> impl FnOnce() -> Result<()> {
    move || {
        let rules = std::mem::take(&mut applied.borrow_mut().rules);
        let mut kernel = shared.try_borrow_mut().map_err(|err| {
            log::error!("rollback vfs rules failed to borrow kernel: {err}");
            Error::Vfs(Box::new(crate::errors::ContextError::new(
                "rollback vfs rules",
                None,
                crate::errors::CausalError::Message(format!("vfs kernel borrow failed: {err}")),
            )))
        })?;
        if rules.is_empty() {
            return Ok(());
        }
        kernel.remove_rules(&rules)
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn vfs_failure_stats(detail: impl Into<String>) -> VfsExecStats {
    VfsExecStats {
        failure: Some(detail.into()),
        ..VfsExecStats::default()
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn record_vfs_failure(state: &mut RunState, plan: &MountPlan, detail: impl Into<String>) {
    state.vfs_provider = None;
    state.vfs_error = Some(detail.into());
    state.vfs_error_modules = plan.vfs_module_id_strings();
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn apply_vfs_phase(
    config: &Config,
    plan: &MountPlan,
    state: &mut RunState,
    use_nomount: bool,
    transaction: &mut crate::platform::sys::transaction::MountTransaction<'_>,
) -> Result<VfsExecStats> {
    if plan.vfs_module_ids.is_empty() {
        log::info!("vfs phase skipped: reason=no_vfs_modules");
        return Ok(VfsExecStats::default());
    }
    log::info!(
        "vfs phase start: modules={}, module_ids={}, isolate_uids={}, strict={}",
        plan.vfs_module_ids.len(),
        plan.vfs_module_ids
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(","),
        config.vfs_isolate_uids.len(),
        config.vfs_strict
    );
    match crate::mount::vfs::boot_guard::inspect() {
        Ok(crate::mount::vfs::boot_guard::VfsGuardVerdict::Absent) => {}
        // The marker speaks for this build, so the interrupted mutation is not retried.
        Ok(crate::mount::vfs::boot_guard::VfsGuardVerdict::Own(record)) => {
            let detail = format!(
                "VFS boot guard present; VFS backend skipped this boot ({}); run 'hybrid-mount vfs guard clear --yes' after resolving the crash to retry",
                record.describe()
            );
            record_vfs_failure(state, plan, detail.clone());
            log::warn!("{detail}");
            return Ok(vfs_failure_stats(detail));
        }
        // A marker of another build, or the legacy marker without identity, cannot describe a
        // crash this build would repeat: it is retired and the backend is tried again.
        Ok(crate::mount::vfs::boot_guard::VfsGuardVerdict::Foreign { record }) => {
            log::warn!(
                "retiring a VFS boot guard this build cannot attribute to itself: {}",
                crate::mount::vfs::boot_guard::describe(record.as_ref())
            );
            if let Err(err) = crate::mount::vfs::boot_guard::clear_rule_guard() {
                // A metamodule script that exits non-zero stops every module from mounting, so an
                // unremovable stale marker degrades to skipping the backend instead of failing.
                let detail = format!("VFS boot guard could not be retired: {err}");
                record_vfs_failure(state, plan, detail.clone());
                log::warn!("{detail}");
                return Ok(vfs_failure_stats(detail));
            }
        }
        Err(err) => {
            record_vfs_failure(state, plan, err.to_string());
            log::warn!("vfs boot guard could not be read: {err}");
            return Ok(vfs_failure_stats(err.to_string()));
        }
    }
    // Any handled return past this point clears the guard on Drop.
    let _guard = match crate::mount::vfs::boot_guard::VfsBootGuard::arm(
        crate::mount::vfs::boot_guard::VfsMutationSource::Boot,
    ) {
        Ok(guard) => guard,
        Err(err) => {
            record_vfs_failure(state, plan, err.to_string());
            return Err(err);
        }
    };

    let channel = if use_nomount {
        KeyringChannel::Nomount
    } else {
        KeyringChannel::Hybridmount
    };
    let attached = crate::mount::vfs::backend::open_active().and_then(|kernel| {
        if kernel.channel() != channel {
            return Err(Error::msg("VFS provider changed after planning"));
        }
        Ok(kernel)
    });
    let mut kernel = match attached {
        Ok(kernel) => kernel,
        Err(err) => {
            let detail = err.to_string();
            record_vfs_failure(state, plan, &detail);
            if config.vfs_strict {
                return Err(err);
            }
            return Ok(vfs_failure_stats(detail));
        }
    };
    let provider = kernel.provider();

    // Build the full batch and register it for rollback before applying: the kernel
    // applies non-atomically, so a mid-batch failure leaves an applied prefix to undo.
    // Rollback is always targeted, never a CLEAR_RULES of the whole provider table.
    let planned = match crate::mount::vfs::exec::plan_rules(plan) {
        Ok(planned) => planned,
        Err(err) => {
            record_vfs_failure(state, plan, err.to_string());
            return Err(err);
        }
    };
    if use_nomount {
        let preflight = (|| {
            crate::mount::vfs::protocol::build_add_rule_payloads_for(
                &planned.rules,
                0,
                kernel.format(),
            )?;
            let existing = kernel.list_rules()?;
            if let Some(rule) = existing.iter().find(|rule| {
                rule.scope == 0.into()
                    && !crate::mount::vfs::protocol::is_derived_parent(rule.flags, &rule.real_path)
                    && planned
                        .rules
                        .iter()
                        .any(|planned| planned.virtual_path == rule.virtual_path.as_bytes())
            }) {
                return Err(Error::msg(format!(
                    "NoMount already has a rule for {}",
                    rule.virtual_path
                )));
            }
            Ok(())
        })();
        if let Err(err) = preflight {
            let detail = err.to_string();
            record_vfs_failure(state, plan, &detail);
            if config.vfs_strict {
                return Err(err);
            }
            return Ok(vfs_failure_stats(detail));
        }
    }
    let planned_rule_count = planned.rules.len();
    log::info!(
        "vfs batch planned: rules={}, targets={}",
        planned_rule_count,
        planned.stats.active_targets.len()
    );
    let shared = Rc::new(RefCell::new(kernel));
    let applied = Rc::new(RefCell::new(planned));
    transaction.register_rollback_only(
        "vfs_rules",
        rollback_vfs_rules(Rc::clone(&shared), Rc::clone(&applied)),
    );

    let outcome = {
        let mut kernel = shared.borrow_mut();
        let batch = applied.borrow();
        crate::mount::vfs::exec::apply_rules_with_policy_diagnosed(
            &mut *kernel,
            &batch,
            &config.vfs_isolate_uids,
            config.vfs_strict,
        )
    };

    let outcome = match outcome {
        Ok(outcome) => outcome,
        Err(err) => {
            record_vfs_failure(state, plan, err.to_string());
            return Err(err);
        }
    };
    let Some(stats) = outcome.stats else {
        // Already deleted inline; clearing avoids a duplicate DEL_RULE at rollback.
        applied.borrow_mut().rules.clear();
        let detail = format!(
            "VFS apply failed; rules were rolled back: {}",
            outcome
                .failure
                .unwrap_or_else(|| "unknown provider error".to_owned())
        );
        record_vfs_failure(state, plan, &detail);
        log::warn!(
            "vfs backend degraded: {detail} (rules={})",
            planned_rule_count
        );
        return Ok(vfs_failure_stats(detail));
    };

    state.vfs_provider = Some(provider.as_str().to_owned());
    state.vfs_error = None;
    state.vfs_error_modules.clear();
    log::info!(
        "vfs phase complete: provider={}, injected={}, whiteouts={}, opaque={}",
        provider.as_str(),
        stats.injected,
        stats.whiteouts,
        stats.opaque
    );

    // An acknowledged batch is not proof the rules are installed, so read the table back and
    // report the difference. This is the line that tells a device log whether VFS is really
    // active. A listing failure is treated as unconfirmed and the batch is removed before the
    // strict/non-strict policy is applied.
    {
        let expected = applied.borrow();
        for rule in &expected.rules {
            log::debug!(
                "vfs rule applied: flags={:#x}, target={}, source={}",
                rule.flags,
                String::from_utf8_lossy(&rule.virtual_path),
                String::from_utf8_lossy(&rule.real_path)
            );
        }
    }
    confirm_vfs_readback(
        config,
        plan,
        state,
        shared.as_ref(),
        applied.as_ref(),
        stats,
    )
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn confirm_vfs_readback(
    config: &Config,
    plan: &MountPlan,
    state: &mut RunState,
    shared: &RefCell<impl VfsKernel>,
    applied: &RefCell<VfsApplied>,
    stats: VfsExecStats,
) -> Result<VfsExecStats> {
    let planned_rule_count = applied.borrow().rules.len();
    // Release the readback borrow before an error branch borrows again for cleanup.
    let listed = shared.borrow_mut().list_rules();
    match listed {
        Ok(listed) => {
            let missing = {
                let expected = applied.borrow();
                crate::mount::vfs::exec::missing_rules(&expected.rules, &listed)
            };
            if missing.is_empty() {
                log::info!(
                    "vfs read-back confirmed: installed={}, expected={}",
                    listed.len(),
                    planned_rule_count
                );
            } else {
                log::warn!(
                    "vfs read-back found {} of {} rules missing after apply: {}",
                    missing.len(),
                    planned_rule_count,
                    missing.join(",")
                );
                let cleanup = {
                    let expected = applied.borrow();
                    shared.borrow_mut().remove_rules(&expected.rules)
                };
                if let Err(cleanup_err) = cleanup {
                    let err = Error::VfsProtocol {
                        detail: format!(
                            "vfs read-back was incomplete and targeted cleanup failed: {cleanup_err}"
                        ),
                    };
                    record_vfs_failure(state, plan, err.to_string());
                    return Err(err);
                }
                applied.borrow_mut().rules.clear();
                let confirmed =
                    match crate::mount::vfs::exec::evaluate_readback(&missing, config.vfs_strict) {
                        Ok(confirmed) => confirmed,
                        Err(err) => {
                            record_vfs_failure(state, plan, err.to_string());
                            return Err(err);
                        }
                    };
                if !confirmed {
                    let detail = format!(
                        "VFS read-back did not confirm {} rule(s): {}",
                        missing.len(),
                        missing.join(",")
                    );
                    record_vfs_failure(state, plan, &detail);
                    log::warn!(
                        "vfs read-back mismatch degraded: removed this run's rules and marked vfs inactive"
                    );
                    return Ok(vfs_failure_stats(detail));
                }
            }
        }
        Err(err) => {
            let cleanup = {
                let expected = applied.borrow();
                shared.borrow_mut().remove_rules(&expected.rules)
            };
            if let Err(cleanup_err) = cleanup {
                let cleanup_failure = Error::VfsProtocol {
                    detail: format!(
                        "vfs read-back failed and targeted cleanup failed: {cleanup_err}; read-back error: {err}"
                    ),
                };
                record_vfs_failure(state, plan, cleanup_failure.to_string());
                return Err(cleanup_failure);
            }
            applied.borrow_mut().rules.clear();
            let detail = format!("VFS read-back failed; rules were rolled back: {err}");
            record_vfs_failure(state, plan, &detail);
            if config.vfs_strict {
                return Err(Error::VfsProtocol { detail });
            }
            log::warn!("{detail}");
            return Ok(vfs_failure_stats(detail));
        }
    }
    Ok(stats)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn mount_magic_phase(
    config: &Config,
    plan: &MountPlan,
    mount_source: &str,
    work_dir: Option<&Path>,
    transaction: &mut crate::platform::sys::transaction::MountTransaction<'_>,
    mounted: &mut MountedTargets,
) -> Result<exec::MagicMountStats> {
    if plan.magic_module_ids.is_empty() {
        return Ok(exec::MagicMountStats::default());
    }

    log::info!(
        "magic mount phase start: modules={}, module_ids={}, shared_tree=true, register_unmountable={}",
        plan.magic_module_ids.len(),
        plan.magic_module_ids.join(","),
        !config.disable_umount
    );
    let mut on_mount = |target: &str| register_mounted_target(transaction, mounted, target);
    let stats = exec::magic_mount(
        &plan.tree,
        mount_source,
        work_dir.ok_or_else(|| Error::msg("magic mount work directory is unavailable"))?,
        !config.disable_umount,
        &mut on_mount,
    )?;
    mounted.paths.extend(stats.owned_mounts.iter().cloned());
    log::info!(
        "magic mount phase complete: files={}, symlinks={}, dirs={}, ignored={}",
        stats.mounted_files,
        stats.mounted_symlinks,
        stats.mounted_dirs,
        stats.ignored_files
    );
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    fn vfs_readback_failures_clean_up_before_strict_or_degraded_result() {
        use crate::mount::vfs::protocol::{EncodedRule, ListedRule};

        struct ReadbackKernel {
            fail_read: bool,
            present: bool,
            cleanup_calls: usize,
        }
        impl VfsKernel for ReadbackKernel {
            fn version(&mut self) -> Result<String> {
                Ok("hm1".into())
            }
            fn apply_rules(&mut self, _: &[EncodedRule]) -> Result<()> {
                Ok(())
            }
            fn add_uids(&mut self, _: &[u32]) -> Result<()> {
                Ok(())
            }
            fn remove_rules(&mut self, rules: &[EncodedRule]) -> Result<()> {
                assert_eq!(rules.len(), 1);
                self.cleanup_calls += 1;
                Ok(())
            }
            fn list_rules(&mut self) -> Result<Vec<ListedRule>> {
                if self.fail_read {
                    return Err(Error::msg("injected readback failure"));
                }
                Ok(if self.present {
                    vec![ListedRule {
                        flags: 0,
                        scope: 0.into(),
                        virtual_path: "/system/etc/hosts".into(),
                        real_path: "/module/hosts".into(),
                    }]
                } else {
                    Vec::new()
                })
            }
        }
        for strict in [false, true] {
            for (fail_read, present) in [(false, true), (false, false), (true, false)] {
                let kernel = RefCell::new(ReadbackKernel {
                    fail_read,
                    present,
                    cleanup_calls: 0,
                });
                let applied = RefCell::new(VfsApplied {
                    rules: vec![EncodedRule {
                        flags: 0,
                        virtual_path: b"/system/etc/hosts".to_vec(),
                        real_path: b"/module/hosts".to_vec(),
                    }],
                    ..VfsApplied::default()
                });
                let config = Config {
                    vfs_strict: strict,
                    ..Config::default()
                };
                let result = confirm_vfs_readback(
                    &config,
                    &MountPlan::default(),
                    &mut RunState::default(),
                    &kernel,
                    &applied,
                    VfsExecStats {
                        injected: 1,
                        ..VfsExecStats::default()
                    },
                );
                assert_eq!(kernel.borrow().cleanup_calls, usize::from(!present));
                assert_eq!(applied.borrow().rules.is_empty(), !present);
                if present {
                    assert_eq!(result.unwrap().injected, 1);
                } else if strict {
                    assert!(result.is_err());
                } else {
                    assert!(result.unwrap().failure.is_some());
                }
            }
        }
    }

    #[test]
    fn startup_failure_replaces_stale_state_and_module_snapshot() {
        let fixture = crate::test_support::Fixture::new("startup-failure-state");
        let state_path = fixture.join("run/state.json");
        let scan_ret_path = fixture.join("scan.ret");
        let stale = RunState {
            timestamp: 1,
            active_mounts: vec!["/system".to_owned()],
            ..RunState::default()
        };
        stale.save_to(&state_path).unwrap();
        std::fs::write(&scan_ret_path, br#"[{"id":"stale"}]"#).unwrap();

        let error = Error::msg("scan failed");
        persist_startup_failure_state_to("scan", &error, &state_path, &scan_ret_path);

        let state = RunState::load_from(&state_path);
        assert_eq!(state.failed_stage.as_deref(), Some("scan"));
        assert!(state.active_mounts.is_empty());
        assert_eq!(std::fs::read_to_string(scan_ret_path).unwrap(), "[]");
    }

    #[test]
    fn pipeline_stats_aggregates_all_sources() {
        let stats = pipeline_stats(2, 3, 10, 4, 5, 6);

        assert_eq!(stats.overlayfs_mounts, 5);
        assert_eq!(stats.files_mounted, 10);
        assert_eq!(stats.symlinks_created, 4);
        assert_eq!(stats.ignored_entries, 5);
        assert_eq!(stats.total_mounts, 25);
        assert_eq!(stats.successful_mounts, 25);
    }

    #[test]
    fn pipeline_stats_counts_magic_directory_mounts_in_total() {
        let stats = pipeline_stats(1, 0, 2, 0, 0, 3);

        assert_eq!(stats.total_mounts, 6);
        assert_eq!(stats.successful_mounts, 6);
        assert_eq!(stats.files_mounted, 2);
        assert_eq!(stats.symlinks_created, 0);
        assert_eq!(stats.overlayfs_mounts, 1);
    }

    #[test]
    fn active_mounts_merge_backends_in_sorted_deduplicated_order() {
        let overlay = vec!["/vendor".to_owned(), "/system".to_owned()];
        let magic = vec!["/system".to_owned(), "/system/etc/hosts".to_owned()];
        let vfs = vec![
            "/system/etc/hosts".to_owned(),
            "/product/etc/build.prop".to_owned(),
        ];

        assert_eq!(
            merge_active_mounts(&overlay, &magic, &vfs),
            vec![
                "/product/etc/build.prop".to_owned(),
                "/system".to_owned(),
                "/system/etc/hosts".to_owned(),
                "/vendor".to_owned(),
            ]
        );
    }

    #[test]
    fn vfs_targets_join_active_mounts_without_overlay_or_magic() {
        let vfs = vec![
            "/system/etc/hosts".to_owned(),
            "/system/etc/hosts".to_owned(),
        ];

        assert_eq!(
            merge_active_mounts(&[], &[], &vfs),
            vec!["/system/etc/hosts".to_owned()]
        );
    }

    #[test]
    fn overlay_sources_are_remapped_under_prepared_storage() {
        let module = ModuleRecord {
            id: crate::mount::model::ModuleId::try_from("adb-ndk").unwrap(),
            name: "ADB".to_owned(),
            version: "1".to_owned(),
            author: "a".to_owned(),
            description: "d".to_owned(),
            disabled: false,
            skip_mount: false,
            edits: Default::default(),
            has_mount_files: true,
            source_path: PathBuf::from("/data/adb/modules/adb-ndk"),
            entries: Vec::new(),
        };

        let staged = staged_overlay_path(
            Path::new("/data/adb/modules/adb-ndk/system/bin"),
            &[module],
            Path::new("/mnt/hm_test"),
        )
        .unwrap();

        assert_eq!(staged, PathBuf::from("/mnt/hm_test/adb-ndk/system/bin"));
    }

    #[cfg(unix)]
    #[test]
    fn overlay_storage_sizing_counts_each_shallow_source_again() {
        use crate::mount::overlayfs::layers::ShallowOverlaySource;
        use std::collections::BTreeMap;
        let module_id = crate::mount::model::ModuleId::try_from("overlay_mod").unwrap();
        let module_root = PathBuf::from("/data/adb/modules/overlay_mod");
        let shallow_source = module_root.join("system/priv-app/large.apk");
        let modules = [ModuleRecord {
            id: module_id.clone(),
            name: "Overlay".to_owned(),
            version: "1".to_owned(),
            author: "a".to_owned(),
            description: "d".to_owned(),
            disabled: false,
            skip_mount: false,
            edits: Default::default(),
            has_mount_files: true,
            source_path: module_root.clone(),
            entries: Vec::new(),
        }];
        let plan = MountPlan {
            overlay_module_ids: vec![module_id],
            ..MountPlan::default()
        };
        let execution_plan = (
            Vec::new(),
            BTreeMap::from([(
                PathBuf::from("/system/priv-app"),
                vec![ShallowOverlaySource {
                    source: shallow_source.clone(),
                    destination_relative: PathBuf::from("large.apk"),
                }],
            )]),
        );

        assert_eq!(
            overlay_storage_sizing_paths(&modules, &plan, &execution_plan),
            vec![module_root, shallow_source]
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn injected_overlay_failure_rolls_back_previous_mount_target() {
        use crate::mount::model::OverlayOperation;

        let _fault_guard = crate::platform::sys::faults::test_lock();

        if !crate::test_support::require_mount_namespace() {
            return;
        }

        let root = crate::test_support::Fixture::new("overlay-rollback");
        let first = root.join("first");
        let second = root.join("second");
        let lower = root.join("lower");
        let staging = root.join("staging");
        std::fs::create_dir_all(&first).unwrap();
        std::fs::create_dir_all(&second).unwrap();
        std::fs::create_dir_all(&lower).unwrap();
        std::fs::create_dir_all(&staging).unwrap();

        let mut plan = MountPlan::default();
        plan.overlay_ops.push(OverlayOperation {
            partition: "system".to_owned(),
            target: first.to_string_lossy().into_owned(),
            lowerdirs: vec![lower.clone()],
        });
        plan.overlay_ops.push(OverlayOperation {
            partition: "system".to_owned(),
            target: second.to_string_lossy().into_owned(),
            lowerdirs: vec![lower.clone()],
        });

        let mut transaction = crate::platform::sys::transaction::MountTransaction::new();
        let mut mounted = MountedTargets::default();
        let execution_plan = build_overlay_execution_plan(&plan).unwrap();
        crate::platform::sys::faults::enable_overlay_mount_failure_after(1);
        let result = mount_overlay_phase(
            OverlayPlans {
                mount: &plan,
                execution: &execution_plan,
            },
            &Config::default(),
            None,
            Some(&staging),
            "overlay",
            &mut transaction,
            &mut mounted,
        );
        crate::platform::sys::faults::reset();

        match result {
            Err(err) if err.to_string().contains("injected overlay mount failure") => {}
            Err(err) => panic!("expected injected overlay mount failure: {err}"),
            Ok(_) => panic!("injected overlay mount failure was not triggered"),
        }

        assert_eq!(mounted.paths.len(), 1);
        assert_eq!(mounted.paths[0], first.to_string_lossy().into_owned());
        let report = transaction.rollback();
        assert!(
            report.failures.is_empty(),
            "rollback failures: {:?}",
            report.failures
        );

        let snapshot = crate::platform::sys::mountinfo::MountSnapshot::read().unwrap();
        assert!(!snapshot.contains(&first));
        assert!(!snapshot.contains(&second));
    }

    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    #[test]
    fn run_mount_pipeline_reports_unsupported_platform_on_host() {
        let err = run_mount_pipeline().unwrap_err();
        assert!(err.to_string().contains("linux/android"), "{err}");
    }
}
