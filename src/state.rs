// SPDX-License-Identifier: GPL-3.0-only

//! 持久化快照:`scan.ret`(模块清单)与 `run/state.json`(启动状态快照),
//! 以及 install-state / clear-mount-errors 等 CLI 命令实现。
//!
//! 状态为启动快照而非常驻服务提供的实时状态；host 构建保留纯逻辑与单测。

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::config::{Config, Mode};
use crate::defs;
use crate::errors::Result;
use crate::plan::{MountPlan, PlanInput, build_plan};
use crate::scanner::{ModuleRecord, list_modules};

/// `modules` 命令输出的模块条目(交互契约参考上游 scanner JSON)。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppModule {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub mode: String,
    pub is_mounted: bool,
    pub enabled: bool,
    pub source_path: String,
    pub mount_error: Option<String>,
    pub suggest_ignore: bool,
    pub rules: AppModuleRules,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppModuleRules {
    /// `None` 表示继承全局默认模式，不能折叠成当前的有效模式。
    pub default_mode: Option<String>,
    pub paths: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct MountStatistics {
    pub total_mounts: usize,
    pub successful_mounts: usize,
    pub failed_mounts: usize,
    pub files_mounted: usize,
    pub symlinks_created: usize,
    pub overlayfs_mounts: usize,
    pub ignored_entries: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct ModeStats {
    pub overlayfs: usize,
    pub magicmount: usize,
}

/// 启动时生成的挂载状态快照(替代常驻实时状态)。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RunState {
    pub timestamp: u64,
    pub pid: u32,
    pub storage_mode: String,
    pub mount_point: PathBuf,
    pub overlay_modules: Vec<String>,
    pub magic_modules: Vec<String>,
    pub skip_mount_modules: Vec<String>,
    /// Deduplicated OverlayFS and Magic Mount targets for existing WebUI clients.
    pub active_mounts: Vec<String>,
    /// Successful OverlayFS targets from the same boot snapshot.
    pub overlay_active_mounts: Vec<String>,
    /// Successful Magic Mount bind and directory targets from the same boot snapshot.
    pub magic_active_mounts: Vec<String>,
    pub mount_error_modules: Vec<String>,
    pub mount_error_reasons: BTreeMap<String, String>,
    pub mount_stats: MountStatistics,
    pub mode_stats: ModeStats,
}

impl RunState {
    pub fn save(&self) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        let path = Path::new(defs::STATE_PATH);
        crate::sys::fs::atomic_write(path, json.as_bytes())
    }

    pub fn load_or_default() -> Self {
        match fs::read_to_string(defs::STATE_PATH) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_else(|err| {
                log::warn!("failed to parse state file, using default: {err}");
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        storage_mode: String,
        mount_point: PathBuf,
        overlay_modules: Vec<String>,
        magic_modules: Vec<String>,
        skip_mount_modules: Vec<String>,
        active_mounts: Vec<String>,
        overlay_active_mounts: Vec<String>,
        magic_active_mounts: Vec<String>,
        mount_stats: MountStatistics,
        mode_stats: ModeStats,
    ) -> Self {
        Self {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            pid: std::process::id(),
            storage_mode,
            mount_point,
            overlay_modules,
            magic_modules,
            skip_mount_modules,
            active_mounts,
            overlay_active_mounts,
            magic_active_mounts,
            mount_error_modules: Vec::new(),
            mount_error_reasons: BTreeMap::new(),
            mount_stats,
            mode_stats,
        }
    }

    /// Build the boot snapshot as soon as planning succeeds.  This keeps the
    /// WebUI contract available even when a later mount operation fails.
    pub fn from_plan(
        config: &Config,
        modules: &[ModuleRecord],
        plan: &MountPlan,
        mount_error_modules: Vec<String>,
    ) -> Self {
        let skip_mount_modules = modules
            .iter()
            .filter(|module| module.skip_mount)
            .map(|module| module.id.clone())
            .collect();
        let mount_error_reasons = mount_error_modules
            .iter()
            .map(|module| (module.clone(), "mount_error marker present".to_owned()))
            .collect();

        let mut state = Self::new(
            config.overlay_mode.as_str().to_owned(),
            PathBuf::new(),
            plan.overlay_module_ids.clone(),
            plan.magic_module_ids.clone(),
            skip_mount_modules,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            MountStatistics::default(),
            ModeStats {
                overlayfs: plan.overlay_module_ids.len(),
                magicmount: plan.magic_module_ids.len(),
            },
        );
        state.mount_error_modules = mount_error_modules;
        state.mount_error_reasons = mount_error_reasons;
        state
    }
}

/// 安装兼容性状态(`install-state`)。
#[derive(Debug, Clone, Serialize)]
pub struct InstallState {
    pub installed: bool,
    pub self_module: bool,
    pub binary: bool,
    pub config_exists: bool,
    pub overlay_supported: bool,
    pub mount_source: String,
    pub compatible: bool,
}

pub fn build_install_state(
    self_module: bool,
    binary: bool,
    config_exists: bool,
    overlay_supported: bool,
    mount_source: &str,
) -> InstallState {
    let compatible = self_module && binary && overlay_supported;
    let installed = self_module && binary && config_exists;

    InstallState {
        installed,
        self_module,
        binary,
        config_exists,
        overlay_supported,
        mount_source: mount_source.to_owned(),
        compatible,
    }
}

/// 由模块清单 + 配置 + 计划生成 `scan.ret` 条目。
pub fn app_modules(
    modules: &[ModuleRecord],
    config: &Config,
    plan: &MountPlan,
    mount_errors: &[String],
) -> Vec<AppModule> {
    modules
        .iter()
        .map(|module| {
            let mode = if plan.overlay_module_ids.contains(&module.id) {
                Mode::Overlay
            } else if plan.magic_module_ids.contains(&module.id) {
                Mode::Magic
            } else {
                Mode::Ignore
            };

            let mount_error = mount_errors
                .iter()
                .any(|id| id == &module.id)
                .then(|| "mount_error marker present".to_owned());

            AppModule {
                id: module.id.clone(),
                name: module.name.clone(),
                version: module.version.clone(),
                author: module.author.clone(),
                description: module.description.clone(),
                mode: mode.as_str().to_owned(),
                is_mounted: module.mountable() && mode != Mode::Ignore,
                enabled: !module.disabled,
                source_path: module.source_path.to_string_lossy().into_owned(),
                suggest_ignore: mount_error.is_some(),
                mount_error,
                rules: app_module_rules(config, &module.id),
            }
        })
        .collect()
}

fn app_module_rules(config: &Config, module_id: &str) -> AppModuleRules {
    let rule = config.rules.get(module_id);
    AppModuleRules {
        default_mode: rule
            .and_then(|rule| rule.default_mode)
            .map(|mode| mode.as_str().to_owned()),
        paths: rule
            .map(|rule| {
                rule.paths
                    .iter()
                    .map(|(path, mode)| (path.clone(), mode.as_str().to_owned()))
                    .collect()
            })
            .unwrap_or_default(),
    }
}

/// The mount result is a boot snapshot, but rules are editable at runtime. Keep
/// the mounted backend/status intact while presenting the latest saved rules.
fn sync_app_module_rules(modules: &mut [AppModule], config: &Config) {
    for module in modules {
        module.rules = app_module_rules(config, &module.id);
    }
}

pub fn write_scan_ret(modules: &[AppModule]) -> Result<()> {
    let json = serde_json::to_string_pretty(modules)?;
    let path = Path::new(defs::SCAN_RET_PATH);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    crate::sys::fs::atomic_write(path, json.as_bytes())
}

/// `modules`:输出启动时缓存的 `scan.ret`。
pub fn handle_modules() -> Result<()> {
    match fs::read_to_string(defs::SCAN_RET_PATH) {
        Ok(text) => match serde_json::from_str::<Vec<AppModule>>(&text) {
            Ok(mut modules) => {
                let config = Config::load_or_default(Path::new(defs::CONFIG_PATH));
                sync_app_module_rules(&mut modules, &config);
                if let Err(write_err) = write_scan_ret(&modules) {
                    log::warn!(
                        "failed to refresh rules in {}: {write_err}",
                        defs::SCAN_RET_PATH
                    );
                }
                println!("{}", serde_json::to_string_pretty(&modules)?);
                return Ok(());
            }
            Err(err) => log::warn!(
                "failed to parse {}, rebuilding module snapshot: {err}",
                defs::SCAN_RET_PATH
            ),
        },
        Err(err) => {
            log::warn!(
                "failed to read {}, rebuilding module snapshot: {err}",
                defs::SCAN_RET_PATH
            );
        }
    }

    let modules = rebuild_module_snapshot();
    if let Err(write_err) = write_scan_ret(&modules) {
        log::warn!(
            "failed to cache rebuilt module snapshot at {}: {write_err}",
            defs::SCAN_RET_PATH
        );
    }
    println!("{}", serde_json::to_string_pretty(&modules)?);
    Ok(())
}

fn rebuild_module_snapshot() -> Vec<AppModule> {
    let config = Config::load_or_default(Path::new(defs::CONFIG_PATH));
    let managed_partitions = defs::MANAGED_PARTITIONS
        .iter()
        .map(|partition| (*partition).to_owned())
        .collect::<Vec<_>>();
    let modules = list_modules(&config.moduledir, &managed_partitions);
    fallback_app_modules(&modules, &config)
}

fn fallback_app_modules(modules: &[ModuleRecord], config: &Config) -> Vec<AppModule> {
    let promoted_partitions = BTreeSet::new();
    let plan = build_plan(&PlanInput {
        modules,
        config,
        promoted_partitions: &promoted_partitions,
    })
    .unwrap_or_else(|err| {
        log::warn!("fallback module plan failed, returning raw module list: {err}");
        MountPlan::default()
    });
    let mount_errors = collect_mount_error_modules(&config.moduledir);
    let mut snapshot = app_modules(modules, config, &plan, &mount_errors);
    for module in &mut snapshot {
        module.is_mounted = false;
    }
    snapshot
}

/// `status`:输出 `run/state.json`(缺失时输出默认快照)。
pub fn handle_status() -> Result<()> {
    let state = RunState::load_or_default();
    println!("{}", serde_json::to_string_pretty(&state)?);
    Ok(())
}

/// `install-state`:安装兼容性状态。
pub fn handle_install_state() -> Result<()> {
    let self_module = Path::new(defs::SELF_MODULE_DIR).is_dir();
    let binary = std::env::current_exe().is_ok_and(|path| path.exists());
    let config_exists = Path::new(defs::CONFIG_PATH).exists();

    #[cfg(any(target_os = "linux", target_os = "android"))]
    let (overlay_supported, mount_source) = {
        use crate::overlayfs::utils::is_overlay_supported;
        use crate::utils::ksu;

        ksu::init();
        let supported = is_overlay_supported().unwrap_or(false);
        let source = if ksu::is_active() { "KSU" } else { "APatch" };
        (supported, source.to_owned())
    };
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    let (overlay_supported, mount_source) = (false, "unknown".to_owned());

    let state = build_install_state(
        self_module,
        binary,
        config_exists,
        overlay_supported,
        &mount_source,
    );
    println!("{}", serde_json::to_string_pretty(&state)?);
    Ok(())
}

/// 收集带 `mount_error` 标记的模块(大小写不敏感,只读)。
pub fn collect_mount_error_modules(moduledir: &Path) -> Vec<String> {
    let mut modules = Vec::new();
    let Ok(entries) = fs::read_dir(moduledir) else {
        return modules;
    };

    for entry in entries {
        let Ok(entry) = entry else {
            continue;
        };
        if !entry.file_type().is_ok_and(|file_type| file_type.is_dir()) {
            continue;
        }
        let Ok(children) = fs::read_dir(entry.path()) else {
            continue;
        };
        let has_marker = children.filter_map(std::result::Result::ok).any(|child| {
            child
                .file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case(defs::MOUNT_ERROR_FILE_NAME)
        });
        if has_marker {
            modules.push(entry.file_name().to_string_lossy().into_owned());
        }
    }

    modules.sort();
    modules
}

/// 清除模块 `mount_error` 标记,返回删除数量。只动模块根标记,不碰 system。
pub fn clear_mount_error_markers(moduledir: &Path) -> usize {
    let mut removed = 0;
    let Ok(entries) = fs::read_dir(moduledir) else {
        return 0;
    };

    for entry in entries {
        let Ok(entry) = entry else {
            continue;
        };
        if !entry.file_type().is_ok_and(|file_type| file_type.is_dir()) {
            continue;
        }
        let Ok(children) = fs::read_dir(entry.path()) else {
            continue;
        };
        for child in children.filter_map(std::result::Result::ok) {
            if !child
                .file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case(defs::MOUNT_ERROR_FILE_NAME)
            {
                continue;
            }
            match crate::sys::fs::remove_path(&child.path()) {
                Ok(()) => removed += 1,
                Err(err) => log::warn!(
                    "failed to remove mount error marker {}: {err}",
                    child.path().display()
                ),
            }
        }
    }

    removed
}

/// `clear-mount-errors`:清除标记并刷新状态快照。
pub fn handle_clear_mount_errors() -> Result<()> {
    let config = Config::load_or_default(Path::new(defs::CONFIG_PATH));
    let removed = clear_mount_error_markers(&config.moduledir);

    let mut state = RunState::load_or_default();
    state.mount_error_modules = collect_mount_error_modules(&config.moduledir);
    state.mount_error_reasons = state
        .mount_error_modules
        .iter()
        .map(|module| (module.clone(), "mount_error marker present".to_owned()))
        .collect();
    state.save()?;

    // `modules` 读取的是启动时缓存。同步清理缓存，避免 WebUI 刷新后重新出现旧错误。
    if let Ok(text) = fs::read_to_string(defs::SCAN_RET_PATH) {
        match serde_json::from_str::<Vec<AppModule>>(&text) {
            Ok(mut modules) => {
                clear_app_module_errors(&mut modules);
                write_scan_ret(&modules)?;
            }
            Err(err) => log::warn!("failed to refresh {}: {err}", defs::SCAN_RET_PATH),
        }
    }

    println!("{}", serde_json::json!({ "ok": true, "removed": removed }));
    Ok(())
}

fn clear_app_module_errors(modules: &mut [AppModule]) {
    for module in modules {
        module.mount_error = None;
        module.suggest_ignore = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(id: &str) -> ModuleRecord {
        ModuleRecord {
            id: id.to_owned(),
            name: id.to_owned(),
            version: "1".to_owned(),
            author: "a".to_owned(),
            description: "d".to_owned(),
            disabled: false,
            skip_mount: false,
            has_mount_files: true,
            source_path: PathBuf::from(format!("/data/adb/modules/{id}")),
            entries: Vec::new(),
        }
    }

    fn test_plan() -> MountPlan {
        MountPlan {
            overlay_module_ids: vec!["overlay_mod".to_owned()],
            magic_module_ids: vec!["magic_mod".to_owned()],
            ..MountPlan::default()
        }
    }

    #[test]
    fn app_modules_reflect_plan_backend_and_rules() {
        let modules = [
            record("overlay_mod"),
            record("magic_mod"),
            record("ignored_mod"),
        ];
        let mut config = Config::default();
        config.rules.insert(
            "magic_mod".to_owned(),
            crate::config::ModuleRule {
                default_mode: Some(Mode::Magic),
                paths: BTreeMap::from([("system/etc/hosts".to_owned(), Mode::Overlay)]),
            },
        );

        let list = app_modules(&modules, &config, &test_plan(), &[]);

        assert_eq!(list[0].mode, "overlay");
        assert!(list[0].is_mounted);
        assert_eq!(list[1].mode, "magic");
        assert_eq!(list[0].rules.default_mode, None);
        assert_eq!(list[1].rules.default_mode.as_deref(), Some("magic"));
        assert_eq!(list[1].rules.paths["system/etc/hosts"], "overlay");
        assert_eq!(list[2].mode, "ignore");
        assert!(!list[2].is_mounted);
    }

    #[test]
    fn cached_snapshot_keeps_boot_backend_but_refreshes_saved_rules() {
        let modules = [record("switchable")];
        let boot_config = Config::default();
        let plan = MountPlan {
            overlay_module_ids: vec!["switchable".to_owned()],
            ..MountPlan::default()
        };
        let mut snapshot = app_modules(&modules, &boot_config, &plan, &[]);

        let mut edited_config = Config::default();
        edited_config.rules.insert(
            "switchable".to_owned(),
            crate::config::ModuleRule {
                default_mode: Some(Mode::Magic),
                paths: BTreeMap::new(),
            },
        );
        sync_app_module_rules(&mut snapshot, &edited_config);

        assert_eq!(snapshot[0].mode, "overlay");
        assert!(snapshot[0].is_mounted);
        assert_eq!(snapshot[0].rules.default_mode.as_deref(), Some("magic"));
    }

    #[test]
    fn app_modules_surface_mount_error_markers() {
        let modules = [record("bad_mod")];
        let config = Config::default();
        let plan = MountPlan::default();

        let list = app_modules(&modules, &config, &plan, &["bad_mod".to_owned()]);

        assert_eq!(
            list[0].mount_error,
            Some("mount_error marker present".to_owned())
        );
        assert!(list[0].suggest_ignore);
    }

    #[test]
    fn fallback_snapshot_scans_planned_mode_without_claiming_mount_success() {
        let mut module = record("fallback_mod");
        module.entries = vec![
            crate::scanner::ModuleEntry {
                relative: "system/etc".to_owned(),
                file_type: crate::mount_tree::NodeFileType::Directory,
                replace: false,
            },
            crate::scanner::ModuleEntry {
                relative: "system/etc/hosts".to_owned(),
                file_type: crate::mount_tree::NodeFileType::RegularFile,
                replace: false,
            },
        ];

        let list = fallback_app_modules(&[module], &Config::default());

        assert_eq!(list.len(), 1);
        assert_eq!(list[0].mode, "overlay");
        assert!(!list[0].is_mounted);
    }

    #[test]
    fn cached_app_module_errors_can_be_cleared() {
        let modules = [record("bad_mod")];
        let mut list = app_modules(
            &modules,
            &Config::default(),
            &MountPlan::default(),
            &["bad_mod".to_owned()],
        );

        clear_app_module_errors(&mut list);

        assert_eq!(list[0].mount_error, None);
        assert!(!list[0].suggest_ignore);
    }

    #[test]
    fn install_state_compatibility_rules() {
        let ok = build_install_state(true, true, true, true, "KSU");
        assert!(ok.compatible && ok.installed);

        let missing_overlay = build_install_state(true, true, true, false, "KSU");
        assert!(!missing_overlay.compatible);

        let not_installed = build_install_state(true, true, false, true, "APatch");
        assert!(!not_installed.installed);
        assert_eq!(not_installed.mount_source, "APatch");
    }

    #[test]
    fn mount_error_markers_collected_and_cleared_without_touching_system() {
        let root =
            std::env::temp_dir().join(format!("hybrid-mount-state-errors-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let module = root.join("bad_mod");
        fs::create_dir_all(module.join("system/etc")).unwrap();
        fs::write(module.join("system/etc/hosts"), "data").unwrap();
        fs::write(module.join("MOUNT_ERROR"), "").unwrap();

        assert_eq!(
            collect_mount_error_modules(&root),
            vec!["bad_mod".to_owned()]
        );

        let removed = clear_mount_error_markers(&root);
        assert_eq!(removed, 1);
        assert!(collect_mount_error_modules(&root).is_empty());
        // system 内容未被触碰
        assert_eq!(
            fs::read_to_string(module.join("system/etc/hosts")).unwrap(),
            "data"
        );

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn run_state_roundtrips_through_json() {
        let state = RunState::new(
            "ext4".to_owned(),
            PathBuf::from("/data/adb/hybrid-mount/run"),
            vec!["a".to_owned()],
            vec!["b".to_owned()],
            vec!["c".to_owned()],
            vec!["/system".to_owned(), "/vendor/lib/demo.so".to_owned()],
            vec!["/system".to_owned()],
            vec!["/vendor/lib/demo.so".to_owned()],
            MountStatistics {
                overlayfs_mounts: 1,
                ..MountStatistics::default()
            },
            ModeStats {
                overlayfs: 1,
                magicmount: 1,
            },
        );

        let json = serde_json::to_string(&state).unwrap();
        let parsed: RunState = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.overlay_modules, vec!["a".to_owned()]);
        assert_eq!(parsed.magic_modules, vec!["b".to_owned()]);
        assert_eq!(parsed.storage_mode, "ext4");
        assert_eq!(
            parsed.active_mounts,
            vec!["/system".to_owned(), "/vendor/lib/demo.so".to_owned()]
        );
        assert_eq!(parsed.overlay_active_mounts, vec!["/system".to_owned()]);
        assert_eq!(
            parsed.magic_active_mounts,
            vec!["/vendor/lib/demo.so".to_owned()]
        );
    }

    #[test]
    fn legacy_run_state_defaults_backend_specific_mount_lists() {
        let json = r#"{
            "timestamp": 1,
            "active_mounts": ["/system"]
        }"#;

        let parsed: RunState = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.active_mounts, vec!["/system".to_owned()]);
        assert!(parsed.overlay_active_mounts.is_empty());
        assert!(parsed.magic_active_mounts.is_empty());
    }

    #[test]
    fn planned_run_state_exposes_backend_before_mounting() {
        let mut skipped = record("skipped_mod");
        skipped.skip_mount = true;
        let config = Config::default();
        let plan = test_plan();

        let state = RunState::from_plan(
            &config,
            &[record("overlay_mod"), record("magic_mod"), skipped],
            &plan,
            vec!["overlay_mod".to_owned()],
        );

        assert_eq!(state.overlay_modules, vec!["overlay_mod".to_owned()]);
        assert_eq!(state.magic_modules, vec!["magic_mod".to_owned()]);
        assert_eq!(state.skip_mount_modules, vec!["skipped_mod".to_owned()]);
        assert!(state.mount_point.as_os_str().is_empty());
        assert!(state.active_mounts.is_empty());
        assert_eq!(state.mount_stats, MountStatistics::default());
        assert_eq!(state.mode_stats.overlayfs, 1);
        assert_eq!(state.mode_stats.magicmount, 1);
        assert_eq!(
            state.mount_error_reasons["overlay_mod"],
            "mount_error marker present"
        );
    }
}
