use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::{Path, PathBuf};
use std::collections::HashSet;
use anyhow::Result;
use serde::Serialize;
use crate::conf::config::Config;
use crate::core::inventory;
use crate::defs;
use crate::core::state::RuntimeState;
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
        let is_whiteout = if file_type.is_char_device() {
            metadata.rdev() == 0
        } else {
            false
        };
        let is_replace = if file_type.is_dir() {
            real_path.join(defs::REPLACE_DIR_FILE_NAME).exists()
        } else {
            false
        };
        let file_name = real_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let is_replace_file = file_name == defs::REPLACE_DIR_FILE_NAME;
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
    let mut mounted_ids = HashSet::new();
    mounted_ids.extend(state.overlay_modules);
    mounted_ids.extend(state.magic_modules);
    mounted_ids.extend(state.hymo_modules);
    let mut infos = Vec::new();
    for m in modules {
        let prop_path = m.source_path.join("module.prop");
        let (name, version, author, description) = read_module_prop(&prop_path);
        let mode_str = match m.rules.default_mode {
            inventory::MountMode::Overlay => "auto",
            inventory::MountMode::HymoFs => "hymofs",
            inventory::MountMode::Magic => "magic",
            inventory::MountMode::Ignore => "ignore",
        };
        infos.push(ModuleInfo {
            id: m.id.clone(),
            name,
            version,
            author,
            description,
            mode: mode_str.to_string(),
            is_mounted: mounted_ids.contains(&m.id),
            rules: m.rules,
        });
    }
    println!("{}", serde_json::to_string(&infos)?);
    Ok(())
}
fn read_module_prop(path: &Path) -> (String, String, String, String) {
    let mut name = String::new();
    let mut version = String::new();
    let mut author = String::new();
    let mut description = String::new();
    if let Ok(file) = fs::File::open(path) {
        let reader = BufReader::new(file);
        for line in reader.lines() {
            if let Ok(l) = line {
                if let Some((k, v)) = l.split_once('=') {
                    let val = v.trim().to_string();
                    match k.trim() {
                        "name" => name = val,
                        "version" => version = val,
                        "author" => author = val,
                        "description" => description = val,
                        _ => {}
                    }
                }
            }
        }
    }
    (name, version, author, description)
}
pub fn update_description(
    storage_mode: &str, 
    nuke_active: bool, 
    overlay_count: usize, 
    magic_count: usize, 
    hymo_count: usize
) {
    let prop_path = Path::new(defs::MODULE_PROP_FILE);
    if !prop_path.exists() {
        return;
    }
    let mode_str = if storage_mode == "tmpfs" { "Tmpfs" } else { "Ext4" };
    let status_emoji = if storage_mode == "tmpfs" { "🐾" } else { "💿" };
    let nuke_str = if nuke_active { " | 肉垫: 开启 ✨" } else { "" };
    let desc_text = format!(
        "description=😋 运行中喵～ ({}) {} | Hymo: {} | Overlay: {} | Magic: {}{}", 
        mode_str, status_emoji, hymo_count, overlay_count, magic_count, nuke_str
    );
    let mut lines = Vec::new();
    if let Ok(file) = fs::File::open(prop_path) {
        let reader = BufReader::new(file);
        for line in reader.lines() {
            if let Ok(l) = line {
                if l.starts_with("description=") {
                    lines.push(desc_text.clone());
                } else {
                    lines.push(l);
                }
            }
        }
    }
    if let Ok(mut file) = OpenOptions::new().write(true).truncate(true).open(prop_path) {
        for line in lines {
            let _ = writeln!(file, "{}", line);
        }
    }
}
