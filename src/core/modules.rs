use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::Path,
};

use anyhow::Result;
use serde::Serialize;

use crate::{
    conf::config::Config,
    core::{
        inventory::{self, MountMode},
        state::RuntimeState,
    },
    defs,
};

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
        if let Ok(file) = fs::File::open(path) {
            for line in BufReader::new(file).lines().map_while(Result::ok) {
                if let Some((k, v)) = line.split_once('=') {
                    let val = v.trim().to_string();
                    match k.trim() {
                        "name" => prop.name = val,
                        "version" => prop.version = val,
                        "author" => prop.author = val,
                        "description" => prop.description = val,
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
    rules: inventory::ModuleRules,
}

impl ModuleInfo {
    fn new(m: inventory::Module, mounted_set: &HashSet<&str>) -> Self {
        let prop = ModuleProp::from(m.source_path.join("module.prop").as_path());

        let mode_str = match m.rules.default_mode {
            MountMode::Overlay => "auto",
            MountMode::HymoFs => "hymofs",
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

pub fn print_list(config: &Config) -> Result<()> {
    let modules = inventory::scan(&config.moduledir, config)?;
    let state = RuntimeState::load().unwrap_or_default();
    let mounted_ids: HashSet<&str> = state
        .overlay_modules
        .iter()
        .chain(state.magic_modules.iter())
        .chain(state.hymo_modules.iter())
        .map(|s| s.as_str())
        .collect();
    let infos: Vec<ModuleInfo> = modules
        .into_iter()
        .map(|m| ModuleInfo::new(m, &mounted_ids))
        .collect();

    println!("{}", serde_json::to_string(&infos)?);
    Ok(())
}

pub fn update_description(
    storage_mode: &str,
    nuke_active: bool,
    overlay_count: usize,
    magic_count: usize,
    hymo_count: usize,
) {
    let prop_path = Path::new(defs::MODULE_PROP_FILE);
    if !prop_path.exists() {
        return;
    }

    let mode_str = if storage_mode == "tmpfs" {
        "Tmpfs"
    } else {
        "Ext4"
    };
    let status_emoji = if storage_mode == "tmpfs" {
        "🐾"
    } else {
        "💿"
    };
    let nuke_str = if nuke_active {
        " | 肉垫: 开启 ✨"
    } else {
        ""
    };

    let desc_text = format!(
        "description=😋 运行中喵～ ({}) {} | Hymo: {} | Overlay: {} | Magic: {}{}",
        mode_str, status_emoji, hymo_count, overlay_count, magic_count, nuke_str
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

    if let Ok(mut file) = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(prop_path)
    {
        for line in lines {
            let _ = writeln!(file, "{}", line);
        }
    }
}
