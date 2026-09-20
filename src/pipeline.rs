// SPDX-License-Identifier: GPL-3.0-only

//! The argument-free boot pipeline: read config, scan read-only, plan, run OverlayFS,
//! run Magic Mount, commit the KSU try-umount list, then write scan.ret and run/state.json.
//!
//! Mounts and shallow staging write only inside the runtime directory; module sources stay read-only.

#[cfg(unix)]
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[cfg(any(target_os = "linux", target_os = "android"))]
use rustix::mount::{
    MountFlags, MountPropagationFlags, UnmountFlags, mount, mount_change, unmount,
};
#[cfg(any(target_os = "linux", target_os = "android"))]
use std::fs;

#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::config::{Config, OverlayMode};
use crate::defs;
use crate::errors::{Error, Result};
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::magic_mount::exec;
#[cfg(unix)]
use crate::plan::MountPlan;
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::plan::{PlanInput, build_plan};
use crate::scanner::ModuleRecord;
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::scanner::list_modules;
use crate::state::MountStatistics;
#[cfg(any(target_os = "linux", target_os = "android", test))]
use crate::state::RunState;
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::state::{app_modules, mounted_module_ids_for_snapshot, write_scan_ret};
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::timing::PhaseTimer;
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::utils;
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::vfs::backend::{KeyringKernel, SUPPORTED_VERSIONS, VfsKernel, select_provider};
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::vfs::exec::{VfsApplied, VfsExecStats};
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::vfs::sys::KeyringChannel;
#[cfg(any(target_os = "linux", target_os = "android"))]
use std::cell::RefCell;
#[cfg(any(target_os = "linux", target_os = "android"))]
use std::rc::Rc;

/// The single entry point for the argument-free boot pipeline.
pub fn run_mount_pipeline() -> Result<()> {
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        Err(Error::msg(
            "mount pipeline is only supported on linux/android",
        ))
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        run_mount_pipeline_impl()
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
    transaction: &mut crate::sys::transaction::MountTransaction<'_>,
    mounted: &mut MountedTargets,
    target: &str,
) {
    let path = PathBuf::from(target);
    transaction.register_rollback_only(format!("mount:{target}"), move || {
        crate::sys::mount::rollback_mount_target(&path)
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
        if crate::sys::faults::should_fail_staging_remove() {
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

pub fn effective_mount_source(ksu_active: bool) -> &'static str {
    if ksu_active { "KSU" } else { "APatch" }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn describe_path_mount(path: &Path) -> String {
    let resolved = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let Ok(mountinfo) = crate::sys::mountinfo::mount_entries() else {
        return "mountinfo=unavailable".to_owned();
    };
    let Some(entry) = mountinfo
        .into_iter()
        .filter(|entry| resolved.starts_with(&entry.mount_point))
        .max_by_key(|entry| entry.mount_point.components().count())
    else {
        return format!("mountinfo=no_match,resolved={}", resolved.display());
    };

    format!(
        "fs={},mount={},source={},device={}",
        entry.fs_type,
        entry.mount_point.display(),
        entry.mount_source.as_deref().unwrap_or("none"),
        entry.majmin
    )
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn log_backend_capabilities() {
    match crate::overlayfs::utils::is_overlay_supported() {
        Ok(supported) => log::info!("capability: overlayfs_supported={supported}"),
        Err(err) => log::warn!("capability probe failed: overlayfs_supported, error={err}"),
    }
    match crate::sys::fs::is_overlay_xattr_supported() {
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

    let stats = crate::sys::fs::stage_overlay_tree(&plan.tree, storage_root)
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
    crate::vfs::exec::VfsExecStats,
);

#[cfg(any(target_os = "linux", target_os = "android"))]
fn execute_mount_phases(
    config: &Config,
    modules: &[ModuleRecord],
    plan: &mut MountPlan,
    mount_source: &str,
    state: &mut RunState,
    transaction: &mut crate::sys::transaction::MountTransaction<'_>,
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
        .then(crate::sys::temp::RuntimeTempDir::create)
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
        let mount_base = crate::sys::temp::create_random_dir(session_root)?;
        let force_ext4 = matches!(config.overlay_mode, OverlayMode::Ext4);
        let handle = crate::storage::setup(
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
        transaction
            .register_retainable("overlay_storage", move || crate::storage::teardown(&handle));
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
        let staging_path = crate::sys::temp::create_random_dir(session_root)?;
        let staging = MagicStagingGuard::mount(staging_path.clone(), mount_source)?;
        let staging_path = staging.path().to_path_buf();
        transaction.register("magic_staging", move || staging.cleanup());
        Some(crate::sys::temp::create_random_dir(&staging_path)?)
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
    let vfs_stats = apply_vfs_phase(config, plan, state, transaction)?;
    vfs_phase.finish();

    crate::utils::ksu::commit_unmount_list()?;

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
    baseline: &crate::sys::mountinfo::MountSnapshot,
    mounted: &MountedTargets,
) -> Result<(Vec<String>, Vec<String>)> {
    let current = crate::sys::mountinfo::MountSnapshot::read()?;
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
    transaction: crate::sys::transaction::MountTransaction<'_>,
    mounted: &MountedTargets,
    baseline: &crate::sys::mountinfo::MountSnapshot,
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
    let mount_errors = crate::state::collect_mount_error_modules(&config.moduledir);
    let snapshot = app_modules(modules, config, plan, &mount_errors, &BTreeSet::new());
    if let Err(err) = write_scan_ret(&snapshot) {
        log::error!("failed to restore unmounted module snapshot: {err}");
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn confirmed_mount_targets(
    targets: &[String],
    snapshot: &crate::sys::mountinfo::MountSnapshot,
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
    if let Err(snapshot_err) = crate::state::write_scan_ret_to(&[], scan_ret_path) {
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
fn run_mount_pipeline_impl() -> Result<()> {
    let startup = PhaseTimer::start("startup");
    utils::ksu::init();

    // Config and scan.ret share the persisted directory; state.json lives in the run subdirectory.
    for path in [defs::CONFIG_PATH, defs::STATE_PATH] {
        if let Some(directory) = Path::new(path).parent()
            && let Err(err) = crate::sys::fs::cleanup_stale_atomic_temp_files(directory)
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
    let ksu_active = utils::ksu::is_active();
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
    let managed_partitions = managed_partition_names();
    let modules = startup_phase("scan", list_modules(&config.moduledir, &managed_partitions))?;
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

    // Probe, then load the bundled module if one is wanted and the key type is still silent.
    // This has to finish before planning: a plan built while the module is unloaded carries no
    // vfs work, which would leave the executor's own load step unreachable. Afterwards the probe
    // is authoritative for the plan and for every surface that advertises vfs.
    let vfs_available = crate::vfs::ensure_loaded_for_plan(
        config.wants_vfs(),
        crate::vfs::available,
        crate::vfs::lkm::load_hm_vfs,
    );
    log::info!(
        "vfs provider probe: wants_vfs={}, available={}, strict={}",
        config.wants_vfs(),
        vfs_available,
        config.vfs_strict
    );

    // `vfs_strict` promises that an unavailable VFS fails the boot, and this is the only point
    // where that is still decidable: the plan below rewrites every `vfs` rule to `ignore`, so
    // `apply_vfs_phase` sees an empty module set and returns before its own strict checks. Without
    // this the option would be silently ineffective for exactly the case it exists to catch.
    if crate::vfs::unavailable_is_fatal(config.wants_vfs(), config.vfs_strict, vfs_available) {
        return startup_phase(
            "vfs",
            Err(Error::VfsUnavailable {
                reason: "no supported VFS kernel provider and vfs_strict is enabled".to_owned(),
            }),
        );
    }

    let plan_phase = PhaseTimer::start("plan");
    let promoted = detect_promoted_partitions();
    log::info!(
        "partition detection: managed={}, promoted={}",
        managed_partitions.join(","),
        promoted.iter().cloned().collect::<Vec<_>>().join(",")
    );
    let mut plan = startup_phase(
        "plan",
        build_plan(&PlanInput {
            modules: &modules,
            config: &config,
            promoted_partitions: &promoted,
            vfs_available,
        }),
    )?;
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

    // `modules` is a boot-time snapshot, not a proof that every mount already
    // succeeded.  Persist it before entering the fallible mount phases so the
    // WebUI can still show the scanned modules and their planned modes when a
    // device rejects one overlay operation.
    let state_phase = PhaseTimer::start("state");
    let initial_mount_errors = crate::state::collect_mount_error_modules(&config.moduledir);
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
    startup_phase("state", state.save())?;
    state_phase.finish();
    log::info!(
        "planned state saved: overlay_modules={}, magic_modules={}",
        state.overlay_modules.len(),
        state.magic_modules.len()
    );

    let baseline = startup_phase("baseline", crate::sys::mountinfo::MountSnapshot::read())?;
    let mut transaction = crate::sys::transaction::MountTransaction::new();
    transaction.register_rollback_only("ksu_try_umount_list", || {
        crate::utils::ksu::clear_unmount_list()
    });
    let mut mounted = MountedTargets::default();
    let (overlay_dir_mounts, shallow_overlay_mounts, active_mounts, magic_stats, vfs_stats) =
        match execute_mount_phases(
            &config,
            &modules,
            &mut plan,
            mount_source,
            &mut state,
            &mut transaction,
            &mut mounted,
        ) {
            Ok(result) => result,
            Err(err) => {
                log::error!(
                    "mount execution failed: phase=mount_execution, error={err}, overlay_modules={}, magic_modules={}",
                    plan.overlay_module_ids.join(","),
                    plan.magic_module_ids.join(",")
                );
                let rollback = rollback_mount_pipeline(transaction, &mounted, &baseline);
                persist_mount_failure_state(&mut state, "mount_execution", &rollback);
                persist_unmounted_module_snapshot(&modules, &config, &plan);
                return Err(err);
            }
        };

    // `is_mounted` comes from mountinfo-confirmed targets, never from the plan.
    // Executor attempt counts stay in `mount_stats` and are never copied into
    // `active_mounts` as proof of still being mounted.
    let final_mountinfo = match crate::sys::mountinfo::MountSnapshot::read() {
        Ok(snapshot) => snapshot,
        Err(err) => {
            log::error!("phase=mountinfo_confirm failed: {err}");
            let rollback = rollback_mount_pipeline(transaction, &mounted, &baseline);
            persist_mount_failure_state(&mut state, "mountinfo_confirm", &rollback);
            persist_unmounted_module_snapshot(&modules, &config, &plan);
            return Err(err);
        }
    };
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

    let mount_error_modules = crate::state::collect_mount_error_modules(&config.moduledir);
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
        let rollback = rollback_mount_pipeline(transaction, &mounted, &baseline);
        persist_mount_failure_state(&mut state, "module_snapshot_save", &rollback);
        persist_unmounted_module_snapshot(&modules, &config, &plan);
        return Err(err);
    }

    let mount_error_reasons = mount_error_modules
        .iter()
        .map(|module| (module.clone(), "mount_error marker present".to_owned()))
        .collect();

    let state_phase = PhaseTimer::start("state");
    state.active_mounts = confirmed_active_mounts.clone();
    state.overlay_active_mounts = confirmed_overlay_targets;
    state.magic_active_mounts = confirmed_magic_targets;
    state.vfs_active_mounts = vfs_stats.active_targets.clone();
    state.confirmed_active_mounts = confirmed_active_mounts;
    state.mount_stats = pipeline_stats(
        overlay_dir_mounts,
        shallow_overlay_mounts,
        magic_stats.mounted_files as usize,
        magic_stats.mounted_symlinks as usize,
        magic_stats.ignored_files as usize,
        magic_stats.mounted_dirs as usize,
    );
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
        let rollback = rollback_mount_pipeline(transaction, &mounted, &baseline);
        persist_mount_failure_state(&mut state, "state_save", &rollback);
        persist_unmounted_module_snapshot(&modules, &config, &plan);
        return Err(err);
    }
    state_phase.finish();

    let cleanup_phase = PhaseTimer::start("cleanup");
    if let Err(err) = transaction.commit(config.disable_umount) {
        cleanup_phase.abort();
        log::error!("phase=mount_transaction_commit failed: {err}");
        let rollback = match mountinfo_mismatches(&baseline, &mounted) {
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
        persist_mount_failure_state(&mut state, "mount_transaction_commit", &rollback);
        persist_unmounted_module_snapshot(&modules, &config, &plan);
        return Err(err);
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

    crate::module_status::update_description(
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
    utils::ensure_dir_exists(tmp_root)?;
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
    if crate::sys::mount::is_mounted(tmp_root)? {
        unmount(tmp_root, UnmountFlags::DETACH).map_err(|err| {
            Error::msg(format!(
                "detach magic staging mount {}: {err}",
                tmp_root.display()
            ))
        })?;
        log::info!("magic staging unmounted: target={}", tmp_root.display());
    }
    if crate::sys::mount::is_mounted(tmp_root)? {
        return Err(Error::msg(format!(
            "magic staging mount still mounted after detach: target={}",
            tmp_root.display()
        )));
    }
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn cleanup_tmp_root_best_effort(tmp_root: &Path) -> Result<()> {
    if crate::sys::mount::is_mounted_best_effort(tmp_root) {
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

#[cfg(any(target_os = "linux", target_os = "android"))]
fn detect_promoted_partitions() -> BTreeSet<String> {
    use crate::mount_tree::BUILTIN_PARTITIONS;

    let builtin_requirements = BUILTIN_PARTITIONS
        .iter()
        .copied()
        .collect::<BTreeMap<_, _>>();

    managed_partition_names()
        .into_iter()
        .filter(|partition| {
            let system_partition = Path::new("/system").join(partition);
            let require_symlink = builtin_requirements
                .get(partition.as_str())
                .copied()
                .unwrap_or(true);
            !require_symlink || system_partition.is_symlink()
        })
        .collect()
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn managed_partition_names() -> Vec<String> {
    crate::defs::MANAGED_PARTITIONS
        .iter()
        .filter(|partition| Path::new("/").join(partition).is_dir())
        .map(|partition| (*partition).to_owned())
        .collect()
}

#[cfg(unix)]
#[derive(Debug, Clone, PartialEq, Eq)]
struct ShallowOverlaySource {
    source: PathBuf,
    destination_relative: PathBuf,
}

#[cfg(unix)]
type ShallowOverlaySources = BTreeMap<PathBuf, Vec<ShallowOverlaySource>>;

#[cfg(unix)]
type OverlayExecutionPlan = (Vec<usize>, ShallowOverlaySources);

#[cfg(any(target_os = "linux", target_os = "android"))]
struct OverlayPlans<'a> {
    mount: &'a MountPlan,
    execution: &'a OverlayExecutionPlan,
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
    let mut paths = modules
        .iter()
        .filter(|module| plan.overlay_module_ids.contains(&module.id))
        .map(|module| module.source_path.clone())
        .collect::<Vec<_>>();
    paths.extend(
        execution_plan
            .1
            .values()
            .flatten()
            .map(|source| source.source.clone()),
    );
    paths
}

#[cfg(unix)]
#[derive(Debug, Clone, PartialEq, Eq)]
enum OverlayDirectoryTarget {
    Existing,
    Missing {
        mount_target: PathBuf,
        destination_relative: PathBuf,
    },
}

/// A directory contributed by a module may not exist in the stock partition
/// (for example `system/product/fonts` on a device without `/product/fonts`).
/// OverlayFS still needs a real mount point, so introduce the missing subtree
/// through the nearest existing non-root ancestor instead.
#[cfg(unix)]
fn resolve_overlay_directory_target(target: &Path) -> Result<OverlayDirectoryTarget> {
    if !target.is_absolute() {
        return Err(Error::msg(format!(
            "overlay target is not absolute: {}",
            target.display()
        )));
    }

    match std::fs::metadata(target) {
        Ok(metadata) if metadata.is_dir() => return Ok(OverlayDirectoryTarget::Existing),
        Ok(_) => {
            return Err(Error::msg(format!(
                "overlay directory target exists but is not a directory: {}",
                target.display()
            )));
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => {
            return Err(Error::msg(format!(
                "inspect overlay target {}: {err}",
                target.display()
            )));
        }
    }

    let mut ancestor = target.parent();
    while let Some(candidate) = ancestor {
        // Mounting a synthetic layer over `/` is far too broad and indicates
        // that the expected Android partition root itself is unavailable.
        if candidate == Path::new("/") {
            break;
        }

        match std::fs::metadata(candidate) {
            Ok(metadata) if metadata.is_dir() => {
                let destination_relative = target
                    .strip_prefix(candidate)
                    .map_err(|err| {
                        Error::msg(format!(
                            "derive shallow overlay path {} below {}: {err}",
                            target.display(),
                            candidate.display()
                        ))
                    })?
                    .to_path_buf();
                if destination_relative.as_os_str().is_empty() {
                    return Err(Error::msg(format!(
                        "empty shallow overlay destination for {}",
                        target.display()
                    )));
                }
                return Ok(OverlayDirectoryTarget::Missing {
                    mount_target: candidate.to_path_buf(),
                    destination_relative,
                });
            }
            Ok(_) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                return Err(Error::msg(format!(
                    "inspect overlay ancestor {}: {err}",
                    candidate.display()
                )));
            }
        }
        ancestor = candidate.parent();
    }

    Err(Error::msg(format!(
        "overlay target has no existing non-root ancestor: {}",
        target.display()
    )))
}

/// OverlayFS takes the merged directory inode metadata from the highest
/// lowerdir. A module directory carrying `adb_data_file` (or any other
/// unsuitable label) must therefore not be allowed to relabel an existing
/// stock mount target such as `/system/bin`.
#[cfg(unix)]
fn normalize_direct_overlay_layer_metadata_with(
    plan: &MountPlan,
    mut clone_metadata: impl FnMut(&Path, &Path) -> Result<()>,
) -> Result<usize> {
    let mut normalized = 0;

    for operation in &plan.overlay_ops {
        let target = Path::new(&operation.target);
        if resolve_overlay_directory_target(target)? != OverlayDirectoryTarget::Existing {
            continue;
        }

        for lowerdir in &operation.lowerdirs {
            clone_metadata(target, lowerdir)?;
            normalized += 1;
        }
    }

    Ok(normalized)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn normalize_direct_overlay_layer_metadata(plan: &MountPlan) -> Result<usize> {
    normalize_direct_overlay_layer_metadata_with(plan, |target, lowerdir| {
        crate::sys::fs::clone_directory_metadata(target, lowerdir).map_err(|err| {
            Error::msg(format!(
                "normalize direct overlay layer metadata {} -> {}: {err}",
                target.display(),
                lowerdir.display()
            ))
        })?;
        log::debug!(
            "overlay layer root metadata normalized: target={}, layer={}",
            target.display(),
            lowerdir.display()
        );
        Ok(())
    })
}

#[cfg(unix)]
fn build_overlay_execution_plan(plan: &MountPlan) -> Result<OverlayExecutionPlan> {
    let mut direct_operations = Vec::new();
    let mut shallow = ShallowOverlaySources::new();

    for (target, sources) in &plan.overlay_files {
        let target_path = Path::new(target);
        let (mount_target, prefix) = match resolve_overlay_directory_target(target_path)? {
            OverlayDirectoryTarget::Existing => (target_path.to_path_buf(), PathBuf::new()),
            OverlayDirectoryTarget::Missing {
                mount_target,
                destination_relative,
            } => {
                log::info!(
                    "shallow overlay parent rerouted: requested_target={}, mount_target={}, relative={}",
                    target_path.display(),
                    mount_target.display(),
                    destination_relative.display()
                );
                (mount_target, destination_relative)
            }
        };

        for source in sources {
            let file_name = source.file_name().ok_or_else(|| {
                Error::msg(format!(
                    "overlay file source has no file name: {}",
                    source.display()
                ))
            })?;
            shallow
                .entry(mount_target.clone())
                .or_default()
                .push(ShallowOverlaySource {
                    source: source.clone(),
                    destination_relative: prefix.join(file_name),
                });
        }
    }

    for (index, operation) in plan.overlay_ops.iter().enumerate() {
        match resolve_overlay_directory_target(Path::new(&operation.target))? {
            OverlayDirectoryTarget::Existing => direct_operations.push(index),
            OverlayDirectoryTarget::Missing {
                mount_target,
                destination_relative,
            } => {
                log::info!(
                    "overlay directory rerouted to shallow parent: index={}, requested_target={}, mount_target={}, relative={}, layers={}",
                    index,
                    operation.target,
                    mount_target.display(),
                    destination_relative.display(),
                    operation.lowerdirs.len()
                );
                let entries = shallow.entry(mount_target).or_default();
                entries.extend(operation.lowerdirs.iter().cloned().map(|source| {
                    ShallowOverlaySource {
                        source,
                        destination_relative: destination_relative.clone(),
                    }
                }));
            }
        }
    }

    for entries in shallow.values_mut() {
        entries.sort_by(|left, right| {
            left.source
                .cmp(&right.source)
                .then_with(|| left.destination_relative.cmp(&right.destination_relative))
        });
    }

    Ok((direct_operations, shallow))
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn mount_overlay_phase(
    plans: OverlayPlans<'_>,
    config: &Config,
    storage_root: Option<&Path>,
    transient_root: Option<&Path>,
    effective_mount_source: &str,
    transaction: &mut crate::sys::transaction::MountTransaction<'_>,
    mounted: &mut MountedTargets,
) -> Result<(usize, usize, Vec<String>)> {
    use crate::overlayfs::overlayfs::{MountEffect, mount_overlay};

    let plan = plans.mount;
    let mut overlay_dir_mounts = 0;
    let mut shallow_overlay_mounts = 0;
    let mut active_mounts = Vec::new();
    let mut on_effect = |effect: MountEffect| match effect {
        MountEffect::Target(target) => register_mounted_target(transaction, mounted, &target),
        MountEffect::Staging(path) => transaction
            .register("intermediate_overlay_staging", move || {
                crate::overlayfs::overlayfs::cleanup_staging_mount(path)
            }),
    };

    let (direct_operations, shallow) = plans.execution;

    for &operation_index in direct_operations {
        let op = &plan.overlay_ops[operation_index];
        let lowerdirs = op
            .lowerdirs
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        let staging_root = transient_root
            .ok_or_else(|| Error::msg("overlay mount requires a runtime temporary session"))?;
        let mount_source = overlay_mount_source(&op.target, effective_mount_source);
        let register_unmountable = !config.disable_umount;
        log::debug!(
            "overlay apply start: index={}, partition={}, target={}, source={}, layers={}, register_unmountable={}",
            operation_index,
            op.partition,
            op.target,
            mount_source,
            lowerdirs.len(),
            register_unmountable
        );
        for (layer_index, lowerdir) in op.lowerdirs.iter().enumerate() {
            log::debug!(
                "overlay apply lowerdir: operation={}, layer={}, path={}, exists={}, is_dir={}, mount={}",
                operation_index,
                layer_index,
                lowerdir.display(),
                lowerdir.exists(),
                lowerdir.is_dir(),
                describe_path_mount(lowerdir)
            );
        }
        if crate::sys::faults::should_fail_next_overlay_mount() {
            return Err(Error::msg(format!(
                "injected overlay mount failure: target={}",
                op.target
            )));
        }
        mount_overlay(
            &op.target,
            &lowerdirs,
            None,
            None,
            staging_root,
            mount_source,
            register_unmountable,
            &mut on_effect,
        )
        .map_err(|err| {
            Error::msg(format!(
                "overlay mount failed: partition={}, target={}: {err}",
                op.partition, op.target
            ))
        })?;
        if register_unmountable {
            utils::ksu::send_unmountable(Path::new(&op.target));
        }
        log::debug!(
            "overlay apply complete: index={}, target={}, target_mount={}",
            operation_index,
            op.target,
            describe_path_mount(Path::new(&op.target))
        );
        active_mounts.push(op.target.clone());
        overlay_dir_mounts += 1;
    }

    if !shallow.is_empty() {
        let storage_root = storage_root
            .ok_or_else(|| Error::msg("shallow overlays require prepared overlay storage"))?;
        shallow_overlay_mounts = mount_overlay_files(
            shallow,
            config,
            storage_root,
            transient_root.ok_or_else(|| {
                Error::msg("shallow overlay requires a runtime temporary session")
            })?,
            effective_mount_source,
            &mut active_mounts,
            &mut on_effect,
        )?;
    }

    active_mounts.sort();
    active_mounts.dedup();
    Ok((overlay_dir_mounts, shallow_overlay_mounts, active_mounts))
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn mount_overlay_files(
    files: &ShallowOverlaySources,
    config: &Config,
    storage_root: &Path,
    transient_root: &Path,
    effective_mount_source: &str,
    active_mounts: &mut Vec<String>,
    on_effect: &mut dyn FnMut(crate::overlayfs::overlayfs::MountEffect),
) -> Result<usize> {
    use crate::overlayfs::overlayfs::mount_overlay;

    let staging_root = crate::sys::temp::create_random_dir(storage_root)?;

    let mut overlay_mounts = 0;
    let mut total_layers = 0;
    log::info!(
        "shallow overlay phase start: targets={}, staging_root={}, staging_mount={}",
        files.len(),
        staging_root.display(),
        describe_path_mount(storage_root)
    );
    for (target_index, (target, sources)) in files.iter().enumerate() {
        let target_string = target.to_string_lossy();
        log::debug!(
            "shallow overlay prepare: index={}, target={}, sources={}",
            target_index,
            target.display(),
            sources.len()
        );
        let mut lowerdirs = Vec::new();
        for (index, entry) in sources.iter().enumerate() {
            let layer_dir = crate::sys::temp::create_random_dir(&staging_root)?;
            let dest =
                prepare_shallow_destination(target, &layer_dir, &entry.destination_relative)?;
            log::debug!(
                "shallow overlay source: target_index={}, layer={}, source={}, exists={}, source_mount={}, relative={}, destination={}",
                target_index,
                index,
                entry.source.display(),
                entry.source.exists(),
                describe_path_mount(&entry.source),
                entry.destination_relative.display(),
                dest.display()
            );
            copy_entry(&entry.source, &dest)?;

            lowerdirs.push(layer_dir.to_string_lossy().into_owned());
        }

        let mount_source = overlay_mount_source(&target_string, effective_mount_source);
        let register_unmountable = !config.disable_umount;
        if crate::sys::faults::should_fail_next_overlay_mount() {
            return Err(Error::msg(format!(
                "injected shallow overlay mount failure: target={}",
                target.display()
            )));
        }
        mount_overlay(
            &target_string,
            &lowerdirs,
            None,
            None,
            transient_root,
            mount_source,
            register_unmountable,
            on_effect,
        )
        .map_err(|err| {
            Error::msg(format!(
                "shallow overlay mount failed: target={}: {err}",
                target.display()
            ))
        })?;
        if register_unmountable {
            utils::ksu::send_unmountable(target);
        }
        log::debug!(
            "shallow overlay complete: index={}, target={}, layers={}, target_mount={}",
            target_index,
            target.display(),
            lowerdirs.len(),
            describe_path_mount(target)
        );
        active_mounts.push(target_string.into_owned());
        total_layers += lowerdirs.len();
        overlay_mounts += 1;
    }

    log::info!(
        "shallow overlay phase complete: targets={}, layers={}",
        files.len(),
        total_layers
    );
    Ok(overlay_mounts)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn prepare_shallow_destination(
    target: &Path,
    layer_dir: &Path,
    destination_relative: &Path,
) -> Result<PathBuf> {
    if destination_relative.as_os_str().is_empty() || destination_relative.is_absolute() {
        return Err(Error::msg(format!(
            "invalid shallow overlay destination: {}",
            destination_relative.display()
        )));
    }

    crate::sys::fs::clone_directory_metadata(target, layer_dir)?;

    let mut structural_dir = layer_dir.to_path_buf();
    if let Some(parent) = destination_relative.parent() {
        for component in parent.components() {
            let std::path::Component::Normal(name) = component else {
                return Err(Error::msg(format!(
                    "unsafe shallow overlay destination: {}",
                    destination_relative.display()
                )));
            };
            structural_dir.push(name);
            fs::create_dir(&structural_dir)?;
            crate::sys::fs::clone_directory_metadata(target, &structural_dir)?;
        }
    }

    Ok(layer_dir.join(destination_relative))
}

fn overlay_mount_source<'a>(target: &str, configured: &'a str) -> &'a str {
    if defs::IGNORE_UNMOUNT_PARTITIONS
        .iter()
        .any(|ignored| ignored.trim() == target.trim())
    {
        "overlay"
    } else {
        configured
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn copy_entry(source: &Path, dest: &Path) -> Result<()> {
    crate::sys::fs::copy_prepared_entry(source, dest)
}

/// RAII guard for the VFS boot guard file: once armed, any handled return (Ok or Err)
/// clears it on Drop. Only a hard crash leaves it behind and trips the next boot.
#[cfg(any(target_os = "linux", target_os = "android"))]
struct VfsBootGuard {
    path: PathBuf,
}

#[cfg(any(target_os = "linux", target_os = "android"))]
impl VfsBootGuard {
    fn arm() -> Result<Self> {
        let path = PathBuf::from(defs::VFS_BOOT_GUARD_PATH);
        crate::sys::fs::atomic_write(&path, b"1").map_err(|err| {
            let cause = match err {
                Error::Io(source) => crate::errors::CausalError::Io(source),
                other => crate::errors::CausalError::Message(other.to_string()),
            };
            Error::Vfs(Box::new(crate::errors::ContextError::new(
                "write vfs boot guard",
                Some(path.clone()),
                cause,
            )))
        })?;
        Ok(Self { path })
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
impl Drop for VfsBootGuard {
    fn drop(&mut self) {
        if let Err(err) = fs::remove_file(&self.path) {
            log::warn!("clear vfs boot guard failed: {err}");
        }
    }
}

/// One-way guard: detects whether a foreign NoMount implementation already exists on the device.
///
/// Probed once, without reading the other side's rules or arbitrating. A failed probe (key
/// type unregistered, unsupported platform, allocation failure) counts as "absent", so this
/// is defence in depth rather than the only guarantee: `hybridmount` and NoMount use different
/// key types, and setup.sh refuses to let both coexist.
#[cfg(any(target_os = "linux", target_os = "android"))]
fn detect_foreign_nomount() -> bool {
    match KeyringKernel::new(KeyringChannel::Nomount) {
        Ok(mut probe) => match probe.version() {
            Ok(version) => {
                log::warn!("foreign NoMount VFS implementation detected (version {version})");
                true
            }
            Err(_) => false,
        },
        Err(_) => false,
    }
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
fn apply_vfs_phase(
    config: &Config,
    plan: &MountPlan,
    state: &mut RunState,
    transaction: &mut crate::sys::transaction::MountTransaction<'_>,
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
    let guard = Path::new(defs::VFS_BOOT_GUARD_PATH);
    if guard.exists() {
        log::warn!("vfs boot guard present; skipping vfs backend this boot");
        return Ok(VfsExecStats::default());
    }
    // Any handled return past this point clears the guard on Drop.
    let _guard = VfsBootGuard::arm()?;

    let mut kernel = KeyringKernel::new(KeyringChannel::Hybridmount)?;
    let foreign_nomount = detect_foreign_nomount();
    state.vfs_foreign_nomount = foreign_nomount;
    let provider = match select_provider(
        &mut kernel,
        SUPPORTED_VERSIONS,
        foreign_nomount,
        crate::vfs::lkm::load_hm_vfs,
    ) {
        Ok(Some(provider)) => provider,
        Ok(None) => {
            log::warn!("vfs backend unavailable; vfs modules are skipped this boot");
            if config.vfs_strict {
                return Err(Error::VfsUnavailable {
                    reason: "no supported VFS kernel provider and vfs_strict is enabled".to_owned(),
                });
            }
            return Ok(VfsExecStats::default());
        }
        // Unsupported version or a foreign NoMount: degrade unless vfs_strict.
        Err(err @ (Error::VfsUnsupportedVersion { .. } | Error::VfsForeignNomount { .. })) => {
            log::warn!("vfs backend is not attached, treating it as unavailable: {err}");
            if config.vfs_strict {
                return Err(err);
            }
            return Ok(VfsExecStats::default());
        }
        Err(err) => return Err(err),
    };

    // Build the full batch and register it for rollback before applying: the kernel
    // applies non-atomically, so a mid-batch failure leaves an applied prefix to undo.
    // Rollback is always targeted, never a CLEAR_RULES of the whole provider table.
    let planned = crate::vfs::exec::plan_rules(plan)?;
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
        crate::vfs::exec::apply_rules_with_policy(
            &mut *kernel,
            &batch,
            &config.vfs_isolate_uids,
            config.vfs_strict,
        )?
    };

    let Some(stats) = outcome else {
        // Already deleted inline; clearing avoids a duplicate DEL_RULE at rollback.
        applied.borrow_mut().rules.clear();
        log::warn!(
            "vfs backend degraded: rules were rolled back and vfs is skipped this boot (rules={})",
            planned_rule_count
        );
        return Ok(VfsExecStats::default());
    };

    state.vfs_provider = Some(provider.as_str().to_owned());
    log::info!(
        "vfs phase complete: provider={}, injected={}, whiteouts={}, opaque={}",
        provider.as_str(),
        stats.injected,
        stats.whiteouts,
        stats.opaque
    );

    // An acknowledged batch is not proof the rules are installed, so read the table back and
    // report the difference. This is the line that tells a device log whether VFS is really
    // active, and a listing failure is logged rather than raised: the rules were applied.
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
    match shared.borrow_mut().list_rules() {
        Ok(listed) => {
            let missing = {
                let expected = applied.borrow();
                crate::vfs::exec::missing_rules(&expected.rules, &listed)
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
                    return Err(Error::VfsProtocol {
                        detail: format!(
                            "vfs read-back was incomplete and targeted cleanup failed: {cleanup_err}"
                        ),
                    });
                }
                applied.borrow_mut().rules.clear();
                if !crate::vfs::exec::evaluate_readback(&missing, config.vfs_strict)? {
                    log::warn!(
                        "vfs read-back mismatch degraded: removed this run's rules and marked vfs inactive"
                    );
                    state.vfs_provider = None;
                    return Ok(VfsExecStats::default());
                }
            }
        }
        Err(err) => log::warn!("vfs read-back failed, rules were applied: {err}"),
    }
    Ok(stats)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn mount_magic_phase(
    config: &Config,
    plan: &MountPlan,
    mount_source: &str,
    work_dir: Option<&Path>,
    transaction: &mut crate::sys::transaction::MountTransaction<'_>,
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

    /// HM-RUST-013: Magic Mount 目录挂载（Move/Replace）必须进入 total_mounts。
    #[test]
    fn pipeline_stats_counts_magic_directory_mounts_in_total() {
        let stats = pipeline_stats(1, 0, 2, 0, 0, 3);

        // 1 overlay-dir + 2 magic files + 3 magic dirs = 6
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

    /// A VFS-only boot reports its injection points as active mount points even though
    /// the kernel never confirms them through mountinfo.
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
            id: crate::module_id::ModuleId::try_from("adb-ndk").unwrap(),
            name: "ADB".to_owned(),
            version: "1".to_owned(),
            author: "a".to_owned(),
            description: "d".to_owned(),
            disabled: false,
            skip_mount: false,
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
        let module_id = crate::module_id::ModuleId::try_from("overlay_mod").unwrap();
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

    #[cfg(unix)]
    #[test]
    fn existing_overlay_directory_stays_direct() {
        let fixture = std::env::temp_dir().join(format!(
            "hybrid-mount-existing-overlay-target-{}",
            std::process::id()
        ));
        let target = fixture.join("product/fonts");
        std::fs::remove_dir_all(&fixture).ok();
        std::fs::create_dir_all(&target).unwrap();

        assert_eq!(
            resolve_overlay_directory_target(&target).unwrap(),
            OverlayDirectoryTarget::Existing
        );

        std::fs::remove_dir_all(&fixture).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn missing_overlay_directory_uses_nearest_existing_parent() {
        let fixture = std::env::temp_dir().join(format!(
            "hybrid-mount-missing-overlay-target-{}",
            std::process::id()
        ));
        let product = fixture.join("product");
        let target = product.join("fonts/google");
        std::fs::remove_dir_all(&fixture).ok();
        std::fs::create_dir_all(&product).unwrap();

        assert_eq!(
            resolve_overlay_directory_target(&target).unwrap(),
            OverlayDirectoryTarget::Missing {
                mount_target: product,
                destination_relative: PathBuf::from("fonts/google"),
            }
        );

        std::fs::remove_dir_all(&fixture).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn non_directory_overlay_target_is_rejected() {
        let fixture = std::env::temp_dir().join(format!(
            "hybrid-mount-file-overlay-target-{}",
            std::process::id()
        ));
        let target = fixture.join("product/fonts");
        std::fs::remove_dir_all(&fixture).ok();
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, "not a directory").unwrap();

        let err = resolve_overlay_directory_target(&target).unwrap_err();
        assert!(err.to_string().contains("not a directory"), "{err}");

        std::fs::remove_dir_all(&fixture).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn direct_overlay_layer_root_uses_stock_directory_label() {
        use crate::plan::OverlayOperation;
        use std::collections::BTreeMap;

        let fixture = std::env::temp_dir().join(format!(
            "hybrid-mount-overlay-root-label-{}",
            std::process::id()
        ));
        let stock_bin = fixture.join("system/bin");
        let droidspaces_bin = fixture.join("staging/droidspaces/system/bin");
        let other_bin = fixture.join("staging/other/system/bin");
        std::fs::remove_dir_all(&fixture).ok();
        std::fs::create_dir_all(&stock_bin).unwrap();
        std::fs::create_dir_all(&droidspaces_bin).unwrap();
        std::fs::create_dir_all(&other_bin).unwrap();

        let mut plan = MountPlan::default();
        plan.overlay_ops.push(OverlayOperation {
            partition: "system".to_owned(),
            target: stock_bin.to_string_lossy().into_owned(),
            lowerdirs: vec![droidspaces_bin.clone(), other_bin.clone()],
        });

        let system_file = "u:object_r:system_file:s0".to_owned();
        let adb_data_file = "u:object_r:adb_data_file:s0".to_owned();
        let mut labels = BTreeMap::from([
            (stock_bin.clone(), system_file.clone()),
            (droidspaces_bin.clone(), adb_data_file.clone()),
            (other_bin.clone(), adb_data_file),
        ]);

        let normalized =
            normalize_direct_overlay_layer_metadata_with(&plan, |source, destination| {
                let label = labels.get(source).cloned().ok_or_else(|| {
                    Error::msg(format!("missing test label for {}", source.display()))
                })?;
                labels.insert(destination.to_path_buf(), label);
                Ok(())
            })
            .unwrap();

        assert_eq!(normalized, 2);
        assert_eq!(labels.get(&droidspaces_bin), Some(&system_file));
        assert_eq!(labels.get(&other_bin), Some(&system_file));

        std::fs::remove_dir_all(&fixture).unwrap();
    }

    #[test]
    fn default_mount_source_follows_the_active_root_backend() {
        assert_eq!(effective_mount_source(true), "KSU");
        assert_eq!(effective_mount_source(false), "APatch");
    }

    #[test]
    fn ignored_overlay_partition_uses_neutral_mount_source() {
        assert_eq!(overlay_mount_source("/system/lib", "KSU"), "overlay");
        assert_eq!(overlay_mount_source("/vendor/lib64", "APatch"), "overlay");
    }

    #[test]
    fn regular_overlay_partition_keeps_backend_mount_source() {
        assert_eq!(overlay_mount_source("/system", "KSU"), "KSU");
        assert_eq!(overlay_mount_source("/product", "APatch"), "APatch");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn injected_overlay_failure_rolls_back_previous_mount_target() {
        use crate::plan::OverlayOperation;

        let _fault_guard = crate::sys::faults::test_lock();

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

        let mut transaction = crate::sys::transaction::MountTransaction::new();
        let mut mounted = MountedTargets::default();
        let execution_plan = build_overlay_execution_plan(&plan).unwrap();
        crate::sys::faults::enable_overlay_mount_failure_after(1);
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
        crate::sys::faults::reset();

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

        let snapshot = crate::sys::mountinfo::MountSnapshot::read().unwrap();
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
