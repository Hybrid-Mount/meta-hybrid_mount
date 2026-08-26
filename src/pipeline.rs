// SPDX-License-Identifier: GPL-3.0-only

//! 无参数启动流水线:读配置 → 只读扫描 → planner → overlayfs 执行 →
//! magic mount 执行 → 提交 KSU 尝试卸载列表 → 写 scan.ret / run/state.json。
//!
//! 挂载与 shallow staging 只写运行目录,模块源目录只读。

#[cfg(any(target_os = "linux", target_os = "android"))]
use std::collections::{BTreeMap, BTreeSet};
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
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::plan::{MountPlan, PlanInput, build_plan};
use crate::scanner::ModuleRecord;
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::scanner::list_modules;
use crate::state::MountStatistics;
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::state::{RunState, app_modules, write_scan_ret};
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::utils;

/// 无参数启动挂载流水线的统一入口。
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

/// 由执行数字汇总出状态快照统计(纯函数,可跨平台测试)。
pub fn pipeline_stats(
    overlay_dir_mounts: usize,
    shallow_overlay_mounts: usize,
    magic_files: usize,
    magic_symlinks: usize,
    ignored_entries: usize,
) -> MountStatistics {
    let successful = overlay_dir_mounts + shallow_overlay_mounts + magic_files + magic_symlinks;

    MountStatistics {
        total_mounts: successful,
        successful_mounts: successful,
        failed_mounts: 0,
        files_mounted: magic_files,
        symlinks_created: magic_symlinks,
        overlayfs_mounts: overlay_dir_mounts + shallow_overlay_mounts,
        ignored_entries,
    }
}

/// 文件级 overlay 规则的 shallow 目录规划:每个源文件一个独立层目录。
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
    Ok(storage_root.join(&module.id).join(relative))
}

#[cfg(any(target_os = "linux", target_os = "android"))]
struct OverlayStorageGuard {
    handle: crate::storage::StorageHandle,
    cleanup: bool,
}

#[cfg(any(target_os = "linux", target_os = "android"))]
impl OverlayStorageGuard {
    fn new(handle: crate::storage::StorageHandle, cleanup: bool) -> Self {
        Self { handle, cleanup }
    }

    fn handle(&self) -> &crate::storage::StorageHandle {
        &self.handle
    }

    fn retain(&mut self) {
        self.cleanup = false;
    }

    fn teardown(mut self) -> Result<()> {
        if !self.cleanup {
            return Ok(());
        }

        crate::storage::teardown(&self.handle)?;
        self.cleanup = false;
        Ok(())
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
impl Drop for OverlayStorageGuard {
    fn drop(&mut self) {
        if !self.cleanup {
            log::info!(
                "storage teardown skipped: mode={}, mount_point={}, reason=disable_umount",
                self.handle.mode().as_str(),
                self.handle.mount_point().display()
            );
            return;
        }
        if let Err(err) = crate::storage::teardown(&self.handle) {
            log::warn!("storage teardown failed: {err}");
        }
    }
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
        self.cleanup_now()
    }

    fn cleanup_now(&mut self) -> Result<()> {
        if !self.cleanup {
            return Ok(());
        }

        cleanup_tmp_root(&self.path)?;
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
        if let Err(err) = self.cleanup_now() {
            log::warn!(
                "magic staging cleanup failed: path={}, error={err}",
                self.path.display()
            );
        }
    }
}

pub fn effective_mount_source(configured: &str, ksu_active: bool) -> &str {
    if !ksu_active && configured == defs::DEFAULT_MOUNT_SOURCE {
        "APatch"
    } else {
        configured
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn describe_path_mount(path: &Path) -> String {
    use procfs::process::Process;

    let resolved = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let Ok(process) = Process::myself() else {
        return "mountinfo=process_unavailable".to_owned();
    };
    let Ok(mountinfo) = process.mountinfo() else {
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

    log::info!(
        "overlay staging complete: root={}, operations={}, shallow_targets={}",
        storage_root.display(),
        plan.overlay_ops.len(),
        plan.overlay_files.len()
    );
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn run_mount_pipeline_impl() -> Result<()> {
    utils::ksu::init();

    let config = Config::load_or_default(Path::new(defs::CONFIG_PATH));
    let ksu_active = utils::ksu::is_active();
    let mount_source = effective_mount_source(&config.mountsource, ksu_active).to_owned();
    log::info!("config info: {}", config.to_toml()?);
    log::info!(
        "runtime: pid={}, configured_mount_source={}, effective_mount_source={}, ksu_ioctl_active={}",
        std::process::id(),
        config.mountsource,
        mount_source,
        ksu_active
    );
    log_backend_capabilities();

    let managed_partitions = managed_partition_names();
    let modules = list_modules(&config.moduledir, &managed_partitions);
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
        log::info!(
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

    let promoted = detect_promoted_partitions();
    log::info!(
        "partition detection: managed={}, promoted={}",
        managed_partitions.join(","),
        promoted.iter().cloned().collect::<Vec<_>>().join(",")
    );
    let mut plan = build_plan(&PlanInput {
        modules: &modules,
        config: &config,
        promoted_partitions: &promoted,
    })?;
    log::info!(
        "plan: overlay_ops={}, overlay_modules={}, magic_modules={}",
        plan.overlay_ops.len(),
        plan.overlay_module_ids.len(),
        plan.magic_module_ids.len()
    );
    for (index, op) in plan.overlay_ops.iter().enumerate() {
        log::info!(
            "plan overlay operation: index={}, partition={}, target={}, layers={}",
            index,
            op.partition,
            op.target,
            op.lowerdirs.len()
        );
        for (layer_index, lowerdir) in op.lowerdirs.iter().enumerate() {
            log::info!(
                "plan overlay lowerdir: operation={}, layer={}, path={}, exists={}, mount={}",
                index,
                layer_index,
                lowerdir.display(),
                lowerdir.exists(),
                describe_path_mount(lowerdir)
            );
        }
    }

    // `modules` is a boot-time snapshot, not a proof that every mount already
    // succeeded.  Persist it before entering the fallible mount phases so the
    // WebUI can still show the scanned modules and their planned modes when a
    // device rejects one overlay operation.
    let initial_mount_errors = crate::state::collect_mount_error_modules(&config.moduledir);
    let mut initial_app_modules = app_modules(&modules, &config, &plan, &initial_mount_errors);
    for module in &mut initial_app_modules {
        module.is_mounted = false;
    }
    write_scan_ret(&initial_app_modules)?;
    log::info!(
        "module snapshot saved: modules={}",
        initial_app_modules.len()
    );

    // State is a boot snapshot, not a daemon-owned live feed.  Save the plan
    // before any fallible mount operation so `status` and the WebUI can still
    // report the selected backends when the device rejects a later mount.
    let mut state = RunState::from_plan(&config, &modules, &plan, initial_mount_errors);
    state.save()?;
    log::info!(
        "planned state saved: overlay_modules={}, magic_modules={}",
        state.overlay_modules.len(),
        state.magic_modules.len()
    );

    let execution_result: Result<_> = (|| {
        let needs_runtime_temp = !plan.overlay_ops.is_empty()
            || !plan.overlay_files.is_empty()
            || !plan.magic_module_ids.is_empty();
        let mut runtime_temp = needs_runtime_temp
            .then(crate::sys::temp::RuntimeTempDir::create)
            .transpose()?;
        let transient_root = runtime_temp
            .as_ref()
            .map(|session| session.path().to_path_buf());

        let mut overlay_storage = if plan.overlay_ops.is_empty() && plan.overlay_files.is_empty() {
            log::info!("overlay storage skipped: reason=no_overlay_operations");
            None
        } else {
            let session = runtime_temp.as_ref().ok_or_else(|| {
                Error::msg("overlay storage requires a runtime temporary session")
            })?;
            let mount_base = session.allocate_dir()?;
            let force_ext4 = matches!(config.overlay_mode, OverlayMode::Ext4);
            let handle = crate::storage::setup(
                &mount_base,
                &config.moduledir,
                force_ext4,
                &mount_source,
                config.disable_umount,
            )
            .map_err(|err| {
                Error::msg(format!(
                    "initialize overlay storage: requested_mode={}, mount_point={}: {err}",
                    config.overlay_mode.as_str(),
                    mount_base.display()
                ))
            })?;
            let guard = OverlayStorageGuard::new(handle, true);
            state.storage_mode = guard.handle().mode().as_str().to_owned();
            state.mount_point = guard.handle().mount_point().to_path_buf();
            state.save()?;
            log::info!(
                "storage state saved: requested_mode={}, actual_mode={}, mount_point={}",
                config.overlay_mode.as_str(),
                state.storage_mode,
                state.mount_point.display()
            );
            prepare_overlay_storage(&modules, &mut plan, guard.handle().mount_point())?;
            Some(guard)
        };

        let magic_staging = if plan.magic_module_ids.is_empty() {
            log::info!("magic staging skipped: reason=no_magic_modules");
            None
        } else {
            let session = runtime_temp
                .as_ref()
                .ok_or_else(|| Error::msg("magic mount requires a runtime temporary session"))?;
            let path = session.allocate_dir()?;
            Some(MagicStagingGuard::mount(path, &mount_source)?)
        };
        let magic_work_dir = magic_staging
            .as_ref()
            .map(|staging| crate::sys::temp::create_random_dir(staging.path()))
            .transpose()?;
        let (overlay_dir_mounts, shallow_overlay_mounts, active_mounts) = mount_overlay_phase(
            &plan,
            &config,
            overlay_storage
                .as_ref()
                .map(|storage| storage.handle().mount_point()),
            transient_root.as_deref(),
            &mount_source,
        )?;
        let magic_stats =
            mount_magic_phase(&config, &plan, &mount_source, magic_work_dir.as_deref())?;

        // Commit the KernelSU try-unmount list. This registers future cleanup;
        // it does not unmount the entries immediately.
        utils::ksu::commit_unmount_list()?;

        if let Some(staging) = magic_staging {
            staging.cleanup()?;
        }

        let retain_storage = config.disable_umount && overlay_storage.is_some();
        if retain_storage {
            if let Some(storage) = overlay_storage.as_mut() {
                storage.retain();
            }
            runtime_temp
                .as_mut()
                .expect("overlay requires session")
                .keep();
        } else {
            if let Some(storage) = overlay_storage.take() {
                storage.teardown()?;
            }
            state.mount_point = PathBuf::new();
            if let Some(session) = runtime_temp.take() {
                session.cleanup()?;
            }
        }
        drop(overlay_storage);
        drop(runtime_temp);

        Ok((
            overlay_dir_mounts,
            shallow_overlay_mounts,
            active_mounts,
            magic_stats,
        ))
    })();

    let (overlay_dir_mounts, shallow_overlay_mounts, active_mounts, magic_stats) =
        match execution_result {
            Ok(result) => result,
            Err(err) => {
                log::error!(
                    "mount execution failed: error={err}, overlay_modules={}, magic_modules={}",
                    plan.overlay_module_ids.join(","),
                    plan.magic_module_ids.join(",")
                );
                state.mount_point = PathBuf::new();
                state.mount_stats.total_mounts = 1;
                state.mount_stats.failed_mounts = 1;
                if let Err(state_err) = state.save() {
                    log::warn!("failed to persist mount failure statistics: {state_err}");
                }
                return Err(err);
            }
        };

    let mount_error_modules = crate::state::collect_mount_error_modules(&config.moduledir);
    let app_modules = app_modules(&modules, &config, &plan, &mount_error_modules);
    write_scan_ret(&app_modules)?;

    let mount_error_reasons = mount_error_modules
        .iter()
        .map(|module| (module.clone(), "mount_error marker present".to_owned()))
        .collect();

    state.active_mounts = active_mounts;
    state.mount_stats = pipeline_stats(
        overlay_dir_mounts,
        shallow_overlay_mounts,
        magic_stats.mounted_files as usize,
        magic_stats.mounted_symlinks as usize,
        magic_stats.ignored_files as usize,
    );
    state.mount_error_modules = mount_error_modules;
    state.mount_error_reasons = mount_error_reasons;
    state.save()?;

    crate::module_status::update_description(
        &state.storage_mode,
        plan.overlay_module_ids.len(),
        plan.magic_module_ids.len(),
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
    if crate::sys::mount::is_mounted(tmp_root) {
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

#[cfg(any(target_os = "linux", target_os = "android"))]
#[derive(Debug, Clone, PartialEq, Eq)]
struct ShallowOverlaySource {
    source: PathBuf,
    destination_relative: PathBuf,
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

#[cfg(any(target_os = "linux", target_os = "android"))]
fn build_overlay_execution_plan(
    plan: &MountPlan,
) -> Result<(Vec<usize>, BTreeMap<PathBuf, Vec<ShallowOverlaySource>>)> {
    let mut direct_operations = Vec::new();
    let mut shallow = BTreeMap::<PathBuf, Vec<ShallowOverlaySource>>::new();

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

#[cfg(any(target_os = "linux", target_os = "android", test))]
fn overlay_rollback_order(active_mounts: &[String]) -> Vec<&Path> {
    // A later parent overlay can hide an earlier child overlay, and the same
    // target can legitimately be overlaid more than once. Peel mounts in the
    // exact reverse application order so each earlier layer becomes visible
    // before its own rollback attempt. Do not sort or deduplicate this list.
    active_mounts
        .iter()
        .rev()
        .map(String::as_str)
        .map(Path::new)
        .collect()
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn rollback_overlay_mounts(active_mounts: &[String]) {
    for target in overlay_rollback_order(active_mounts) {
        if !crate::sys::mount::is_mounted(target) {
            continue;
        }
        match unmount(target, UnmountFlags::DETACH) {
            Ok(()) => log::info!("overlay rollback complete: target={}", target.display()),
            Err(err) => log::warn!(
                "overlay rollback failed: target={}, error={err}",
                target.display()
            ),
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn mount_overlay_phase(
    plan: &MountPlan,
    config: &Config,
    storage_root: Option<&Path>,
    transient_root: Option<&Path>,
    effective_mount_source: &str,
) -> Result<(usize, usize, Vec<String>)> {
    use crate::overlayfs::overlayfs::mount_overlay;

    let mut overlay_dir_mounts = 0;
    let mut shallow_overlay_mounts = 0;
    let mut active_mounts = Vec::new();

    let phase_result = (|| -> Result<()> {
        let (direct_operations, shallow) = build_overlay_execution_plan(plan)?;

        for operation_index in direct_operations {
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
            log::info!(
                "overlay apply start: index={}, partition={}, target={}, source={}, layers={}, register_unmountable={}",
                operation_index,
                op.partition,
                op.target,
                mount_source,
                lowerdirs.len(),
                register_unmountable
            );
            for (layer_index, lowerdir) in op.lowerdirs.iter().enumerate() {
                log::info!(
                    "overlay apply lowerdir: operation={}, layer={}, path={}, exists={}, is_dir={}, mount={}",
                    operation_index,
                    layer_index,
                    lowerdir.display(),
                    lowerdir.exists(),
                    lowerdir.is_dir(),
                    describe_path_mount(lowerdir)
                );
            }
            mount_overlay(
                &op.target,
                &lowerdirs,
                None,
                None,
                staging_root,
                mount_source,
                register_unmountable,
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
            log::info!(
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
                &shallow,
                config,
                storage_root,
                transient_root.ok_or_else(|| {
                    Error::msg("shallow overlay requires a runtime temporary session")
                })?,
                effective_mount_source,
                &mut active_mounts,
            )?;
        }
        Ok(())
    })();

    if let Err(err) = phase_result {
        rollback_overlay_mounts(&active_mounts);
        return Err(err);
    }

    active_mounts.sort();
    active_mounts.dedup();
    Ok((overlay_dir_mounts, shallow_overlay_mounts, active_mounts))
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn mount_overlay_files(
    files: &BTreeMap<PathBuf, Vec<ShallowOverlaySource>>,
    config: &Config,
    storage_root: &Path,
    transient_root: &Path,
    effective_mount_source: &str,
    active_mounts: &mut Vec<String>,
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
        log::info!(
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
            log::info!(
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
        mount_overlay(
            &target_string,
            &lowerdirs,
            None,
            None,
            transient_root,
            mount_source,
            register_unmountable,
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
        log::info!(
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

#[cfg(any(target_os = "linux", target_os = "android"))]
fn mount_magic_phase(
    config: &Config,
    plan: &MountPlan,
    mount_source: &str,
    work_dir: Option<&Path>,
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
    let stats = exec::magic_mount(
        &plan.tree,
        mount_source,
        work_dir.ok_or_else(|| Error::msg("magic mount work directory is unavailable"))?,
        !config.disable_umount,
    )?;
    log::info!(
        "magic mount phase complete: files={}, symlinks={}, ignored={}",
        stats.mounted_files,
        stats.mounted_symlinks,
        stats.ignored_files
    );
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipeline_stats_aggregates_all_sources() {
        let stats = pipeline_stats(2, 3, 10, 4, 5);

        assert_eq!(stats.overlayfs_mounts, 5);
        assert_eq!(stats.files_mounted, 10);
        assert_eq!(stats.symlinks_created, 4);
        assert_eq!(stats.ignored_entries, 5);
        assert_eq!(stats.total_mounts, 19);
        assert_eq!(stats.successful_mounts, 19);
    }

    #[test]
    fn overlay_sources_are_remapped_under_prepared_storage() {
        let module = ModuleRecord {
            id: "adb-ndk".to_owned(),
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

    #[test]
    fn overlay_rollback_reverses_application_order_without_deduplication() {
        let active_mounts = vec![
            "/product/etc".to_owned(),
            "/product".to_owned(),
            "/product".to_owned(),
        ];

        assert_eq!(
            overlay_rollback_order(&active_mounts),
            vec![
                Path::new("/product"),
                Path::new("/product"),
                Path::new("/product/etc"),
            ]
        );
    }

    #[test]
    fn default_mount_source_follows_the_active_root_backend() {
        assert_eq!(effective_mount_source("KSU", true), "KSU");
        assert_eq!(effective_mount_source("KSU", false), "APatch");
        assert_eq!(effective_mount_source("custom", false), "custom");
    }

    #[test]
    fn ignored_overlay_partition_uses_neutral_mount_source() {
        assert_eq!(overlay_mount_source("/system/lib", "KSU"), "overlay");
        assert_eq!(overlay_mount_source("/vendor/lib64", "APatch"), "overlay");
    }

    #[test]
    fn regular_overlay_partition_keeps_configured_mount_source() {
        assert_eq!(overlay_mount_source("/system", "KSU"), "KSU");
        assert_eq!(overlay_mount_source("/product", "APatch"), "APatch");
    }

    #[test]
    fn run_mount_pipeline_reports_unsupported_platform_on_host() {
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        {
            let err = run_mount_pipeline().unwrap_err();
            assert!(err.to_string().contains("linux/android"), "{err}");
        }
    }
}
