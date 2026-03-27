// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    collections::HashSet,
    fs::{self},
    io::{BufRead, BufReader},
    path::Path,
    process::Command,
    sync::{OnceLock, atomic::Ordering},
};

use anyhow::Result;
use regex_lite::Regex;
use serde::Serialize;

use super::scanner as inventory;
use crate::{
    conf::config::{self, MountMode},
    core::state::RuntimeState,
    defs,
    sys::fs::atomic_write,
    utils::KSU,
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
        _ => "Ext4",
    };

    let status_emoji = match storage_mode {
        "tmpfs" => "🐾",
        _ => "💿",
    };

    let desc_text = format!(
        "😋 运行中喵～ ({}) {} | Overlay: {} | Magic: {}",
        mode_str, status_emoji, overlay_count, magic_count
    );
    set_description(prop_path, &desc_text);
}

pub fn update_crash_description(reason: &str) {
    let prop_path = Path::new(defs::MODULE_PROP_FILE);

    if !prop_path.exists() {
        return;
    }

    let desc_text = format!("😭 崩溃了呜～ | 原因: {}", reason);
    set_description(prop_path, &desc_text);
}

fn set_description(prop_path: &Path, desc_text: &str) {
    if KSU.load(Ordering::Relaxed) {
        let result = Command::new("ksud")
            .arg("module")
            .arg("config")
            .arg("set")
            .arg("override.description")
            .arg(desc_text)
            .status();

        if let Ok(status) = result
            && status.success()
        {
            return;
        }
    }

    let lines: Vec<String> = match fs::File::open(prop_path) {
        Ok(file) => BufReader::new(file)
            .lines()
            .map_while(Result::ok)
            .map(|line| {
                if line.starts_with("description=") {
                    format!("description={}", desc_text)
                } else {
                    line
                }
            })
            .collect(),
        Err(_) => return,
    };

    let content = lines.join("\n");
    if let Err(e) = atomic_write(prop_path, format!("{}\n", content)) {
        log::warn!("Failed to update module description: {}", e);
    }
}
