// SPDX-License-Identifier: GPL-3.0-only

//! 经过验证的模块 ID。
//!
//! 模块 ID 只在 `ModuleId::try_from(String)` 这一个入口完成合法性验证，
//! scanner、config rules、mount tree、plan 与 state 内部只消费该类型。
//! 序列化仍然是普通字符串，因此 TOML/JSON 线格式与旧版本兼容。

use std::borrow::Borrow;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::errors::{Error, Result};

/// `^[a-zA-Z][a-zA-Z0-9._-]*$`（与既有 `validate_module_id` 行为一致）。
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ModuleId(String);

impl ModuleId {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl TryFrom<String> for ModuleId {
    type Error = Error;

    fn try_from(module_id: String) -> Result<Self> {
        validate(&module_id)?;
        Ok(Self(module_id))
    }
}

impl TryFrom<&str> for ModuleId {
    type Error = Error;

    fn try_from(module_id: &str) -> Result<Self> {
        validate(module_id)?;
        Ok(Self(module_id.to_owned()))
    }
}

impl FromStr for ModuleId {
    type Err = Error;

    fn from_str(module_id: &str) -> Result<Self> {
        Self::try_from(module_id)
    }
}

impl fmt::Display for ModuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl AsRef<str> for ModuleId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for ModuleId {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl From<ModuleId> for String {
    fn from(module_id: ModuleId) -> Self {
        module_id.0
    }
}

/// 测试与状态查询经常把 newtype 与字面量比较；字符串永远只是投影，
/// 不会反过来构造未验证的 `ModuleId`。
impl PartialEq<str> for ModuleId {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for ModuleId {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl PartialEq<String> for ModuleId {
    fn eq(&self, other: &String) -> bool {
        self.0 == *other
    }
}

impl Serialize for ModuleId {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ModuleId {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct ModuleIdVisitor;

        impl serde::de::Visitor<'_> for ModuleIdVisitor {
            type Value = ModuleId;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a module id matching /^[a-zA-Z][a-zA-Z0-9._-]*$/")
            }

            fn visit_str<E>(self, value: &str) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                ModuleId::try_from(value).map_err(E::custom)
            }

            fn visit_string<E>(self, value: String) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                ModuleId::try_from(value).map_err(E::custom)
            }
        }

        deserializer.deserialize_string(ModuleIdVisitor)
    }
}

fn validate(module_id: &str) -> Result<()> {
    let mut chars = module_id.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() => {}
        _ => {
            return Err(Error::InvalidModuleID {
                module_id: module_id.to_owned(),
            });
        }
    }

    if chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' || ch == '-') {
        Ok(())
    } else {
        Err(Error::InvalidModuleID {
            module_id: module_id.to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_module_ids_accepted() {
        for id in ["ab", "a-b.c_d", "Z9", "module_name"] {
            assert!(ModuleId::try_from(id).is_ok(), "{id}");
        }
    }

    #[test]
    fn invalid_module_ids_rejected() {
        for id in ["", "1abc", "-abc", "_abc", ".abc", "ab/c", "ab cd"] {
            assert!(ModuleId::try_from(id).is_err(), "{id}");
        }
    }

    #[test]
    fn module_id_projects_to_str_without_reborrowing_validation() {
        let id = ModuleId::try_from("hosts_redirect").unwrap();
        assert_eq!(id.as_str(), "hosts_redirect");
        assert_eq!(id.to_string(), "hosts_redirect");
        assert_eq!(id, "hosts_redirect");
        assert_eq!(id, "hosts_redirect".to_owned());
        assert_eq!(<ModuleId as Borrow<str>>::borrow(&id), "hosts_redirect");
    }

    #[test]
    fn module_id_roundtrips_as_plain_string_in_json() {
        let id = ModuleId::try_from("hosts_redirect").unwrap();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, r#""hosts_redirect""#);
        assert_eq!(
            serde_json::from_str::<ModuleId>(&json).unwrap(),
            "hosts_redirect"
        );
    }

    #[test]
    fn invalid_json_module_id_fails_deserialization() {
        let err = serde_json::from_str::<ModuleId>(r#""1bad""#).unwrap_err();
        assert!(err.to_string().contains("Invalid module ID"), "{err}");
    }
}
