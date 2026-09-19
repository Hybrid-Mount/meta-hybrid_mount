// SPDX-License-Identifier: GPL-3.0-only

//! TOML config schema, defaults and persistence.
//!
//! ```toml
//! moduledir = "/data/adb/modules"
//! overlay_mode = "ext4"      # tmpfs | ext4
//! disable_umount = false
//! default_mode = "overlay"   # overlay | magic | vfs
//!
//! [rules."<module_id>"]
//! default_mode = "magic"
//!
//! [rules."<module_id>".paths]
//! "system/etc/hosts" = "overlay"
//! ```
//!
//! Unknown fields are rejected so the config contract cannot drift silently.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::defs;
use crate::errors::{Error, Result};
use crate::module_id::ModuleId;

/// Mount backend selectable per path or per module.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Overlay,
    Magic,
    Vfs,
    Ignore,
}

impl Mode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Overlay => "overlay",
            Self::Magic => "magic",
            Self::Vfs => "vfs",
            Self::Ignore => "ignore",
        }
    }
}

/// OverlayFS staging backend (v4.2.0 semantics).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OverlayMode {
    Tmpfs,
    #[default]
    Ext4,
}

impl OverlayMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Tmpfs => "tmpfs",
            Self::Ext4 => "ext4",
        }
    }
}

/// Rules for one module: a module-level default backend plus per-path overrides.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleRule {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_mode: Option<Mode>,

    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub paths: BTreeMap<String, Mode>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default = "default_moduledir")]
    pub moduledir: PathBuf,

    #[serde(default)]
    pub overlay_mode: OverlayMode,

    #[serde(default)]
    pub disable_umount: bool,

    #[serde(default)]
    pub default_mode: Mode,

    /// Fail the boot (`true`) or degrade (`false`) when the VFS backend is unavailable.
    #[serde(default)]
    pub vfs_strict: bool,

    /// UIDs to isolate, meaning they see the native filesystem. Sent to the VFS provider.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vfs_isolate_uids: Vec<u32>,

    #[serde(default)]
    pub rules: BTreeMap<ModuleId, ModuleRule>,

    /// The module blacklist lives in its own TOML file, not in the WebUI-writable config.
    #[serde(skip)]
    pub(crate) module_blacklist: BTreeSet<ModuleId>,

    /// Used when the main config file is absent. For the `show-config` diagnostic display
    /// only, never written back to TOML. A corrupt or unreadable file also falls back to the default, but is not marked missing.
    #[serde(skip)]
    pub config_missing: bool,

    /// Retired input accepted only to preserve settings during upgrades.
    /// Mount sources now follow the detected root backend.
    #[serde(default, rename = "mountsource", skip_serializing)]
    pub(crate) legacy_mountsource: Option<String>,

    /// Upgrade-only input from releases that exposed custom bind mounts.
    /// The backend no longer implements that feature; accepting and omitting
    /// this field prevents one obsolete empty array from discarding the rest
    /// of an otherwise valid configuration.
    #[serde(default, rename = "custom_mounts", skip_serializing)]
    pub(crate) legacy_custom_mounts: Vec<toml::Value>,

    /// Upgrade-only input from releases with a persistent daemon. The current
    /// boot pipeline has no daemon; accepting this retired key preserves rules.
    #[serde(default, rename = "daemon_startup_mode", skip_serializing)]
    pub(crate) legacy_daemon_startup_mode: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            moduledir: default_moduledir(),
            overlay_mode: OverlayMode::default(),
            disable_umount: false,
            default_mode: Mode::default(),
            vfs_strict: false,
            vfs_isolate_uids: Vec::new(),
            rules: BTreeMap::new(),
            module_blacklist: BTreeSet::new(),
            config_missing: false,
            legacy_mountsource: None,
            legacy_custom_mounts: Vec::new(),
            legacy_daemon_startup_mode: None,
        }
    }
}

impl Config {
    /// Parses TOML text; empty text is equivalent to all defaults.
    ///
    /// `default_mode = "ignore"` parses but is deprecated, so it is rejected outright
    /// rather than silently normalised to Overlay. Disable a module or path with `[rules.*]`.
    pub fn from_toml(text: &str) -> Result<Self> {
        let config: Self = toml::from_str(text)?;
        if config.default_mode == Mode::Ignore {
            return Err(Error::UnsupportedGlobalDefaultMode);
        }
        Ok(config)
    }

    /// Serialises to TOML text.
    pub fn to_toml(&self) -> Result<String> {
        Ok(toml::to_string_pretty(self)?)
    }

    /// WebUI config response. Runtime capabilities only control option visibility and are not persisted to TOML.
    /// `config_missing` tells the WebUI whether the main config file is absent.
    pub fn to_webui_json(&self, tmpfs_xattr_supported: bool) -> Result<String> {
        #[derive(Serialize)]
        struct WebUiConfig<'a> {
            #[serde(flatten)]
            config: &'a Config,
            tmpfs_xattr_supported: bool,
            config_missing: bool,
        }

        Ok(serde_json::to_string_pretty(&WebUiConfig {
            config: self,
            tmpfs_xattr_supported,
            config_missing: self.config_missing,
        })?)
    }

    /// Reads config from disk. Read, parse and blacklist errors all carry the config path as context.
    pub fn load(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path).map_err(|source| Error::ConfigRead {
            path: path.to_path_buf(),
            source,
        })?;
        let mut config = match Self::from_toml(&text) {
            Ok(config) => config,
            Err(Error::TomlParse(source)) => {
                return Err(Error::ConfigParse {
                    path: path.to_path_buf(),
                    source,
                });
            }
            Err(err) => return Err(err),
        };
        config.load_module_blacklists(path)?;
        Ok(config)
    }

    fn finish_loaded(mut config: Self) -> Self {
        if config.legacy_mountsource.take().is_some() {
            log::info!("ignoring obsolete mountsource; using the detected root backend");
        }
        if !config.legacy_custom_mounts.is_empty() {
            log::warn!(
                "obsolete custom mount entries are ignored; configure module path rules instead"
            );
        }
        config.legacy_custom_mounts.clear();
        if config.legacy_daemon_startup_mode.take().is_some() {
            log::info!("ignoring obsolete daemon_startup_mode during configuration upgrade");
        }
        config
    }

    fn defaults_for_missing(path: &Path) -> Result<Self> {
        log::info!(
            "config file missing, using defaults: path={}",
            path.display()
        );
        let mut config = Self {
            config_missing: true,
            ..Self::default()
        };
        config.load_module_blacklists(path)?;
        Ok(config)
    }

    fn main_config_is_genuinely_missing(path: &Path) -> bool {
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        fs::symlink_metadata(path).is_err_and(|err| err.kind() == ErrorKind::NotFound)
            && fs::metadata(parent).is_ok_and(|metadata| metadata.is_dir())
    }

    fn load_or_missing_tolerant(path: &Path) -> Result<Self> {
        match Self::load(path) {
            Ok(config) => Ok(Self::finish_loaded(config)),
            Err(Error::ConfigRead { source, .. }) if source.kind() == ErrorKind::NotFound => {
                Self::defaults_for_missing(path)
            }
            Err(err) => Err(err),
        }
    }

    /// Boot-time loader: a genuinely absent main config uses defaults, while
    /// corrupt, unsupported, unreadable, dangling-symlink, and blacklist errors
    /// abort before module scanning or mount planning begins.
    pub fn load_for_boot(path: &Path) -> Result<Self> {
        match Self::load(path) {
            Ok(config) => {
                let config = Self::finish_loaded(config);
                Ok(config)
            }
            Err(Error::ConfigRead { source, .. })
                if source.kind() == ErrorKind::NotFound
                    && Self::main_config_is_genuinely_missing(path) =>
            {
                Self::defaults_for_missing(path)
            }
            Err(err) => Err(err),
        }
    }

    /// Reads config: a missing file uses defaults and sets `config_missing`.
    /// A corrupt, unreadable or unsupported main config logs a warning and uses defaults without overwriting the file.
    /// A corrupt or unreadable standalone module blacklist is still an error, staying fail-closed.
    pub fn load_or_default(path: &Path) -> Result<Self> {
        match Self::load_or_missing_tolerant(path) {
            Ok(config) => Ok(config),
            Err(err) => match &err {
                Error::ConfigRead { .. }
                | Error::ConfigParse { .. }
                | Error::UnsupportedGlobalDefaultMode => {
                    log::warn!(
                        "failed to load config, using defaults: path={}, error={err}",
                        path.display()
                    );
                    let mut config = Self::default();
                    config.load_module_blacklists(path)?;
                    Ok(config)
                }
                _ => Err(err),
            },
        }
    }

    /// Persists config, creating parent directories as needed.
    /// Goes through `sys::fs::atomic_write` (temp file + fsync + rename), so a failure never exposes truncated content.
    /// Like `from_toml`/`apply_patch` it refuses to write the deprecated global ignore to disk.
    /// The atomic rename replaces a symlink itself rather than following it as the old `fs::write` did,
    /// so a symlinked target is an explicit error instead of silently changing the user's data layout.
    pub fn save(&self, path: &Path) -> Result<()> {
        if self.default_mode == Mode::Ignore {
            return Err(Error::UnsupportedGlobalDefaultMode);
        }
        match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(Error::msg(format!(
                    "refusing to replace symlinked config file {}; remove the symlink and save a regular file",
                    path.display()
                )));
            }
            Ok(_) => {}
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            Err(source) => {
                return Err(Error::ConfigRead {
                    path: path.to_path_buf(),
                    source,
                });
            }
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                Error::msg(format!(
                    "create config parent directory {}: {err}",
                    parent.display()
                ))
            })?;
        }
        crate::sys::fs::atomic_write(path, self.to_toml()?.as_bytes())
            .map_err(|err| Error::msg(format!("atomically save config {}: {err}", path.display())))
    }

    /// `gen-config`: resets to the default config, writes it to disk and returns it.
    pub fn write_default(path: &Path) -> Result<Self> {
        let config = Self::default();
        config.save(path)?;
        Ok(config)
    }

    /// Merges a config patch: absent fields are kept and `rules` merge per module.
    /// Validation runs before any mutation, so an invalid patch leaves no partial update.
    pub fn apply_patch(&mut self, patch: ConfigPatch) -> Result<()> {
        if patch.default_mode == Some(Mode::Ignore) {
            return Err(Error::UnsupportedGlobalDefaultMode);
        }

        if let Some(moduledir) = patch.moduledir {
            self.moduledir = moduledir;
        }
        if let Some(overlay_mode) = patch.overlay_mode {
            self.overlay_mode = overlay_mode;
        }
        if let Some(disable_umount) = patch.disable_umount {
            self.disable_umount = disable_umount;
        }
        if let Some(default_mode) = patch.default_mode {
            self.default_mode = default_mode;
        }

        if patch.replace_rules.unwrap_or(false) {
            self.rules.clear();
        }

        if let Some(rules) = patch.rules {
            for (module_id, rule_patch) in rules {
                let rule = self.rules.entry(module_id).or_default();
                if let Some(default_mode) = rule_patch.default_mode {
                    rule.default_mode = default_mode;
                }
                if let Some(paths) = rule_patch.paths {
                    rule.paths = paths;
                }
            }
        }

        Ok(())
    }

    pub(crate) fn is_module_blacklisted(&self, module_id: &str) -> bool {
        self.module_blacklist.contains(module_id)
    }

    /// Whether any rule anywhere selects the VFS backend.
    ///
    /// The boot pipeline loads the bundled kernel module only when this is true, so a device
    /// configured for overlay or magic alone never tries to `insmod` anything. A rule that
    /// names a module which is disabled, missing or blacklisted still counts: the config is
    /// the only input available before the module scan, and an unnecessary load attempt is
    /// harmless next to missing one the user asked for.
    pub fn wants_vfs(&self) -> bool {
        self.default_mode == Mode::Vfs
            || self
                .rules
                .values()
                .any(|rule| rule.default_mode == Some(Mode::Vfs))
            || self
                .rules
                .values()
                .any(|rule| rule.paths.values().any(|mode| *mode == Mode::Vfs))
    }

    /// Loads the bundled and user-persisted module blacklists.
    ///
    /// A **missing** file means no blacklist from that source, which is normal.
    /// A file that exists but is **corrupt or unreadable** is an error the caller must treat as fail-closed,
    /// so a module the user explicitly blocked cannot rejoin the mount set because parsing failed.
    fn load_module_blacklists(&mut self, config_path: &Path) -> Result<()> {
        let persistent_path = if config_path == Path::new(defs::CONFIG_PATH) {
            PathBuf::from(defs::MODULE_BLACKLIST_PATH)
        } else {
            config_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(defs::MODULE_BLACKLIST_FILE_NAME)
        };

        // Released defaults live inside the module and must remain effective
        // after an upgrade. The persistent copy additionally preserves local
        // additions. Merge both sources instead of letting one shadow the
        // other.
        if config_path == Path::new(defs::CONFIG_PATH) {
            self.module_blacklist
                .extend(read_module_blacklist(Path::new(
                    defs::BUNDLED_MODULE_BLACKLIST_PATH,
                ))?);
        }
        self.module_blacklist
            .extend(read_module_blacklist(&persistent_path)?);

        log::info!(
            "module blacklist loaded: entries={}",
            self.module_blacklist.len()
        );
        Ok(())
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ModuleBlacklistFile {
    blacklist: Vec<String>,
}

fn read_module_blacklist(path: &Path) -> Result<BTreeSet<ModuleId>> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(BTreeSet::new()),
        Err(source) => {
            return Err(Error::ModuleBlacklistRead {
                path: path.to_path_buf(),
                source,
            });
        }
    };

    let file = toml::from_str::<ModuleBlacklistFile>(&text).map_err(|source| {
        Error::ModuleBlacklistParse {
            path: path.to_path_buf(),
            source,
        }
    })?;

    let mut blacklist = BTreeSet::new();
    for entry in file.blacklist {
        let id = entry.trim();
        if id.is_empty() {
            continue;
        }
        let module_id =
            ModuleId::try_from(id.to_owned()).map_err(|_| Error::InvalidBlacklistModuleId {
                path: path.to_path_buf(),
                module_id: id.to_owned(),
            })?;
        blacklist.insert(module_id);
    }
    Ok(blacklist)
}

/// Partial config patch from `save-config --payload <hex>`; absent fields are kept.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigPatch {
    #[serde(default)]
    pub moduledir: Option<PathBuf>,

    #[serde(default)]
    pub overlay_mode: Option<OverlayMode>,

    #[serde(default)]
    pub disable_umount: Option<bool>,

    #[serde(default)]
    pub default_mode: Option<Mode>,

    #[serde(default)]
    pub rules: Option<BTreeMap<ModuleId, ModuleRulePatch>>,

    /// A full config save clears the old rules first; when absent, the historical patch-merge semantics apply.
    #[serde(default)]
    pub replace_rules: Option<bool>,
}

/// Module rule patch: `default_mode: null` clears the module-level mode, and a present
/// `paths` replaces that module's path rules wholesale.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleRulePatch {
    #[serde(default, deserialize_with = "deserialize_optional_mode_clear")]
    pub default_mode: Option<Option<Mode>>,

    #[serde(default)]
    pub paths: Option<BTreeMap<String, Mode>>,
}

/// Distinguishes a missing field (`None`) from an explicit null (`Some(None)`), the latter clearing the module-level mode.
/// serde skips this for a missing field and reaches `visit_none` only for a present null.
fn deserialize_optional_mode_clear<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<Option<Mode>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct OptionalModeClearVisitor;

    impl<'de> serde::de::Visitor<'de> for OptionalModeClearVisitor {
        type Value = Option<Option<Mode>>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("an overlay/magic/ignore mode or null")
        }

        fn visit_none<E>(self) -> std::result::Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(Some(None))
        }

        fn visit_some<D>(self, deserializer: D) -> std::result::Result<Self::Value, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            Mode::deserialize(deserializer).map(|mode| Some(Some(mode)))
        }
    }

    deserializer.deserialize_option(OptionalModeClearVisitor)
}

/// Extracts the payload from `["--payload", "<hex>", ...]`.
pub fn parse_payload_arg(args: &[String]) -> Result<&str> {
    args.windows(2)
        .find_map(|window| (window[0] == "--payload").then_some(window[1].as_str()))
        .ok_or_else(|| Error::msg("missing required --payload argument"))
}

/// hex payload -> UTF-8 JSON text.
pub fn decode_payload_arg(payload_hex: &str) -> Result<String> {
    let bytes =
        hex::decode(payload_hex).map_err(|err| Error::msg(format!("decode payload hex: {err}")))?;
    String::from_utf8(bytes).map_err(|err| Error::msg(format!("payload is not valid UTF-8: {err}")))
}

/// Parses the payload and merges or persists it to the given path.
pub fn save_config_payload(path: &Path, payload_hex: &str) -> Result<()> {
    let payload_json = decode_payload_arg(payload_hex)?;
    let patch: ConfigPatch = serde_json::from_str(&payload_json).map_err(|err| {
        Error::msg(format!(
            "parse config payload json for {}: {err}",
            path.display()
        ))
    })?;

    // Saving must not turn a corrupt or unreadable config into defaults. A genuinely
    // missing file is the only fallback that is safe to materialize on disk.
    let mut config = Config::load_or_missing_tolerant(path)?;
    config.apply_patch(patch)?;
    config.save(path)
}

/// `show-config`: outputs the JSON config.
/// When it is missing, outputs the default with `config_missing: true`; when corrupt or unreadable, outputs the default.
pub fn handle_show_config() -> Result<()> {
    let config = Config::load_or_default(Path::new(defs::CONFIG_PATH))?;
    let tmpfs_xattr_supported = match crate::sys::fs::is_overlay_xattr_supported() {
        Ok(supported) => supported,
        Err(err) => {
            log::warn!("capability probe failed: tmpfs_xattr, error={err}");
            false
        }
    };
    println!("{}", config.to_webui_json(tmpfs_xattr_supported)?);
    Ok(())
}

/// `save-config --payload <hex>`: merges and persists the config, returning `{ok:true}`.
pub fn handle_save_config(args: &[String]) -> Result<()> {
    let payload = parse_payload_arg(args)?;
    save_config_payload(Path::new(defs::CONFIG_PATH), payload)?;
    println!("{}", serde_json::json!({ "ok": true }));
    Ok(())
}

/// `gen-config`: resets to the default config, returning `{ok:true}`.
pub fn handle_gen_config() -> Result<()> {
    Config::write_default(Path::new(defs::CONFIG_PATH))?;
    println!("{}", serde_json::json!({ "ok": true }));
    Ok(())
}

fn default_moduledir() -> PathBuf {
    PathBuf::from(defs::DEFAULT_MODULE_DIR)
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
