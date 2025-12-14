use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use crate::{defs, conf::config};
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MountMode {
    Overlay,
    HymoFs,
    Magic,
    Ignore,
}
impl Default for MountMode {
    fn default() -> Self {
        MountMode::Overlay
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModuleRules {
    #[serde(default)]
    pub default_mode: MountMode,
    #[serde(default)]
    pub paths: HashMap<String, MountMode>, 
}
impl ModuleRules {
    pub fn load(module_dir: &Path, module_id: &str) -> Self {
        let mut rules = ModuleRules::default();
        let internal_config = module_dir.join("hybrid_rules.json");
        if let Ok(content) = fs::read_to_string(&internal_config) {
            if let Ok(r) = serde_json::from_str::<ModuleRules>(&content) {
                rules = r;
            }
        }
        let user_rules_dir = Path::new("/data/adb/meta-hybrid/rules");
        let user_config = user_rules_dir.join(format!("{}.json", module_id));
        if let Ok(content) = fs::read_to_string(&user_config) {
            if let Ok(user_rules) = serde_json::from_str::<ModuleRules>(&content) {
                rules.default_mode = user_rules.default_mode;
                rules.paths.extend(user_rules.paths);
            }
        }
        rules
    }
    pub fn get_mode(&self, relative_path: &str) -> MountMode {
        if let Some(mode) = self.paths.get(relative_path) {
            return mode.clone();
        }
        self.default_mode.clone()
    }
}
#[derive(Debug, Clone)]
pub struct Module {
    pub id: String,
    pub source_path: PathBuf,
    pub rules: ModuleRules,
}
pub fn scan(source_dir: &Path, _config: &config::Config) -> Result<Vec<Module>> {
    let mut modules = Vec::new();
    if !source_dir.exists() {
        return Ok(modules);
    }
    for entry in fs::read_dir(source_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() { continue; }
        let id = entry.file_name().to_string_lossy().to_string();
        if id == "meta-hybrid" || id == "lost+found" || id == ".git" { continue; }
        if path.join(defs::DISABLE_FILE_NAME).exists() || 
           path.join(defs::REMOVE_FILE_NAME).exists() || 
           path.join(defs::SKIP_MOUNT_FILE_NAME).exists() { 
            continue; 
        }
        let rules = ModuleRules::load(&path, &id);
        modules.push(Module {
            id,
            source_path: path,
            rules,
        });
    }
    modules.sort_by(|a, b| b.id.cmp(&a.id));
    Ok(modules)
}
