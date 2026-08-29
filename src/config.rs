// SPDX-License-Identifier: GPL-3.0-only

//! TOML 配置 schema、默认值与持久化核心。
//!
//! ```toml
//! moduledir = "/data/adb/modules"
//! mountsource = "KSU"
//! overlay_mode = "ext4"      # tmpfs | ext4
//! disable_umount = false
//! default_mode = "overlay"   # overlay | magic
//!
//! [rules."<module_id>"]
//! default_mode = "magic"
//!
//! [rules."<module_id>".paths]
//! "system/etc/hosts" = "overlay"
//! ```
//!
//! 未知字段会被拒绝,保证配置契约不会被悄悄漂移。

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::defs;
use crate::errors::{Error, Result};

/// 单个路径/模块可选的挂载后端。
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Overlay,
    Magic,
    Ignore,
}

impl Mode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Overlay => "overlay",
            Self::Magic => "magic",
            Self::Ignore => "ignore",
        }
    }
}

/// overlayfs staging 后端(v4.2.0 语义)。
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

/// 单个模块的规则:模块级默认后端 + 路径级覆盖。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleRule {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_mode: Option<Mode>,

    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub paths: BTreeMap<String, Mode>,
}

/// 持久配置根对象。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default = "default_moduledir")]
    pub moduledir: PathBuf,

    #[serde(default = "default_mountsource")]
    pub mountsource: String,

    #[serde(default)]
    pub overlay_mode: OverlayMode,

    #[serde(default)]
    pub disable_umount: bool,

    #[serde(default)]
    pub default_mode: Mode,

    #[serde(default)]
    pub rules: BTreeMap<String, ModuleRule>,

    /// 模块黑名单来自独立 TOML 文件，不属于 WebUI 可写配置。
    #[serde(skip)]
    pub(crate) module_blacklist: BTreeSet<String>,

    /// 主配置文件不存在时使用默认值；仅用于 `show-config` 的诊断展示，
    /// 不写入 TOML。配置文件存在但损坏/不可读时，`load_or_default` 返回错误。
    #[serde(skip)]
    pub config_missing: bool,

    /// Upgrade-only input from releases that exposed custom bind mounts.
    /// The backend no longer implements that feature; accepting and omitting
    /// this field prevents one obsolete empty array from discarding the rest
    /// of an otherwise valid configuration.
    #[serde(default, rename = "custom_mounts", skip_serializing)]
    pub(crate) legacy_custom_mounts: Vec<toml::Value>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            moduledir: default_moduledir(),
            mountsource: default_mountsource(),
            overlay_mode: OverlayMode::default(),
            disable_umount: false,
            default_mode: Mode::default(),
            rules: BTreeMap::new(),
            module_blacklist: BTreeSet::new(),
            config_missing: false,
            legacy_custom_mounts: Vec::new(),
        }
    }
}

impl Config {
    /// 解析 TOML 文本(空文本等价于全默认)。
    ///
    /// `default_mode = "ignore"` 是"可解析但已废弃"的值：直接报错，
    /// 不再静默规范化为 Overlay。按模块/路径禁用请使用 `[rules.*]`。
    pub fn from_toml(text: &str) -> Result<Self> {
        let config: Self = toml::from_str(text)?;
        if config.default_mode == Mode::Ignore {
            return Err(Error::UnsupportedGlobalDefaultMode);
        }
        Ok(config)
    }

    /// 序列化为 TOML 文本。
    pub fn to_toml(&self) -> Result<String> {
        Ok(toml::to_string_pretty(self)?)
    }

    /// WebUI 配置响应。运行时能力只用于控制选项可见性，不持久化到 TOML。
    /// `config_missing` 让 WebUI 区分"从未创建配置"与"配置损坏"。
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

    /// 从磁盘读取配置。读取、解析和黑名单加载错误都携带配置路径上下文。
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

    /// 读取配置：文件不存在时使用默认值并标记 `config_missing`；
    /// 文件存在但损坏、无权限或黑名单不可用时返回错误，绝不伪装成缺失。
    pub fn load_or_default(path: &Path) -> Result<Self> {
        match Self::load(path) {
            Ok(mut config) => {
                if !config.legacy_custom_mounts.is_empty() {
                    log::warn!(
                        "obsolete custom mount entries are ignored; configure module path rules instead"
                    );
                }
                config.legacy_custom_mounts.clear();
                Ok(config)
            }
            Err(Error::ConfigRead { source, .. }) if source.kind() == ErrorKind::NotFound => {
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
            Err(err) => Err(err),
        }
    }

    /// 持久化配置；父目录不存在时自动创建。
    /// 通过 `sys::fs::atomic_write` 写临时文件 + fsync + rename，失败不会暴露截断内容。
    /// 与 `from_toml`/`apply_patch` 一样拒绝把已废弃的全局 ignore 落到磁盘。
    /// 原子 rename 会替换符号链接本身，与旧的 `fs::write` 跟随链接语义不同，
    /// 因此对符号链接目标显式报错，避免静默改变用户的数据布局。
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

    /// `gen-config`:重置为默认配置并写入磁盘,返回写入后的配置。
    pub fn write_default(path: &Path) -> Result<Self> {
        let config = Self::default();
        config.save(path)?;
        Ok(config)
    }

    /// 合并配置 patch:未出现的字段保留,`rules` 按模块合并。
    /// 校验在修改前完成：非法 patch 不会留下部分更新。
    pub fn apply_patch(&mut self, patch: ConfigPatch) -> Result<()> {
        if patch.default_mode == Some(Mode::Ignore) {
            return Err(Error::UnsupportedGlobalDefaultMode);
        }

        if let Some(moduledir) = patch.moduledir {
            self.moduledir = moduledir;
        }
        if let Some(mountsource) = patch.mountsource {
            self.mountsource = mountsource;
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

    /// 加载随包发布与用户持久化的模块黑名单。
    ///
    /// 语义（G04）：文件**缺失** = 无对应来源的黑名单，属于正常状态；
    /// 文件存在但**损坏或不可读** = 错误，调用方必须 fail-closed，
    /// 防止用户明确屏蔽的模块因解析失败而重新参与挂载。
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

fn read_module_blacklist(path: &Path) -> Result<BTreeSet<String>> {
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
    Ok(file
        .blacklist
        .into_iter()
        .map(|id| id.trim().to_owned())
        .filter(|id| !id.is_empty())
        .collect())
}

/// `save-config --payload <hex>` 的部分配置 patch:缺省字段保留。
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigPatch {
    #[serde(default)]
    pub moduledir: Option<PathBuf>,

    #[serde(default)]
    pub mountsource: Option<String>,

    #[serde(default)]
    pub overlay_mode: Option<OverlayMode>,

    #[serde(default)]
    pub disable_umount: Option<bool>,

    #[serde(default)]
    pub default_mode: Option<Mode>,

    #[serde(default)]
    pub rules: Option<BTreeMap<String, ModuleRulePatch>>,

    /// 全量配置保存时先清空旧规则；缺省时保持历史 patch 合并语义。
    #[serde(default)]
    pub replace_rules: Option<bool>,
}

/// 模块规则 patch:`default_mode: null` 表示清除模块级模式,
/// `paths` 出现时全量替换该模块路径规则。
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleRulePatch {
    #[serde(default, deserialize_with = "deserialize_optional_mode_clear")]
    pub default_mode: Option<Option<Mode>>,

    #[serde(default)]
    pub paths: Option<BTreeMap<String, Mode>>,
}

/// 区分字段缺失(`None`)与显式 null(`Some(None)`),后者清除模块级模式。
/// 字段缺失时 serde 不会调用本函数;字段出现且为 null 时走 `visit_none`。
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

/// 从 `["--payload", "<hex>", ...]` 中取出 payload(参考项目方式)。
pub fn parse_payload_arg(args: &[String]) -> Result<&str> {
    args.windows(2)
        .find_map(|window| (window[0] == "--payload").then_some(window[1].as_str()))
        .ok_or_else(|| Error::msg("missing required --payload argument"))
}

/// hex payload -> UTF-8 JSON 文本。
pub fn decode_payload_arg(payload_hex: &str) -> Result<String> {
    let bytes =
        hex::decode(payload_hex).map_err(|err| Error::msg(format!("decode payload hex: {err}")))?;
    String::from_utf8(bytes).map_err(|err| Error::msg(format!("payload is not valid UTF-8: {err}")))
}

/// 解析 payload 并合并/持久化到指定路径。
pub fn save_config_payload(path: &Path, payload_hex: &str) -> Result<()> {
    let payload_json = decode_payload_arg(payload_hex)?;
    let patch: ConfigPatch = serde_json::from_str(&payload_json).map_err(|err| {
        Error::msg(format!(
            "parse config payload json for {}: {err}",
            path.display()
        ))
    })?;

    let mut config = Config::load_or_default(path)?;
    config.apply_patch(patch)?;
    config.save(path)
}

/// `show-config`:输出 JSON 配置。
/// 配置缺失时输出默认配置并带 `config_missing: true`；损坏/不可读时返回错误。
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

/// `save-config --payload <hex>`:合并/持久化配置,返回 `{ok:true}`。
pub fn handle_save_config(args: &[String]) -> Result<()> {
    let payload = parse_payload_arg(args)?;
    save_config_payload(Path::new(defs::CONFIG_PATH), payload)?;
    println!("{}", serde_json::json!({ "ok": true }));
    Ok(())
}

/// `gen-config`:重置默认配置,返回 `{ok:true}`。
pub fn handle_gen_config() -> Result<()> {
    Config::write_default(Path::new(defs::CONFIG_PATH))?;
    println!("{}", serde_json::json!({ "ok": true }));
    Ok(())
}

fn default_moduledir() -> PathBuf {
    PathBuf::from(defs::DEFAULT_MODULE_DIR)
}

fn default_mountsource() -> String {
    defs::DEFAULT_MOUNT_SOURCE.to_owned()
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
