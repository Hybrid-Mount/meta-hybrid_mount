// ReHybrid-Mount
//
// SPDX-License-Identifier: GPL-3.0-only

//! 无参数启动流水线:读配置 → 只读扫描 → planner → overlayfs 执行 →
//! magic mount 执行 → 提交 KSU 尝试卸载列表 → 写 scan.ret / run/state.json。
//!
//! 铁律:挂载与 shallow staging 只写运行目录,模块源目录只读。
//!
//! Stage 5 脚手架:执行实现仅在 Linux/Android 编译;纯函数在 host 测试。
#![allow(dead_code)]

#[cfg(any(target_os = "linux", target_os = "android"))]
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[cfg(any(target_os = "linux", target_os = "android"))]
use rustix::mount::{MountFlags, UnmountFlags, mount, unmount};
#[cfg(any(target_os = "linux", target_os = "android"))]
use std::fs;
#[cfg(any(target_os = "linux", target_os = "android"))]
use std::os::unix::fs::symlink;

#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::config::Config;
use crate::defs;
use crate::errors::{Error, Result};
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::magic_mount::exec;
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::magic_mount::scan::{ScanOptions, Selection};
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::plan::{MountPlan, PlanInput, build_plan};
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::scanner::{ModuleRecord, list_modules};
use crate::state::MountStatistics;
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::state::{ModeStats, RunState, app_modules, write_scan_ret};
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::utils;

/// `emulated-soft-reboot` 等命令与无参数流水线的统一入口。
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
    shallow_layer_mounts: usize,
    magic_files: usize,
    magic_symlinks: usize,
    ignored_entries: usize,
) -> MountStatistics {
    let successful = overlay_dir_mounts + shallow_layer_mounts + magic_files + magic_symlinks;

    MountStatistics {
        total_mounts: successful,
        successful_mounts: successful,
        failed_mounts: 0,
        files_mounted: magic_files,
        symlinks_created: magic_symlinks,
        overlayfs_mounts: overlay_dir_mounts + shallow_layer_mounts,
        ignored_entries,
    }
}

/// 文件级 overlay 规则的 shallow 目录规划:每个源文件一个独立层目录。
pub fn shallow_dir_for(target: &str, index: usize) -> PathBuf {
    let safe_target = target.trim_matches('/').replace('/', "_");
    Path::new(defs::SHALLOW_STAGING_DIR).join(format!("{safe_target}_{index}"))
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn run_mount_pipeline_impl() -> Result<()> {
    utils::ksu::init();

    let config = Config::load_or_default(Path::new(defs::CONFIG_PATH));
    log::info!("config info: {}", config.to_toml()?);

    let modules = list_modules(&config.moduledir, &[]);
    log::info!("scanned modules: {}", modules.len());

    let promoted = detect_promoted_partitions();
    let plan = build_plan(&PlanInput {
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

    prepare_tmp_root(&config.mountsource)?;

    let (overlay_dir_mounts, shallow_layer_mounts, active_mounts) =
        mount_overlay_phase(&plan, &config)?;
    let magic_stats = mount_magic_phase(&config, &modules, &plan)?;

    // 提交 KernelSU 尝试卸载列表(注册语义,非立即卸载)。
    utils::ksu::commit_unmount_list()?;

    cleanup_tmp_root();

    let mount_error_modules = crate::state::collect_mount_error_modules(&config.moduledir);
    let app_modules = app_modules(&modules, &config, &plan, &mount_error_modules);
    write_scan_ret(&app_modules)?;

    let skip_mount_modules = modules
        .iter()
        .filter(|module| module.skip_mount)
        .map(|module| module.id.clone())
        .collect::<Vec<_>>();
    let mount_error_reasons = mount_error_modules
        .iter()
        .map(|module| (module.clone(), "mount_error marker present".to_owned()))
        .collect();

    let mut state = RunState::new(
        config.overlay_mode.as_str().to_owned(),
        PathBuf::from(defs::STATE_DIR),
        plan.overlay_module_ids.clone(),
        plan.magic_module_ids.clone(),
        skip_mount_modules,
        active_mounts,
        pipeline_stats(
            overlay_dir_mounts,
            shallow_layer_mounts,
            magic_stats.mounted_files as usize,
            magic_stats.mounted_symlinks as usize,
            magic_stats.ignored_files as usize,
        ),
        ModeStats {
            overlayfs: plan.overlay_module_ids.len(),
            magicmount: plan.magic_module_ids.len(),
        },
    );
    state.mount_error_modules = mount_error_modules;
    state.mount_error_reasons = mount_error_reasons;
    state.save()?;

    log::info!("mount pipeline completed");
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn prepare_tmp_root(mount_source: &str) -> Result<()> {
    let tmp_root = Path::new(defs::TMP_ROOT);
    utils::ensure_dir_exists(tmp_root)?;
    mount(mount_source, tmp_root, "tmpfs", MountFlags::empty(), None).map_err(|err| {
        Error::msg(format!(
            "mount tmpfs {mount_source} at {}: {err}",
            tmp_root.display()
        ))
    })
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn cleanup_tmp_root() {
    let tmp_root = Path::new(defs::TMP_ROOT);
    if let Err(err) = unmount(tmp_root, UnmountFlags::DETACH) {
        log::warn!("unmount {}: {err}", tmp_root.display());
    }
    if let Err(err) = fs::remove_dir(tmp_root) {
        log::warn!("remove {}: {err}", tmp_root.display());
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn detect_promoted_partitions() -> BTreeSet<String> {
    use crate::magic_mount::node::BUILTIN_PARTITIONS;

    BUILTIN_PARTITIONS
        .iter()
        .filter(|(partition, require_symlink)| {
            let root_partition = Path::new("/").join(partition);
            let system_partition = Path::new("/system").join(partition);
            root_partition.is_dir() && (!require_symlink || system_partition.is_symlink())
        })
        .map(|(partition, _)| (*partition).to_owned())
        .collect()
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn mount_overlay_phase(plan: &MountPlan, config: &Config) -> Result<(usize, usize, Vec<String>)> {
    use crate::overlayfs::overlayfs::mount_overlay;

    let mut overlay_dir_mounts = 0;
    let mut shallow_layer_mounts = 0;
    let mut active_mounts = Vec::new();

    for op in &plan.overlay_ops {
        let lowerdirs = op
            .lowerdirs
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        mount_overlay(&op.target, &lowerdirs, None, None, &config.mountsource).map_err(|err| {
            Error::msg(format!(
                "overlay mount failed: partition={}, target={}: {err}",
                op.partition, op.target
            ))
        })?;
        utils::ksu::send_unmountable(Path::new(&op.target));
        active_mounts.push(op.target.clone());
        overlay_dir_mounts += 1;
    }

    if !plan.overlay_files.is_empty() {
        shallow_layer_mounts =
            mount_overlay_files(&plan.overlay_files, &config.mountsource, &mut active_mounts)?;
    }

    active_mounts.sort();
    active_mounts.dedup();
    Ok((overlay_dir_mounts, shallow_layer_mounts, active_mounts))
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn mount_overlay_files(
    files: &BTreeMap<String, Vec<PathBuf>>,
    mount_source: &str,
    active_mounts: &mut Vec<String>,
) -> Result<usize> {
    use crate::overlayfs::overlayfs::mount_overlay;

    // 启动时清空上次的 shallow staging(只写运行目录)。
    let staging_root = Path::new(defs::SHALLOW_STAGING_DIR);
    if staging_root.exists() {
        let _ = crate::sys::fs::remove_path(staging_root);
    }

    let mut layer_mounts = 0;
    for (target, sources) in files {
        let mut lowerdirs = Vec::new();
        for (index, source) in sources.iter().enumerate() {
            let layer_dir = shallow_dir_for(target, index);
            fs::create_dir_all(&layer_dir)?;

            let file_name = source.file_name().ok_or_else(|| {
                Error::msg(format!(
                    "overlay file source has no file name: {}",
                    source.display()
                ))
            })?;
            let dest = layer_dir.join(file_name);
            copy_entry(source, &dest)?;

            lowerdirs.push(layer_dir.to_string_lossy().into_owned());
        }

        mount_overlay(target, &lowerdirs, None, None, mount_source).map_err(|err| {
            Error::msg(format!(
                "shallow overlay mount failed: target={target}: {err}"
            ))
        })?;
        utils::ksu::send_unmountable(Path::new(target));
        active_mounts.push(target.clone());
        layer_mounts += lowerdirs.len();
    }

    Ok(layer_mounts)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn copy_entry(source: &Path, dest: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(source)?;
    if metadata.file_type().is_symlink() {
        symlink(fs::read_link(source)?, dest)?;
    } else {
        fs::copy(source, dest)?;
        fs::set_permissions(dest, metadata.permissions())?;
    }

    if let Ok(context) = utils::lgetfilecon(source)
        && let Err(err) = utils::lsetfilecon(dest, &context)
    {
        log::warn!(
            "clone selinux context skipped: src={}, dst={}, error={err}",
            source.display(),
            dest.display()
        );
    }

    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn mount_magic_phase(
    config: &Config,
    modules: &[ModuleRecord],
    plan: &MountPlan,
) -> Result<exec::MagicMountStats> {
    if plan.magic_module_ids.is_empty() {
        return Ok(exec::MagicMountStats::default());
    }

    let magic_modules: BTreeSet<String> = plan.magic_module_ids.iter().cloned().collect();
    let source_by_id: BTreeMap<String, PathBuf> = modules
        .iter()
        .map(|module| (module.id.clone(), module.source_path.clone()))
        .collect();
    let magic_rules = plan.magic_path_rules.clone();

    let path_filter = move |module_id: &str, path: &Path| -> bool {
        let Some(allowed) = magic_rules.get(module_id) else {
            return true;
        };
        let Some(root) = source_by_id.get(module_id) else {
            return true;
        };
        let Ok(relative) = path.strip_prefix(root) else {
            return true;
        };
        let relative = relative
            .components()
            .filter_map(|component| component.as_os_str().to_str())
            .collect::<Vec<_>>()
            .join("/");

        allowed
            .iter()
            .any(|allow| relative == *allow || relative.starts_with(&format!("{allow}/")))
    };

    let selection = Selection {
        modules: Some(&magic_modules),
        path_filter: Some(&path_filter),
    };
    let options = ScanOptions {
        extra_partitions: &[],
        ignore_sources: &[],
        selection,
    };

    exec::magic_mount(
        &config.moduledir,
        &config.mountsource,
        &options,
        !config.disable_umount,
    )
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
    fn shallow_dir_planning_is_stable_and_separated() {
        let first = shallow_dir_for("/system/etc", 0);
        let second = shallow_dir_for("/system/etc", 1);
        let other = shallow_dir_for("/vendor/lib", 0);

        assert_eq!(
            first,
            PathBuf::from("/data/adb/hybrid-mount/run/shallow/system_etc_0")
        );
        assert_eq!(
            second,
            PathBuf::from("/data/adb/hybrid-mount/run/shallow/system_etc_1")
        );
        assert_ne!(first, other);
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
