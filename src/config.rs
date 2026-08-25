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

use std::collections::BTreeMap;
use std::fs;
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
            legacy_custom_mounts: Vec::new(),
        }
    }
}

impl Config {
    /// 解析 TOML 文本(空文本等价于全默认)。
    pub fn from_toml(text: &str) -> Result<Self> {
        let mut config: Self = toml::from_str(text)?;
        config.normalize_global_default();
        Ok(config)
    }

    /// 序列化为 TOML 文本。
    pub fn to_toml(&self) -> Result<String> {
        Ok(toml::to_string_pretty(self)?)
    }

    /// WebUI 配置响应。运行时能力只用于控制选项可见性，不持久化到 TOML。
    pub fn to_webui_json(&self, tmpfs_xattr_supported: bool) -> Result<String> {
        #[derive(Serialize)]
        struct WebUiConfig<'a> {
            #[serde(flatten)]
            config: &'a Config,
            tmpfs_xattr_supported: bool,
        }

        Ok(serde_json::to_string_pretty(&WebUiConfig {
            config: self,
            tmpfs_xattr_supported,
        })?)
    }

    /// 从磁盘读取配置。
    pub fn load(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path)?;
        Self::from_toml(&text)
    }

    /// 读取配置;失败或不存在时回退默认值(参考项目行为)。
    pub fn load_or_default(path: &Path) -> Self {
        match Self::load(path) {
            Ok(mut config) => {
                if !config.legacy_custom_mounts.is_empty() {
                    log::warn!(
                        "obsolete custom mount entries are ignored; configure module path rules instead"
                    );
                }
                config.legacy_custom_mounts.clear();
                config
            }
            Err(err) => {
                log::warn!("failed to load config, using default: {err}");
                Self::default()
            }
        }
    }

    /// 持久化配置;父目录不存在时自动创建。
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, self.to_toml()?)?;
        Ok(())
    }

    /// `gen-config`:重置为默认配置并写入磁盘,返回写入后的配置。
    pub fn write_default(path: &Path) -> Result<Self> {
        let config = Self::default();
        config.save(path)?;
        Ok(config)
    }

    /// 合并配置 patch:未出现的字段保留,`rules` 按模块合并。
    pub fn apply_patch(&mut self, patch: ConfigPatch) {
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
            if default_mode == Mode::Ignore {
                log::warn!("ignored unsupported global default_mode=ignore patch");
            } else {
                self.default_mode = default_mode;
            }
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

        self.normalize_global_default();
    }

    fn normalize_global_default(&mut self) {
        if self.default_mode == Mode::Ignore {
            log::warn!(
                "global default_mode=ignore is no longer supported; falling back to overlay"
            );
            self.default_mode = Mode::Overlay;
        }
    }
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
    let patch: ConfigPatch = serde_json::from_str(&payload_json)
        .map_err(|err| Error::msg(format!("parse config payload json: {err}")))?;

    let mut config = Config::load_or_default(path);
    config.apply_patch(patch);
    config.save(path)
}

/// `show-config`:输出 JSON 配置。
pub fn handle_show_config() -> Result<()> {
    let config = Config::load_or_default(Path::new(defs::CONFIG_PATH));
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
mod tests {
    use super::*;

    #[test]
    fn defaults_match_contract() {
        let config = Config::default();

        assert_eq!(config.moduledir, PathBuf::from("/data/adb/modules"));
        assert_eq!(config.mountsource, "KSU");
        assert_eq!(config.overlay_mode, OverlayMode::Ext4);
        assert!(!config.disable_umount);
        assert_eq!(config.default_mode, Mode::Overlay);
        assert!(config.rules.is_empty());
    }

    #[test]
    fn parses_empty_toml_as_defaults() {
        let config = Config::from_toml("").unwrap();
        assert_eq!(config, Config::default());
    }

    #[test]
    fn legacy_global_ignore_falls_back_without_changing_rule_ignores() {
        let config = Config::from_toml(
            r#"
default_mode = "ignore"

[rules.demo]
default_mode = "ignore"

[rules.demo.paths]
"system/etc/hosts" = "ignore"
"#,
        )
        .unwrap();

        assert_eq!(config.default_mode, Mode::Overlay);
        assert_eq!(config.rules["demo"].default_mode, Some(Mode::Ignore));
        assert_eq!(config.rules["demo"].paths["system/etc/hosts"], Mode::Ignore);
    }

    #[test]
    fn parses_planned_example() {
        let text = r#"
moduledir = "/data/adb/modules"
mountsource = "KSU"
overlay_mode = "ext4"
disable_umount = false
default_mode = "overlay"

[rules."hosts_redirect"]
default_mode = "magic"

[rules."hosts_redirect".paths]
"system/etc/hosts" = "overlay"
"#;

        let config = Config::from_toml(text).unwrap();

        let rule = config.rules.get("hosts_redirect").unwrap();
        assert_eq!(rule.default_mode, Some(Mode::Magic));
        assert_eq!(rule.paths.get("system/etc/hosts"), Some(&Mode::Overlay));
    }

    #[test]
    fn accepts_and_drops_obsolete_empty_custom_mounts_during_upgrade() {
        let config = Config::from_toml(
            r#"
moduledir = "/data/adb/modules"
default_mode = "magic"
custom_mounts = []
"#,
        )
        .unwrap();

        assert_eq!(config.default_mode, Mode::Magic);
        assert!(config.legacy_custom_mounts.is_empty());
        assert!(!config.to_toml().unwrap().contains("custom_mounts"));
    }

    #[test]
    fn toml_roundtrip_preserves_rules() {
        let mut config = Config {
            default_mode: Mode::Magic,
            ..Config::default()
        };
        config.rules.insert(
            "demo".to_owned(),
            ModuleRule {
                default_mode: Some(Mode::Ignore),
                paths: BTreeMap::from([
                    ("system/etc/hosts".to_owned(), Mode::Overlay),
                    ("system/bin/app".to_owned(), Mode::Magic),
                ]),
            },
        );

        let text = config.to_toml().unwrap();
        let reparsed = Config::from_toml(&text).unwrap();

        assert_eq!(reparsed, config);
    }

    #[test]
    fn rejects_invalid_mode() {
        let err = Config::from_toml(r#"default_mode = "transparent""#).unwrap_err();
        let message = err.to_string();

        assert!(message.contains("default_mode"), "{message}");
    }

    #[test]
    fn rejects_unknown_top_level_field() {
        let err = Config::from_toml(
            r#"
default_mode = "overlay"
unknown_option = true
"#,
        )
        .unwrap_err();

        assert!(err.to_string().contains("unknown field"), "{}", err);
    }

    #[test]
    fn rejects_unknown_rule_field() {
        let err = Config::from_toml(
            r#"
[rules.demo]
unknown_option = "overlay"
"#,
        )
        .unwrap_err();

        assert!(err.to_string().contains("unknown field"), "{}", err);
    }

    #[test]
    fn json_uses_contract_shape() {
        let config = Config::default();

        let value: serde_json::Value =
            serde_json::from_str(&serde_json::to_string_pretty(&config).unwrap()).unwrap();

        assert_eq!(value["moduledir"], "/data/adb/modules");
        assert_eq!(value["mountsource"], "KSU");
        assert_eq!(value["overlay_mode"], "ext4");
        assert_eq!(value["default_mode"], "overlay");
        assert_eq!(value["disable_umount"], false);
        assert_eq!(value["rules"], serde_json::json!({}));
    }

    #[test]
    fn webui_json_exposes_tmpfs_capability_without_persisting_it() {
        let config = Config::default();
        let value: serde_json::Value =
            serde_json::from_str(&config.to_webui_json(false).unwrap()).unwrap();

        assert_eq!(value["tmpfs_xattr_supported"], false);
        assert!(!config.to_toml().unwrap().contains("tmpfs_xattr_supported"));
    }

    #[test]
    fn save_creates_parent_and_load_roundtrips() {
        let dir = test_dir("save-load");
        let path = dir.join("nested").join("config.toml");

        let mut config = Config {
            disable_umount: true,
            ..Config::default()
        };
        config.rules.insert(
            "demo".to_owned(),
            ModuleRule {
                default_mode: Some(Mode::Magic),
                paths: BTreeMap::new(),
            },
        );

        config.save(&path).unwrap();
        let loaded = Config::load(&path).unwrap();

        assert_eq!(loaded, config);
        cleanup(&dir);
    }

    #[test]
    fn write_default_resets_disk_content() {
        let dir = test_dir("write-default");
        let path = dir.join("config.toml");

        let config = Config {
            default_mode: Mode::Ignore,
            ..Config::default()
        };
        config.save(&path).unwrap();

        let written = Config::write_default(&path).unwrap();

        assert_eq!(written, Config::default());
        assert_eq!(Config::load(&path).unwrap(), Config::default());
        cleanup(&dir);
    }

    #[test]
    fn load_or_default_falls_back_on_missing_file() {
        let dir = test_dir("load-or-default");
        let missing = dir.join("missing.toml");

        let config = Config::load_or_default(&missing);
        assert_eq!(config, Config::default());

        cleanup(&dir);
    }

    #[test]
    fn patch_merges_while_preserving_untouched_fields() {
        let mut config = Config {
            default_mode: Mode::Magic,
            ..Config::default()
        };
        config.rules.insert(
            "keep".to_owned(),
            ModuleRule {
                default_mode: Some(Mode::Ignore),
                paths: BTreeMap::from([("system/etc/a".to_owned(), Mode::Overlay)]),
            },
        );

        let patch: ConfigPatch = serde_json::from_str(
            r#"{"disable_umount":true,"rules":{"new":{"default_mode":"overlay","paths":{"system/etc/hosts":"magic"}}}}"#,
        )
        .unwrap();
        config.apply_patch(patch);

        assert!(config.disable_umount);
        assert_eq!(config.default_mode, Mode::Magic);
        assert_eq!(config.rules["keep"].default_mode, Some(Mode::Ignore));
        assert_eq!(config.rules["keep"].paths["system/etc/a"], Mode::Overlay);
        assert_eq!(config.rules["new"].default_mode, Some(Mode::Overlay));
        assert_eq!(config.rules["new"].paths["system/etc/hosts"], Mode::Magic);
    }

    #[test]
    fn patch_cannot_set_ignore_as_global_default() {
        let mut config = Config {
            default_mode: Mode::Magic,
            ..Config::default()
        };
        let patch: ConfigPatch = serde_json::from_str(r#"{"default_mode":"ignore"}"#).unwrap();

        config.apply_patch(patch);

        assert_eq!(config.default_mode, Mode::Magic);
    }

    #[test]
    fn patch_null_clears_module_default_mode() {
        let mut config = Config::default();
        config.rules.insert(
            "m".to_owned(),
            ModuleRule {
                default_mode: Some(Mode::Magic),
                paths: BTreeMap::new(),
            },
        );

        let patch: ConfigPatch =
            serde_json::from_str(r#"{"rules":{"m":{"default_mode":null}}}"#).unwrap();
        config.apply_patch(patch);

        assert_eq!(config.rules["m"].default_mode, None);
    }

    #[test]
    fn patch_can_replace_all_rules_for_full_editor_save() {
        let mut config = Config::default();
        config
            .rules
            .insert("stale".to_owned(), ModuleRule::default());
        config
            .rules
            .insert("keep".to_owned(), ModuleRule::default());

        let patch: ConfigPatch = serde_json::from_str(
            r#"{"replace_rules":true,"rules":{"keep":{"default_mode":"magic"}}}"#,
        )
        .unwrap();
        config.apply_patch(patch);

        assert!(!config.rules.contains_key("stale"));
        assert_eq!(config.rules.len(), 1);
        assert_eq!(config.rules["keep"].default_mode, Some(Mode::Magic));
    }

    #[test]
    fn payload_hex_roundtrips_through_save() {
        let dir = test_dir("payload");
        let path = dir.join("config.toml");

        let json = r#"{"default_mode":"magic","disable_umount":true}"#;
        let payload_hex = hex::encode(json);
        save_config_payload(&path, &payload_hex).unwrap();

        let saved = Config::load(&path).unwrap();
        assert_eq!(saved.default_mode, Mode::Magic);
        assert!(saved.disable_umount);
        assert_eq!(saved.moduledir, PathBuf::from("/data/adb/modules"));

        cleanup(&dir);
    }

    #[test]
    fn payload_arg_requires_marker_and_rejects_invalid_hex() {
        assert_eq!(
            parse_payload_arg(&["--payload".to_owned(), "7b7d".to_owned()]).unwrap(),
            "7b7d"
        );
        assert!(parse_payload_arg(&["x".to_owned()]).is_err());
        assert!(decode_payload_arg("zz").is_err());
        assert_eq!(decode_payload_arg("7b7d").unwrap(), "{}");
    }

    fn test_dir(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("hybrid-mount-{tag}-{}", std::process::id()))
    }

    fn cleanup(dir: &Path) {
        fs::remove_dir_all(dir).ok();
    }
}
