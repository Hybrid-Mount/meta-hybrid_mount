// Copyright 2025 Meta-Hybrid Mount Authors
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    collections::HashSet,
    fs::{self},
    io::{BufRead, BufReader},
    os::unix::fs::{FileTypeExt, MetadataExt},
    path::{Path, PathBuf},
};

use anyhow::Result;
#[cfg(any(target_os = "linux", target_os = "android"))]
use extattr::lgetxattr;
use serde::Serialize;

use crate::{
    conf::config::Config,
    core::{
        inventory::{self, MountMode},
        state::RuntimeState,
    },
    defs, utils,
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

pub struct ModuleFile {
    pub relative_path: PathBuf,
    pub real_path: PathBuf,
    pub file_type: fs::FileType,
    pub is_whiteout: bool,
    pub is_replace: bool,
    pub is_replace_file: bool,
}

impl ModuleFile {
    pub fn new(root: &Path, relative: &Path) -> Result<Self> {
        let real_path = root.join(relative);

        let metadata = fs::symlink_metadata(&real_path)?;

        let file_type = metadata.file_type();

        let is_whiteout = file_type.is_char_device() && metadata.rdev() == 0;

        let check_replace = || -> bool {
            if real_path.join(defs::REPLACE_DIR_FILE_NAME).exists() {
                return true;
            }

            #[cfg(any(target_os = "linux", target_os = "android"))]
            if let Ok(val) = lgetxattr(&real_path, defs::REPLACE_DIR_XATTR) {
                return String::from_utf8_lossy(&val) == "y";
            }

            false
        };

        let is_replace = file_type.is_dir() && check_replace();

        let is_replace_file = real_path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s == defs::REPLACE_DIR_FILE_NAME)
            .unwrap_or(false);

        Ok(Self {
            relative_path: relative.to_path_buf(),
            real_path,
            file_type,
            is_whiteout,
            is_replace,
            is_replace_file,
        })
    }
}

pub fn print_list(config: &Config) -> Result<()> {
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

pub fn update_description(
    storage_mode: &str,
    nuke_active: bool,
    overlay_count: usize,
    magic_count: usize,
) {
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

    let nuke_str = if nuke_active {
        " | 肉垫: 开启 ✨"
    } else {
        ""
    };

    let desc_text = format!(
        "description=😋 运行中喵～ ({}) {} | Overlay: {} | Magic: {}{}",
        mode_str, status_emoji, overlay_count, magic_count, nuke_str
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
