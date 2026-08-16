// ReHybrid-Mount
//
// SPDX-License-Identifier: GPL-3.0-only

//! TOML 配置 schema、默认值与持久化核心。
//!
//! 契约见 REHYBRID_MOUNT_PLAN.md 第 4.1 节:
//!
//! ```toml
//! moduledir = "/data/adb/modules"
//! mountsource = "KSU"
//! overlay_mode = "ext4"      # tmpfs | ext4
//! disable_umount = false
//! default_mode = "overlay"   # overlay | magic | ignore
//!
//! [rules."<module_id>"]
//! default_mode = "magic"
//!
//! [rules."<module_id>".paths]
//! "system/etc/hosts" = "overlay"
//! ```
//!
//! 未知字段会被拒绝,保证配置契约不会被悄悄漂移。
//!
//! Stage 1 脚手架:公开 API 在 Stage 5 CLI 接入前暂未被二进制入口使用;
//! 接入完成后移除本豁免,恢复 dead_code 检查。
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::defs;
use crate::errors::Result;

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

/// overlayfs staging 后端(v4.2.0 语义)。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OverlayMode {
    Tmpfs,
    #[default]
    Ext4,
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
        }
    }
}

impl Config {
    /// 解析 TOML 文本(空文本等价于全默认)。
    pub fn from_toml(text: &str) -> Result<Self> {
        Ok(toml::from_str(text)?)
    }

    /// 序列化为 TOML 文本。
    pub fn to_toml(&self) -> Result<String> {
        Ok(toml::to_string_pretty(self)?)
    }

    /// `show-config` 的 JSON 输出。
    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    /// 从磁盘读取配置。
    pub fn load(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path)?;
        Self::from_toml(&text)
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

        let value: serde_json::Value = serde_json::from_str(&config.to_json().unwrap()).unwrap();

        assert_eq!(value["moduledir"], "/data/adb/modules");
        assert_eq!(value["mountsource"], "KSU");
        assert_eq!(value["overlay_mode"], "ext4");
        assert_eq!(value["default_mode"], "overlay");
        assert_eq!(value["disable_umount"], false);
        assert_eq!(value["rules"], serde_json::json!({}));
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

    fn test_dir(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("rehybrid-mount-{tag}-{}", std::process::id()))
    }

    fn cleanup(dir: &Path) {
        fs::remove_dir_all(dir).ok();
    }
}
