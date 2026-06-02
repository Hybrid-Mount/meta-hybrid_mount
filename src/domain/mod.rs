// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum DefaultMode {
    #[default]
    Overlay,
    Magic,
    Kasumi,
}

impl DefaultMode {
    pub fn as_mount_mode(&self) -> MountMode {
        match self {
            Self::Overlay => MountMode::Overlay,
            Self::Magic => MountMode::Magic,
            Self::Kasumi => MountMode::Kasumi,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum MountMode {
    #[default]
    Overlay,
    Magic,
    Kasumi,
    Ignore,
}

impl MountMode {
    pub fn as_strategy(&self) -> &'static str {
        match self {
            Self::Overlay => "overlay",
            Self::Magic => "magic",
            Self::Kasumi => "kasumi",
            Self::Ignore => "ignore",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ModuleRules {
    pub default_mode: MountMode,
    pub paths: HashMap<String, MountMode>,
}

impl ModuleRules {
    pub fn get_mode(&self, relative_path: &str) -> MountMode {
        let mut candidate = Some(relative_path);
        while let Some(path) = candidate {
            if let Some(mode) = self.paths.get(path) {
                return *mode;
            }
            candidate = path.rsplit_once('/').map(|(parent, _)| parent);
        }

        self.default_mode
    }

    pub fn effective_mode(&self, relative_path: &Path, use_kasumi: bool) -> MountMode {
        let mode = self.get_mode(relative_path.to_string_lossy().as_ref());
        if matches!(mode, MountMode::Kasumi) && !use_kasumi {
            MountMode::Ignore
        } else {
            mode
        }
    }

    pub fn has_descendant_rule(&self, relative_path: &Path) -> bool {
        let relative = relative_path.to_string_lossy();
        let prefix = format!("{relative}/");
        self.paths.keys().any(|path| path.starts_with(&prefix))
    }

    pub fn descendant_rule_prefixes(&self) -> HashSet<String> {
        let mut prefixes = HashSet::new();
        for path in self.paths.keys() {
            let mut current = path.as_str();
            while let Some((parent, _)) = current.rsplit_once('/') {
                if parent.is_empty() {
                    break;
                }
                prefixes.insert(parent.to_string());
                current = parent;
            }
        }
        prefixes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_rules(default_mode: MountMode, paths: &[(&str, MountMode)]) -> ModuleRules {
        ModuleRules {
            default_mode,
            paths: paths.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
        }
    }

    #[test]
    fn exact_match_rules() {
        // Exact path match takes precedence over prefix
        let rules = make_rules(MountMode::Overlay, &[("system", MountMode::Magic)]);
        assert_eq!(rules.get_mode("system"), MountMode::Magic);

        // Duplicate keys: later entry overwrites (HashMap semantics)
        let rules = make_rules(
            MountMode::Overlay,
            &[("sys", MountMode::Magic), ("sys", MountMode::Kasumi)],
        );
        assert_eq!(rules.get_mode("sys"), MountMode::Kasumi);
    }

    #[test]
    fn prefix_match_rules() {
        // Prefix match: "system" covers "system/app"
        let rules = make_rules(MountMode::Overlay, &[("system", MountMode::Magic)]);
        assert_eq!(rules.get_mode("system/app"), MountMode::Magic);

        // "sys" is a substring, not a path-component prefix of "system"
        let rules = make_rules(MountMode::Overlay, &[("sys", MountMode::Magic)]);
        assert_eq!(rules.get_mode("system"), MountMode::Overlay);
    }

    #[test]
    fn longest_match_wins() {
        let rules = make_rules(
            MountMode::Overlay,
            &[
                ("system", MountMode::Magic),
                ("system/app", MountMode::Kasumi),
            ],
        );
        assert_eq!(rules.get_mode("system/app/foo"), MountMode::Kasumi);
        assert_eq!(rules.get_mode("system/priv-app"), MountMode::Magic);
    }

    #[test]
    fn default_mode_rules() {
        let rules = make_rules(MountMode::Ignore, &[]);
        assert_eq!(rules.get_mode("any/path"), MountMode::Ignore);

        let rules = make_rules(MountMode::Kasumi, &[]);
        assert_eq!(rules.get_mode("system"), MountMode::Kasumi);
    }

    #[test]
    fn trailing_slash_not_prefix() {
        // "system/" is not a prefix of "system" because the slash requires
        // deeper path components
        let rules = make_rules(MountMode::Overlay, &[("system/", MountMode::Magic)]);
        assert_eq!(rules.get_mode("system"), MountMode::Overlay);
    }

    #[test]
    fn descendant_rule_prefixes_include_rule_ancestors_only() {
        let rules = make_rules(
            MountMode::Overlay,
            &[
                ("system/app/private", MountMode::Magic),
                ("vendor/lib", MountMode::Kasumi),
            ],
        );
        let prefixes = rules.descendant_rule_prefixes();

        assert!(prefixes.contains("system"));
        assert!(prefixes.contains("system/app"));
        assert!(prefixes.contains("vendor"));
        assert!(!prefixes.contains("system/app/private"));
        assert!(!prefixes.contains("vendor/lib"));
    }
}
