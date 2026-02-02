use std::{
    collections::HashSet,
    fs::{self},
    io::{BufRead, BufReader},
    path::Path,
    sync::OnceLock,
};

use anyhow::Result;
use regex_lite::Regex;
use serde::Serialize;

use super::scanner as inventory;
use crate::{
    conf::config::{self, MountMode},
    core::state::RuntimeState,
    defs, utils,
};

static MODULE_PROP_REGEX: OnceLock<Regex> = OnceLock::new();

#[derive(Default)]
struct ModuleProp {
    name: String,
    version: String,
    author: String,
    description: String,
}

impl From<&Path> for ModuleProp {
    fn from(path: &Path) -> Self {
        let mut prop = ModuleProp::default();
        let re = MODULE_PROP_REGEX.get_or_init(|| {
            Regex::new(r"^([a-zA-Z0-9_.]+)=(.*)$").expect("Failed to compile module prop regex")
        });

        if let Ok(file) = fs::File::open(path) {
            for line in BufReader::new(file).lines().map_while(Result::ok) {
                if let Some(caps) = re.captures(line.trim()) {
                    let k = caps.get(1).map_or("", |m| m.as_str());
                    let v = caps.get(2).map_or("", |m| m.as_str());

                    match k {
                        "name" => prop.name = v.to_string(),
                        "version" => prop.version = v.to_string(),
                        "author" => prop.author = v.to_string(),
                        "description" => prop.description = v.to_string(),
                        _ => {}
                    }
                }
            }
        }
        prop
    }
}

#[derive(Serialize)]
struct ModuleInfo {
    id: String,
    name: String,
    version: String,
    author: String,
    description: String,
    mode: String,
    is_mounted: bool,
    rules: config::ModuleRules,
}

impl ModuleInfo {
    fn new(m: inventory::Module, mounted_set: &HashSet<&str>) -> Self {
        let prop = ModuleProp::from(m.source_path.join("module.prop").as_path());

        let mode_str = match m.rules.default_mode {
            MountMode::Overlay => "auto",
            MountMode::Magic => "magic",
            MountMode::Ignore => "ignore",
        };

        Self {
            is_mounted: mounted_set.contains(m.id.as_str()),
            id: m.id,
            name: prop.name,
            version: prop.version,
            author: prop.author,
            description: prop.description,
            mode: mode_str.to_string(),
            rules: m.rules,
        }
    }
}

pub fn print_list(config: &config::Config) -> Result<()> {
    let modules = inventory::scan(&config.moduledir, config)?;

    let state = RuntimeState::load().unwrap_or_default();

    let mounted_ids: HashSet<&str> = state
        .overlay_modules
        .iter()
        .chain(state.magic_modules.iter())
        .map(|s| s.as_str())
        .collect();

    let infos: Vec<ModuleInfo> = modules
        .into_iter()
        .map(|m| ModuleInfo::new(m, &mounted_ids))
        .collect();

    println!("{}", serde_json::to_string(&infos)?);

    Ok(())
}

pub fn update_description(storage_mode: &str, overlay_count: usize, magic_count: usize) {
    let prop_path = Path::new(defs::MODULE_PROP_FILE);

    if !prop_path.exists() {
        return;
    }

    let mode_str = match storage_mode {
        "tmpfs" => "Tmpfs",
        "erofs" => "EROFS",
        _ => "Ext4",
    };

    let status_emoji = match storage_mode {
        "tmpfs" => "🐾",
        "erofs" => "🚀",
        _ => "💿",
    };

    let desc_text = format!(
        "description=😋 运行中喵～ ({}) {} | Overlay: {} | Magic: {}",
        mode_str, status_emoji, overlay_count, magic_count
    );

    let lines: Vec<String> = match fs::File::open(prop_path) {
        Ok(file) => BufReader::new(file)
            .lines()
            .map_while(Result::ok)
            .map(|line| {
                if line.starts_with("description=") {
                    desc_text.clone()
                } else {
                    line
                }
            })
            .collect(),
        Err(_) => return,
    };

    let content = lines.join("\n");
    if let Err(e) = utils::atomic_write(prop_path, format!("{}\n", content)) {
        log::warn!("Failed to update module description: {}", e);
    }
}
