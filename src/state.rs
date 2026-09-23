// SPDX-License-Identifier: GPL-3.0-only

//! Persisted snapshots: `scan.ret` (module list) and `run/state.json` (boot state),
//! plus the install-state and clear-mount-errors CLI commands.
//!
//! State is a boot snapshot, not a live service; host builds keep the pure logic and its tests.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::config::{Config, Mode};
use crate::defs;
use crate::errors::{CausalError, ContextError, Error, Result};
use crate::module_id::ModuleId;
use crate::plan::{MountPlan, PlanInput, build_plan};
use crate::scanner::{ModuleRecord, list_modules};

/// A module entry in the `modules` command output, and its JSON contract.
/// `id` is still validated on deserialisation; the wire format stays a plain JSON string.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppModule {
    pub id: ModuleId,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub mode: String,
    pub is_mounted: bool,
    pub enabled: bool,
    /// Whether this module is blocked by the bundled or persistent blacklist.
    /// Missing in older `scan.ret` snapshots, where it defaults to `false`.
    #[serde(default)]
    pub blacklisted: bool,
    pub source_path: String,
    pub mount_error: Option<String>,
    pub suggest_ignore: bool,
    pub rules: AppModuleRules,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppModuleRules {
    /// `None` inherits the global default mode and must not be collapsed into the effective mode.
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
    /// Magic Mount directory targets (Move, Replace). Separate from `files_mounted` so the
    /// WebUI can distinguish file binds from full-directory mounts.
    pub magic_dirs: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct ModeStats {
    pub overlayfs: usize,
    pub magicmount: usize,
    pub vfs: usize,
}

/// Provenance of `run/state.json`. A corrupt state must be visible in the `status` JSON,
/// not merely logged before silently falling back to the default.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StateLoadKind {
    #[default]
    Missing,
    Loaded,
    Corrupt,
    IoError,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct StateLoadInfo {
    pub kind: StateLoadKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl StateLoadInfo {
    pub fn loaded() -> Self {
        Self {
            kind: StateLoadKind::Loaded,
            detail: None,
        }
    }

    fn corrupt(detail: impl Into<String>) -> Self {
        Self {
            kind: StateLoadKind::Corrupt,
            detail: Some(detail.into()),
        }
    }

    fn io_error(detail: impl Into<String>) -> Self {
        Self {
            kind: StateLoadKind::IoError,
            detail: Some(detail.into()),
        }
    }
}

/// The mount state snapshot written at boot, replacing a resident live state.
///
/// Every new field carries a serde default so old `run/state.json` files and old WebUI
/// clients keep working; failure diagnostics serialise only on failure.
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
    /// Deduplicated active targets from every backend for existing WebUI clients.
    pub active_mounts: Vec<String>,
    /// Successful OverlayFS targets from the same boot snapshot.
    pub overlay_active_mounts: Vec<String>,
    /// Successful Magic Mount bind and directory targets from the same boot snapshot.
    pub magic_active_mounts: Vec<String>,
    /// VFS-injected modules and successful targets. VFS is not a real kernel mount, so
    /// it stays out of the KSU try-umount list, but its targets are counted as active
    /// mount points in `active_mounts`.
    pub vfs_modules: Vec<String>,
    pub vfs_active_mounts: Vec<String>,
    /// The provider actually bound this boot (v2 has only `hm`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vfs_provider: Option<String>,
    /// Explicit VFS failure detail when the backend was requested but could not be confirmed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vfs_error: Option<String>,
    /// VFS modules whose planned rules were not applied or confirmed.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub vfs_error_modules: Vec<String>,
    /// One-way guard result: whether a foreign NoMount implementation exists on the device.
    #[serde(default)]
    pub vfs_foreign_nomount: bool,
    /// Final confirmed targets: mountinfo-confirmed for OverlayFS and Magic Mount,
    /// provider read-back confirmed for VFS. Executor attempts stay in `mount_stats`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub confirmed_active_mounts: Vec<String>,
    pub mount_error_modules: Vec<String>,
    pub mount_error_reasons: BTreeMap<String, String>,
    pub mount_stats: MountStatistics,
    pub mode_stats: ModeStats,
    /// State file provenance for this `status` output; defaulting to `missing` for old clients.
    pub state_load: StateLoadInfo,
    /// The failing phase, such as `mount_execution`, `state_save` or `mount_transaction_commit`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_stage: Option<String>,
    /// Displayable boot failure reason; old snapshots default to `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
    /// `pending_commit` / `committed` / `clean` / `incomplete` / `unverified`。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rollback_status: Option<String>,
    /// Mount targets still present after rollback.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub leftover_mount_targets: Vec<String>,
}

impl RunState {
    pub fn save(&self) -> Result<()> {
        self.save_to(Path::new(defs::STATE_PATH))
    }

    pub(crate) fn save_to(&self, path: &Path) -> Result<()> {
        if crate::sys::faults::should_fail_state_save() {
            return Err(Error::State(Box::new(ContextError::new(
                "save run state",
                Some(path.to_path_buf()),
                CausalError::Message("injected state save failure".to_owned()),
            ))));
        }
        let json = serde_json::to_string_pretty(self).map_err(|source| {
            Error::State(Box::new(ContextError::new(
                "serialize run state",
                Some(path.to_path_buf()),
                source,
            )))
        })?;
        crate::sys::fs::atomic_write(path, json.as_bytes()).map_err(|source| {
            Error::State(Box::new(ContextError::new(
                "atomically save run state",
                Some(path.to_path_buf()),
                source.to_string(),
            )))
        })
    }

    pub fn load_or_default() -> Self {
        Self::load_from(Path::new(defs::STATE_PATH))
    }

    pub(crate) fn load_from(path: &Path) -> Self {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Self::default();
            }
            Err(err) => {
                let state = Self {
                    state_load: StateLoadInfo::io_error(format!("read {}: {err}", path.display())),
                    ..Self::default()
                };
                log::warn!(
                    "state file read failed, using default with diagnostic: path={}, error={err}",
                    path.display()
                );
                return state;
            }
        };

        match serde_json::from_str::<Self>(&text) {
            Ok(mut state) => {
                state.state_load = StateLoadInfo::loaded();
                state
            }
            Err(err) => {
                let state = Self {
                    state_load: StateLoadInfo::corrupt(format!("parse {}: {err}", path.display())),
                    ..Self::default()
                };
                log::warn!(
                    "state file is corrupt, using default with diagnostic: path={}, error={err}",
                    path.display()
                );
                state
            }
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
            vfs_modules: Vec::new(),
            vfs_active_mounts: Vec::new(),
            vfs_provider: None,
            vfs_error: None,
            vfs_error_modules: Vec::new(),
            vfs_foreign_nomount: false,
            confirmed_active_mounts: Vec::new(),
            mount_error_modules: Vec::new(),
            mount_error_reasons: BTreeMap::new(),
            mount_stats,
            mode_stats,
            state_load: StateLoadInfo::loaded(),
            failed_stage: None,
            failure_reason: None,
            rollback_status: None,
            leftover_mount_targets: Vec::new(),
        }
    }

    /// Build the boot snapshot as soon as planning succeeds.  This keeps the
    /// WebUI contract available even when a later mount operation fails.
    pub fn from_plan(
        _config: &Config,
        modules: &[ModuleRecord],
        plan: &MountPlan,
        mount_error_modules: Vec<String>,
    ) -> Self {
        let skip_mount_modules = modules
            .iter()
            .filter(|module| module.skip_mount)
            .map(|module| module.id.to_string())
            .collect();
        let mount_error_reasons = mount_error_modules
            .iter()
            .map(|module| (module.clone(), "mount_error marker present".to_owned()))
            .collect();

        let mut state = Self::new(
            // No overlay staging exists yet at plan time; the storage phase replaces this
            // with the real Tmpfs/Ext4 mode, or leaves the sentinel for VFS-only boots.
            crate::defs::NO_STORAGE_MODE.to_owned(),
            PathBuf::new(),
            plan.overlay_module_ids
                .iter()
                .map(ModuleId::to_string)
                .collect(),
            plan.magic_module_ids
                .iter()
                .map(ModuleId::to_string)
                .collect(),
            skip_mount_modules,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            MountStatistics::default(),
            ModeStats {
                overlayfs: plan.overlay_module_ids.len(),
                magicmount: plan.magic_module_ids.len(),
                vfs: plan.vfs_module_ids.len(),
            },
        );
        state.vfs_modules = plan
            .vfs_module_ids
            .iter()
            .map(ModuleId::to_string)
            .collect();
        state.mount_error_modules = mount_error_modules;
        state.mount_error_reasons = mount_error_reasons;
        state
    }

    /// Build a fresh fail-closed snapshot for errors that occur before any
    /// mount side effect or module snapshot can be created.
    pub fn from_startup_failure(stage: &str, reason: impl Into<String>) -> Self {
        let mut state = Self::new(
            crate::defs::NO_STORAGE_MODE.to_owned(),
            PathBuf::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            MountStatistics::default(),
            ModeStats::default(),
        );
        state.failed_stage = Some(stage.to_owned());
        state.failure_reason = Some(reason.into());
        state.rollback_status = Some("clean".to_owned());
        state
    }
}

/// Install compatibility state (`install-state`).
#[derive(Debug, Clone, Serialize)]
pub struct InstallState {
    pub installed: bool,
    pub self_module: bool,
    pub binary: bool,
    pub config_exists: bool,
    pub overlay_supported: bool,
    pub tmpfs_supported: bool,
    /// Current ext4 staging concealment result; None means it has not been verified.
    pub nuke_supported: Option<bool>,
    /// Nuke backend used by this installation: `ksud` for KernelSU, or APatch for non-KSU.
    pub nuke_type: String,
    /// Live protocol probe: includes built-in providers and excludes failed LKM loads.
    pub vfs_supported: bool,
    /// How the provider is present on the device: `lkm` while `/proc/modules` lists
    /// `hybridmount`, `builtin` while it lives in the kernel image, and `unknown` when neither
    /// table entry exists. Independent of [`Self::vfs_supported`], which additionally requires
    /// the key type to answer this boot.
    pub vfs_type: String,
    pub mount_source: String,
    pub compatible: bool,
}

/// Label for the [`InstallState::vfs_type`] field, from the module tables alone.
pub fn vfs_type_label(presence: crate::vfs::doctor::ModulePresence) -> &'static str {
    use crate::vfs::doctor::ModulePresence;

    match presence {
        ModulePresence::Loadable => "lkm",
        ModulePresence::BuiltIn => "builtin",
        ModulePresence::NotPresent => "unknown",
    }
}

pub fn build_install_state(
    self_module: bool,
    binary: bool,
    config_exists: bool,
    overlay_supported: bool,
    vfs_supported: bool,
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
        tmpfs_supported: false,
        nuke_supported: None,
        nuke_type: "unknown".to_owned(),
        vfs_supported,
        vfs_type: "unknown".to_owned(),
        mount_source: mount_source.to_owned(),
        compatible,
    }
}

/// Builds `scan.ret` entries from the module list, config, plan and final mounted targets.
///
/// `mounted_module_ids` must come from the execution result, never from the plan's selection;
/// an empty set means nothing is mounted yet, or everything was rolled back.
pub fn app_modules(
    modules: &[ModuleRecord],
    config: &Config,
    plan: &MountPlan,
    mount_errors: &[String],
    mounted_module_ids: &BTreeSet<String>,
) -> Vec<AppModule> {
    modules
        .iter()
        .map(|module| {
            let blacklisted = config.is_module_blacklisted(module.id.as_str());
            let mode = if blacklisted {
                Mode::Ignore
            } else if plan.overlay_module_ids.contains(&module.id) {
                Mode::Overlay
            } else if plan.magic_module_ids.contains(&module.id) {
                Mode::Magic
            } else if plan.vfs_module_ids.contains(&module.id) {
                Mode::Vfs
            } else {
                Mode::Ignore
            };

            let mount_error = mount_errors
                .iter()
                .any(|id| id == module.id.as_str())
                .then(|| "mount_error marker present".to_owned());

            AppModule {
                id: module.id.clone(),
                name: module.name.clone(),
                version: module.version.clone(),
                author: module.author.clone(),
                description: module.description.clone(),
                mode: mode.as_str().to_owned(),
                is_mounted: !blacklisted && mounted_module_ids.contains(module.id.as_str()),
                enabled: !module.disabled && !blacklisted,
                blacklisted,
                source_path: module.source_path.to_string_lossy().into_owned(),
                suggest_ignore: mount_error.is_some(),
                mount_error,
                rules: app_module_rules(config, &module.id),
            }
        })
        .collect()
}

/// Derives which modules actually mounted from the final successful targets.
///
/// The plan's `overlay_module_ids`/`magic_module_ids` only say what was chosen and
/// must not stand in for `is_mounted`, so the executed target list is the input here:
/// - a directory-level overlay target reads that node's own contribution;
/// - a shallow overlay target reads its whole subtree's contributions;
/// - a magic target reads that node's own magic sources.
pub fn mounted_module_ids_for_snapshot(
    modules: &[ModuleRecord],
    plan: &MountPlan,
    overlay_targets: &[String],
    magic_targets: &[String],
) -> BTreeSet<String> {
    let mut mounted = BTreeSet::new();

    for target in overlay_targets {
        if plan.overlay_files.contains_key(target) {
            mounted.extend(
                plan.tree
                    .module_ids_for_subtree(Mode::Overlay, target)
                    .into_iter()
                    .map(ModuleId::to_string),
            );
        } else {
            mounted.extend(
                plan.tree
                    .module_ids_for_target(Mode::Overlay, target)
                    .into_iter()
                    .map(ModuleId::to_string),
            );
        }
    }

    for target in magic_targets {
        mounted.extend(
            plan.tree
                .module_ids_for_target(Mode::Magic, target)
                .into_iter()
                .map(ModuleId::to_string),
        );
    }

    // Report only modules actually scanned here, filtering stale ids left in the tree.
    let scanned = modules
        .iter()
        .map(|module| module.id.as_str())
        .collect::<BTreeSet<_>>();
    mounted.retain(|module| scanned.contains(module.as_str()));
    mounted
}

fn app_module_rules(config: &Config, module_id: &ModuleId) -> AppModuleRules {
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
        module.blacklisted = config.is_module_blacklisted(module.id.as_str());
        if module.blacklisted {
            module.enabled = false;
        }
        module.rules = app_module_rules(config, &module.id);
    }
}

pub fn write_scan_ret(modules: &[AppModule]) -> Result<()> {
    write_scan_ret_to(modules, Path::new(defs::SCAN_RET_PATH))
}

pub(crate) fn write_scan_ret_to(modules: &[AppModule], path: &Path) -> Result<()> {
    let json = serde_json::to_string_pretty(modules)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    crate::sys::fs::atomic_write(path, json.as_bytes())
}

/// Merge live installation metadata into the committed runtime snapshot. A
/// disable/blacklist edit changes the next boot, never already active resources.
fn merge_module_snapshot(
    cached: Vec<AppModule>,
    installed: &[ModuleRecord],
    config: &Config,
) -> Vec<AppModule> {
    let mut cached: BTreeMap<_, _> = cached
        .into_iter()
        .map(|module| (module.id.clone(), module))
        .collect();
    let mut merged = fallback_app_modules(installed, config);
    for module in &mut merged {
        if let Some(previous) = cached.remove(&module.id) {
            module.mode = previous.mode;
            module.is_mounted = previous.is_mounted;
        }
    }
    // Removed sources can still have pinned VFS rules or mounts. Keep their
    // ownership visible until an explicit unload or cleanup removes it.
    for mut removed in cached.into_values().filter(|module| module.is_mounted) {
        removed.enabled = false;
        merged.push(removed);
    }
    sync_app_module_rules(&mut merged, config);
    merged
}

fn query_module_snapshot(
    path: &Path,
    installed: &[ModuleRecord],
    config: &Config,
) -> Vec<AppModule> {
    let cached = match fs::read_to_string(path) {
        Ok(text) => match serde_json::from_str::<Vec<AppModule>>(&text) {
            Ok(modules) => modules,
            Err(err) => {
                log::warn!(
                    "failed to parse {}, rebuilding module view: {err}",
                    path.display()
                );
                Vec::new()
            }
        },
        Err(err) => {
            log::warn!(
                "failed to read {}, rebuilding module view: {err}",
                path.display()
            );
            Vec::new()
        }
    };
    merge_module_snapshot(cached, installed, config)
}

/// `modules`: combine committed runtime ownership with current installed metadata.
/// A query never rewrites the cache: a concurrent hot operation owns that snapshot.
pub fn handle_modules() -> Result<()> {
    let config = Config::load_or_default(Path::new(defs::CONFIG_PATH))?;
    let managed_partitions = defs::MANAGED_PARTITIONS
        .iter()
        .map(|partition| (*partition).to_owned())
        .collect::<Vec<_>>();
    let installed = list_modules(&config.moduledir, &managed_partitions)?;
    let modules = query_module_snapshot(Path::new(defs::SCAN_RET_PATH), &installed, &config);
    println!("{}", serde_json::to_string_pretty(&modules)?);
    Ok(())
}

fn fallback_app_modules(modules: &[ModuleRecord], config: &Config) -> Vec<AppModule> {
    let promoted_partitions = BTreeSet::new();
    let plan = build_plan(&PlanInput {
        modules,
        config,
        promoted_partitions: &promoted_partitions,
        vfs_available: crate::vfs::available(),
    })
    .unwrap_or_else(|err| {
        log::warn!("fallback module plan failed, returning raw module list: {err}");
        MountPlan::default()
    });
    let mount_errors = collect_mount_error_modules(&config.moduledir);
    app_modules(modules, config, &plan, &mount_errors, &BTreeSet::new())
}

/// `status`: outputs `run/state.json`, or the default snapshot when absent.
pub fn handle_status() -> Result<()> {
    let state = RunState::load_or_default();
    println!("{}", serde_json::to_string_pretty(&state)?);
    Ok(())
}

/// `install-state`: install compatibility state.
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
        let source = crate::pipeline::effective_mount_source(ksu::is_active());
        (supported, source.to_owned())
    };
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    let (overlay_supported, mount_source) = (false, "unknown".to_owned());

    let mut state = build_install_state(
        self_module,
        binary,
        config_exists,
        overlay_supported,
        crate::vfs::available(),
        &mount_source,
    );
    state.tmpfs_supported = crate::sys::fs::is_overlay_xattr_supported().unwrap_or(false);
    // Read-only module-table classification; never triggers an insmod.
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        state.vfs_type = vfs_type_label(crate::vfs::doctor::presence_on_device()).to_owned();
    }
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        state.nuke_type = if crate::utils::ksu::is_active() {
            "ksud".to_owned()
        } else {
            "apatch".to_owned()
        };
        let run = RunState::load_or_default();
        if !crate::utils::ksu::is_active()
            && run.storage_mode == "ext4"
            && !run.mount_point.as_os_str().is_empty()
        {
            state.nuke_supported = crate::sys::nuke::concealment_status(&run.mount_point);
        }
    }
    println!("{}", serde_json::to_string_pretty(&state)?);
    Ok(())
}

/// Collects modules carrying a `mount_error` marker (case-insensitive, read-only).
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

/// Clears module `mount_error` markers and returns how many were deleted. Only marker files are removed, never directories.
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

            let marker_path = child.path();
            match child.file_type() {
                Ok(file_type) if file_type.is_file() => match fs::remove_file(&marker_path) {
                    Ok(()) => {
                        removed += 1;
                        log::info!("cleared mount_error marker: {}", marker_path.display());
                    }
                    Err(err) => log::warn!(
                        "failed to remove mount_error marker {}: {err}",
                        marker_path.display()
                    ),
                },
                Ok(_) => log::warn!(
                    "mount_error is not a regular file: {}",
                    marker_path.display()
                ),
                Err(err) => log::warn!(
                    "failed to check mount_error marker {}: {err}",
                    marker_path.display()
                ),
            }
        }
    }

    removed
}

/// `clear-mount-errors`: clears the markers and refreshes the state snapshot.
pub fn handle_clear_mount_errors() -> Result<()> {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    let _operation = crate::runtime::ledger::OperationLock::acquire()?;
    let config = Config::load_or_default(Path::new(defs::CONFIG_PATH))?;
    let removed = clear_mount_error_markers(&config.moduledir);

    let mut state = RunState::load_or_default();
    state.state_load = StateLoadInfo::loaded();
    state.mount_error_modules = collect_mount_error_modules(&config.moduledir);
    state.mount_error_reasons = state
        .mount_error_modules
        .iter()
        .map(|module| (module.clone(), "mount_error marker present".to_owned()))
        .collect();
    state.save()?;

    // `modules` reads the boot-time cache, so clear it too or the WebUI will show the old errors again.
    if let Ok(text) = fs::read_to_string(defs::SCAN_RET_PATH) {
        match serde_json::from_str::<Vec<AppModule>>(&text) {
            Ok(mut modules) => {
                clear_app_module_errors(&mut modules, &state.mount_error_modules);
                write_scan_ret(&modules)?;
            }
            Err(err) => log::warn!("failed to refresh {}: {err}", defs::SCAN_RET_PATH),
        }
    }

    println!("{}", clear_errors_payload(removed));
    Ok(())
}

fn clear_errors_payload(removed: usize) -> String {
    serde_json::json!({ "ok": true, "removed": removed }).to_string()
}

fn clear_app_module_errors(modules: &mut [AppModule], remaining_errors: &[String]) {
    for module in modules {
        module.mount_error = remaining_errors
            .iter()
            .any(|id| id == module.id.as_str())
            .then(|| "mount_error marker present".to_owned());
        module.suggest_ignore = module.mount_error.is_some();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(id: &str) -> ModuleRecord {
        ModuleRecord {
            id: ModuleId::try_from(id).unwrap(),
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
            overlay_module_ids: vec![ModuleId::try_from("overlay_mod").unwrap()],
            magic_module_ids: vec![ModuleId::try_from("magic_mod").unwrap()],
            ..MountPlan::default()
        }
    }

    #[test]
    fn mode_stats_exposes_vfs_counter() {
        let json = serde_json::to_string(&ModeStats::default()).unwrap();
        assert!(json.contains("\"vfs\":0"), "json was {json}");
    }

    #[test]
    fn run_state_defaults_vfs_provider_to_none() {
        let state = RunState::default();
        assert!(state.vfs_provider.is_none());
        assert!(!state.vfs_foreign_nomount);
        assert!(state.vfs_modules.is_empty());
        assert!(state.vfs_active_mounts.is_empty());
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
            ModuleId::try_from("magic_mod").unwrap(),
            crate::config::ModuleRule {
                default_mode: Some(Mode::Magic),
                paths: BTreeMap::from([("system/etc/hosts".to_owned(), Mode::Overlay)]),
            },
        );

        let list = app_modules(
            &modules,
            &config,
            &test_plan(),
            &[],
            &BTreeSet::from(["overlay_mod".to_owned(), "magic_mod".to_owned()]),
        );

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
            overlay_module_ids: vec![ModuleId::try_from("switchable").unwrap()],
            ..MountPlan::default()
        };
        let mut snapshot = app_modules(
            &modules,
            &boot_config,
            &plan,
            &[],
            &BTreeSet::from(["switchable".to_owned()]),
        );

        let mut edited_config = Config::default();
        edited_config.rules.insert(
            ModuleId::try_from("switchable").unwrap(),
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
    fn query_merge_refreshes_metadata_and_enablement_without_changing_runtime_ownership() {
        let original = record("existing");
        let config = Config::default();
        let plan = MountPlan {
            vfs_module_ids: vec![original.id.clone()],
            ..MountPlan::default()
        };
        let cached = app_modules(
            std::slice::from_ref(&original),
            &config,
            &plan,
            &[],
            &BTreeSet::from(["existing".into()]),
        );
        let mut installed = original;
        installed.name = "updated name".into();
        installed.version = "2".into();
        installed.disabled = true;
        let mut edited = config;
        edited.rules.insert(
            installed.id.clone(),
            crate::config::ModuleRule {
                default_mode: Some(Mode::Magic),
                paths: BTreeMap::new(),
            },
        );
        edited.module_blacklist.insert(installed.id.clone());
        let merged = merge_module_snapshot(cached, &[installed], &edited);
        assert_eq!(merged[0].name, "updated name");
        assert_eq!(merged[0].version, "2");
        assert!(!merged[0].enabled);
        assert!(merged[0].blacklisted);
        assert_eq!(merged[0].rules.default_mode.as_deref(), Some("magic"));
        assert!(merged[0].is_mounted);
        assert_eq!(merged[0].mode, "vfs");
    }

    #[test]
    fn query_merge_exposes_new_modules_without_claiming_they_are_mounted() {
        let mut installed = record("new_module");
        installed.entries.push(crate::scanner::ModuleEntry {
            relative: "system/etc/new".into(),
            file_type: crate::mount_tree::NodeFileType::RegularFile,
            replace: false,
        });
        let merged = merge_module_snapshot(Vec::new(), &[installed], &Config::default());
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].id, "new_module");
        assert!(merged[0].enabled);
        assert!(!merged[0].is_mounted);
    }

    #[test]
    fn query_merge_keeps_removed_active_modules_for_unload_and_drops_inactive_ones() {
        let records = [record("active"), record("inactive")];
        let plan = MountPlan {
            vfs_module_ids: vec![records[0].id.clone()],
            ..MountPlan::default()
        };
        let cached = app_modules(
            &records,
            &Config::default(),
            &plan,
            &[],
            &BTreeSet::from(["active".into()]),
        );
        let merged = merge_module_snapshot(cached, &[], &Config::default());
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].id, "active");
        assert!(merged[0].is_mounted);
        assert!(!merged[0].enabled);
        assert_eq!(merged[0].mode, "vfs");
    }

    #[test]
    fn module_query_keeps_committed_cache_bytes_unchanged() {
        let fixture = crate::test_support::Fixture::new("module-query-readonly");
        let path = fixture.join("scan.ret");
        let original = record("module");
        let cached = app_modules(
            std::slice::from_ref(&original),
            &Config::default(),
            &MountPlan::default(),
            &[],
            &BTreeSet::new(),
        );
        write_scan_ret_to(&cached, &path).unwrap();
        let committed = fs::read(&path).unwrap();
        let mut updated = original;
        updated.name = "live metadata".into();
        let view = query_module_snapshot(&path, &[updated], &Config::default());
        assert_eq!(view[0].name, "live metadata");
        assert_eq!(fs::read(path).unwrap(), committed);
    }

    #[test]
    fn module_query_does_not_create_missing_cache() {
        let fixture = crate::test_support::Fixture::new("module-query-missing");
        let path = fixture.join("scan.ret");
        let view = query_module_snapshot(&path, &[record("installed")], &Config::default());
        assert_eq!(view.len(), 1);
        assert!(!path.exists());
    }

    #[test]
    fn app_modules_surface_mount_error_markers() {
        let modules = [record("bad_mod")];
        let config = Config::default();
        let plan = MountPlan::default();

        let list = app_modules(
            &modules,
            &config,
            &plan,
            &["bad_mod".to_owned()],
            &BTreeSet::new(),
        );

        assert_eq!(
            list[0].mount_error,
            Some("mount_error marker present".to_owned())
        );
        assert!(list[0].suggest_ignore);
    }

    #[test]
    fn app_modules_surface_blacklist_status_and_skip_mounting() {
        let modules = [record("blocked")];
        let mut config = Config::default();
        config
            .module_blacklist
            .insert(ModuleId::try_from("blocked").unwrap());
        let plan = MountPlan {
            overlay_module_ids: vec![ModuleId::try_from("blocked").unwrap()],
            ..MountPlan::default()
        };

        let list = app_modules(
            &modules,
            &config,
            &plan,
            &[],
            &BTreeSet::from(["blocked".to_owned()]),
        );

        assert!(list[0].blacklisted);
        assert_eq!(list[0].mode, "ignore");
        assert!(!list[0].enabled);
        assert!(!list[0].is_mounted);
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
            &BTreeSet::new(),
        );

        clear_app_module_errors(&mut list, &[]);

        assert_eq!(list[0].mount_error, None);
        assert!(!list[0].suggest_ignore);
    }

    #[test]
    fn clearing_markers_preserves_non_regular_entries_and_their_cached_errors() {
        let root = std::env::temp_dir().join(format!(
            "hybrid-mount-state-remaining-errors-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("cleared_mod")).unwrap();
        fs::write(root.join("cleared_mod/mount_error"), "").unwrap();
        fs::create_dir_all(root.join("blocked_mod/mount_error")).unwrap();
        fs::write(root.join("blocked_mod/mount_error/keep"), "data").unwrap();
        let modules = [record("cleared_mod"), record("blocked_mod")];
        let mut list = app_modules(
            &modules,
            &Config::default(),
            &MountPlan::default(),
            &collect_mount_error_modules(&root),
            &BTreeSet::new(),
        );

        assert_eq!(clear_mount_error_markers(&root), 1);
        let remaining = collect_mount_error_modules(&root);
        clear_app_module_errors(&mut list, &remaining);

        assert_eq!(remaining, vec!["blocked_mod".to_owned()]);
        assert_eq!(list[0].mount_error, None);
        assert!(!list[0].suggest_ignore);
        assert!(list[1].mount_error.is_some());
        assert!(list[1].suggest_ignore);
        assert_eq!(
            fs::read_to_string(root.join("blocked_mod/mount_error/keep")).unwrap(),
            "data"
        );
        fs::remove_dir_all(root).ok();
    }

    #[cfg(unix)]
    #[test]
    fn clearing_markers_does_not_follow_symlinks() {
        use std::os::unix::fs::symlink;

        let root = std::env::temp_dir().join(format!(
            "hybrid-mount-state-symlink-errors-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("bad_mod")).unwrap();
        let target = root.join("keep");
        fs::write(&target, "data").unwrap();
        symlink(&target, root.join("bad_mod/Mount_Error")).unwrap();

        assert_eq!(clear_mount_error_markers(&root), 0);
        assert_eq!(collect_mount_error_modules(&root), vec!["bad_mod"]);
        assert_eq!(fs::read_to_string(target).unwrap(), "data");
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn install_state_compatibility_rules() {
        let ok = build_install_state(true, true, true, true, false, "KSU");
        assert!(ok.compatible && ok.installed);

        let missing_overlay = build_install_state(true, true, true, false, false, "KSU");
        assert!(!missing_overlay.compatible);

        let not_installed = build_install_state(true, true, false, true, false, "APatch");
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
        assert_eq!(
            fs::read_to_string(module.join("system/etc/hosts")).unwrap(),
            "data"
        );

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn app_module_id_keeps_string_wire_format_and_validates_on_read() {
        let json = serde_json::to_string(&AppModule {
            id: ModuleId::try_from("hosts").unwrap(),
            name: "H".to_owned(),
            version: "1".to_owned(),
            author: "a".to_owned(),
            description: "d".to_owned(),
            mode: "overlay".to_owned(),
            is_mounted: true,
            enabled: true,
            blacklisted: false,
            source_path: "/data/adb/modules/hosts".to_owned(),
            mount_error: None,
            suggest_ignore: false,
            rules: AppModuleRules {
                default_mode: None,
                paths: BTreeMap::new(),
            },
        })
        .unwrap();
        assert!(json.contains(r#""id":"hosts""#), "{json}");

        let parsed: AppModule = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "hosts");

        let invalid_json = json.replace(r#""id":"hosts""#, r#""id":"1bad""#);
        let err = serde_json::from_str::<AppModule>(&invalid_json).unwrap_err();
        assert!(err.to_string().contains("Invalid module ID"), "{err}");
    }

    #[test]
    fn state_save_failure_injection_is_one_shot() {
        let _fault_guard = crate::sys::faults::test_lock();
        let dir =
            std::env::temp_dir().join(format!("hybrid-mount-state-fault-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("state.json");
        let state = RunState::default();

        crate::sys::faults::enable_state_save_failure();
        let err = state.save_to(&path).unwrap_err();
        assert!(err.to_string().contains("injected state save"), "{err}");
        crate::sys::faults::reset();

        state.save_to(&path).unwrap();
        assert!(path.exists());
        fs::remove_dir_all(&dir).ok();
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
                vfs: 0,
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
        // The planned snapshot never claims an overlay storage backend before the
        // storage phase actually creates one.
        assert_eq!(state.storage_mode, crate::defs::NO_STORAGE_MODE);
        assert_eq!(state.mount_stats, MountStatistics::default());
        assert_eq!(state.mode_stats.overlayfs, 1);
        assert_eq!(state.mode_stats.magicmount, 1);
        assert_eq!(
            state.mount_error_reasons["overlay_mod"],
            "mount_error marker present"
        );
    }

    #[test]
    fn startup_failure_state_contains_no_stale_mount_success() {
        let state = RunState::from_startup_failure("config", "parse config failed");
        let wire = serde_json::to_value(&state).unwrap();

        assert_eq!(
            (
                state.storage_mode.as_str(),
                state.failed_stage.as_deref(),
                state.failure_reason.as_deref(),
                state.rollback_status.as_deref(),
                state.mount_stats.total_mounts,
                state.mount_stats.failed_mounts,
                state.active_mounts,
                state.overlay_modules,
                state.magic_modules,
                wire["failure_reason"].as_str(),
            ),
            (
                "none",
                Some("config"),
                Some("parse config failed"),
                Some("clean"),
                0,
                0,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Some("parse config failed"),
            )
        );
    }

    #[test]
    fn app_modules_mounted_flags_follow_executed_targets_not_plan() {
        let modules = [record("overlay_mod"), record("magic_mod")];
        let config = Config::default();
        let plan = test_plan();
        let mount_errors: &[String] = &[];

        let planned = app_modules(&modules, &config, &plan, mount_errors, &BTreeSet::new());
        assert!(!planned[0].is_mounted);
        assert!(!planned[1].is_mounted);

        let overlay_only = app_modules(
            &modules,
            &config,
            &plan,
            mount_errors,
            &BTreeSet::from(["overlay_mod".to_owned()]),
        );
        assert!(overlay_only[0].is_mounted);
        assert!(!overlay_only[1].is_mounted);
    }

    #[test]
    fn mounted_module_ids_derive_from_executed_targets_in_the_shared_tree() {
        let modules = [record("overlay_mod"), record("magic_mod")];
        let mut plan = test_plan();
        plan.tree.insert(
            "/system",
            crate::mount_tree::MountSource {
                module_id: ModuleId::try_from("overlay_mod").unwrap(),
                relative: "system".to_owned(),
                source_path: PathBuf::from("/data/adb/modules/overlay_mod/system"),
                file_type: crate::mount_tree::NodeFileType::Directory,
                replace: false,
                backend: Mode::Overlay,
            },
        );
        plan.tree.insert(
            "/system/etc/hosts",
            crate::mount_tree::MountSource {
                module_id: ModuleId::try_from("magic_mod").unwrap(),
                relative: "system/etc/hosts".to_owned(),
                source_path: PathBuf::from("/data/adb/modules/magic_mod/system/etc/hosts"),
                file_type: crate::mount_tree::NodeFileType::RegularFile,
                replace: false,
                backend: Mode::Magic,
            },
        );

        let mounted = mounted_module_ids_for_snapshot(
            &modules,
            &plan,
            &["/system".to_owned()],
            &["/system/etc/hosts".to_owned()],
        );

        assert_eq!(
            mounted,
            BTreeSet::from(["magic_mod".to_owned(), "overlay_mod".to_owned()])
        );

        let only_failed_targets = mounted_module_ids_for_snapshot(
            &modules,
            &plan,
            &["/vendor".to_owned()],
            &["/system/etc/absent".to_owned()],
        );
        assert!(only_failed_targets.is_empty());
    }

    #[test]
    fn load_state_distinguishes_missing_from_corrupt() {
        let dir =
            std::env::temp_dir().join(format!("hybrid-mount-state-load-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let missing_path = dir.join("missing.json");
        let missing = RunState::load_from(&missing_path);
        assert_eq!(missing.state_load.kind, StateLoadKind::Missing);
        assert!(!missing_path.exists());

        let corrupt_path = dir.join("corrupt.json");
        fs::write(&corrupt_path, "{not-json").unwrap();
        let corrupt = RunState::load_from(&corrupt_path);
        assert_eq!(corrupt.state_load.kind, StateLoadKind::Corrupt);
        assert!(corrupt.state_load.detail.is_some());

        fs::write(&corrupt_path, "{}").unwrap();
        let loaded = RunState::load_from(&corrupt_path);
        assert_eq!(loaded.state_load.kind, StateLoadKind::Loaded);
        assert!(loaded.state_load.detail.is_none());

        let io_path = dir.join("directory-as-state.json");
        fs::create_dir_all(&io_path).unwrap();
        let io_error = RunState::load_from(&io_path);
        assert_eq!(io_error.state_load.kind, StateLoadKind::IoError);
        assert!(io_error.state_load.detail.is_some());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn status_wire_snapshot_is_stable_for_legacy_webui_clients() {
        let state = RunState {
            timestamp: 12,
            pid: 34,
            storage_mode: "ext4".to_owned(),
            mount_point: PathBuf::new(),
            overlay_modules: vec!["alpha".to_owned()],
            magic_modules: vec!["beta".to_owned()],
            skip_mount_modules: vec!["gamma".to_owned()],
            active_mounts: vec!["/system".to_owned()],
            overlay_active_mounts: vec!["/system".to_owned()],
            magic_active_mounts: Vec::new(),
            vfs_modules: Vec::new(),
            vfs_active_mounts: Vec::new(),
            vfs_provider: None,
            vfs_error: None,
            vfs_error_modules: Vec::new(),
            vfs_foreign_nomount: false,
            confirmed_active_mounts: vec!["/system".to_owned()],
            mount_error_modules: Vec::new(),
            mount_error_reasons: BTreeMap::new(),
            mount_stats: MountStatistics {
                total_mounts: 1,
                successful_mounts: 1,
                overlayfs_mounts: 1,
                ..MountStatistics::default()
            },
            mode_stats: ModeStats {
                overlayfs: 1,
                magicmount: 1,
                vfs: 0,
            },
            state_load: StateLoadInfo::loaded(),
            failed_stage: Some("mount_execution".to_owned()),
            failure_reason: None,
            rollback_status: Some("clean".to_owned()),
            leftover_mount_targets: Vec::new(),
        };

        let json = serde_json::to_string_pretty(&state).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();

        // Old WebUI normalization only reads the existing fields; new diagnostic
        // fields are additive and must not rename or remove old keys.
        assert_eq!(value["timestamp"], 12);
        assert_eq!(value["active_mounts"][0], "/system");
        assert_eq!(value["confirmed_active_mounts"][0], "/system");
        assert_eq!(value["failed_stage"], "mount_execution");
        assert_eq!(value["rollback_status"], "clean");
        assert_eq!(value["state_load"]["kind"], "loaded");

        let legacy: RunState =
            serde_json::from_str(r#"{"timestamp":1,"active_mounts":["/system"]}"#).unwrap();
        assert_eq!(legacy.state_load.kind, StateLoadKind::Missing);
        assert_eq!(legacy.overlay_active_mounts, Vec::<String>::new());
        assert_eq!(legacy.confirmed_active_mounts, Vec::<String>::new());
    }

    #[test]
    fn status_wire_snapshot_distinguishes_failure_rollback_and_incomplete() {
        let clean = RunState {
            failed_stage: Some("mount_execution".to_owned()),
            rollback_status: Some("clean".to_owned()),
            ..RunState::default()
        };
        let incomplete = RunState {
            failed_stage: Some("state_save".to_owned()),
            rollback_status: Some("incomplete".to_owned()),
            leftover_mount_targets: vec!["/system".to_owned()],
            ..RunState::default()
        };

        let clean_json = serde_json::to_value(&clean).unwrap();
        assert_eq!(clean_json["failed_stage"], "mount_execution");
        assert_eq!(clean_json["rollback_status"], "clean");

        let incomplete_json = serde_json::to_value(&incomplete).unwrap();
        assert_eq!(incomplete_json["failed_stage"], "state_save");
        assert_eq!(incomplete_json["rollback_status"], "incomplete");
        assert_eq!(incomplete_json["leftover_mount_targets"][0], "/system");
    }

    #[test]
    fn app_module_wire_snapshot_is_stable() {
        let module = AppModule {
            id: ModuleId::try_from("hosts").unwrap(),
            name: "Hosts".to_owned(),
            version: "1".to_owned(),
            author: "author".to_owned(),
            description: "description".to_owned(),
            mode: "overlay".to_owned(),
            is_mounted: true,
            enabled: true,
            blacklisted: false,
            source_path: "/data/adb/modules/hosts".to_owned(),
            mount_error: None,
            suggest_ignore: false,
            rules: AppModuleRules {
                default_mode: Some("magic".to_owned()),
                paths: BTreeMap::from([("system/etc/hosts".to_owned(), "overlay".to_owned())]),
            },
        };

        assert_eq!(
            serde_json::to_string_pretty(&module).unwrap(),
            r#"{
  "id": "hosts",
  "name": "Hosts",
  "version": "1",
  "author": "author",
  "description": "description",
  "mode": "overlay",
  "is_mounted": true,
  "enabled": true,
  "blacklisted": false,
  "source_path": "/data/adb/modules/hosts",
  "mount_error": null,
  "suggest_ignore": false,
  "rules": {
    "default_mode": "magic",
    "paths": {
      "system/etc/hosts": "overlay"
    }
  }
}"#
        );
    }

    #[test]
    fn install_state_reports_unavailable_vfs_without_hiding_other_backends() {
        let state = build_install_state(true, true, true, true, false, "KSU");
        let payload = serde_json::to_value(state).unwrap();
        assert_eq!(payload["vfs_supported"], false);
        assert_eq!(payload["compatible"], true);
    }

    #[test]
    fn install_state_reports_a_responding_vfs_provider() {
        let state = build_install_state(true, true, true, true, true, "KSU");
        assert!(state.vfs_supported);
    }

    /// The UI offers "LKM" or "Built-in" from this label, so the mapping has to follow
    /// `/proc/modules` (loadable) versus a `/sys/module` entry alone (built into the kernel).
    #[test]
    fn install_state_vfs_type_labels_follow_the_module_tables() {
        use crate::vfs::doctor::ModulePresence;

        assert_eq!(vfs_type_label(ModulePresence::Loadable), "lkm");
        assert_eq!(vfs_type_label(ModulePresence::BuiltIn), "builtin");
        assert_eq!(vfs_type_label(ModulePresence::NotPresent), "unknown");
        assert_eq!(
            build_install_state(true, true, true, true, false, "KSU").vfs_type,
            "unknown",
            "the builder has no module tables to read, so it must not claim a provider"
        );
    }

    #[test]
    fn install_state_wire_snapshot_is_stable() {
        let state = build_install_state(true, true, true, true, false, "KSU");

        assert_eq!(
            serde_json::to_string_pretty(&state).unwrap(),
            r#"{
  "installed": true,
  "self_module": true,
  "binary": true,
  "config_exists": true,
  "overlay_supported": true,
  "tmpfs_supported": false,
  "nuke_supported": null,
  "nuke_type": "unknown",
  "vfs_supported": false,
  "vfs_type": "unknown",
  "mount_source": "KSU",
  "compatible": true
}"#
        );
    }

    #[test]
    fn clear_errors_wire_snapshot_is_stable() {
        assert_eq!(clear_errors_payload(3), r#"{"ok":true,"removed":3}"#);
    }
}
