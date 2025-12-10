use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::Path;

use anyhow::{Context, Result};
use log::debug;
use walkdir::WalkDir;

const HYMO_CTL: &str = "/proc/hymo_ctl";
const EXPECTED_PROTOCOL_VERSION: i32 = 3;

pub struct HymoFs;

impl HymoFs {
    pub fn is_available() -> bool {
        if !Path::new(HYMO_CTL).exists() {
            return false;
        }
        
        if let Ok(content) = fs::read_to_string(HYMO_CTL) {
             if let Some(line) = content.lines().next() {
                 if let Some(ver_str) = line.strip_prefix("HymoFS Protocol: ") {
                     if let Ok(ver) = ver_str.parse::<i32>() {
                         return ver == EXPECTED_PROTOCOL_VERSION;
                     }
                 }
             }
        }
        false
    }

    fn send_cmd(cmd: &str) -> Result<()> {
        let mut file = File::create(HYMO_CTL)
            .with_context(|| format!("Failed to open {}", HYMO_CTL))?;
        writeln!(file, "{}", cmd)?;
        debug!("HymoFS Cmd: {}", cmd);
        Ok(())
    }

    pub fn clear() -> Result<()> {
        Self::send_cmd("clear")
    }

    pub fn add_rule(src: &Path, target: &Path, file_type: Option<u32>) -> Result<()> {
        let type_str = file_type.unwrap_or(0).to_string();
        let cmd = format!("add {} {} {}", src.display(), target.display(), type_str);
        Self::send_cmd(&cmd)
    }

    #[allow(dead_code)]
    pub fn delete_rule(src: &Path) -> Result<()> {
        Self::send_cmd(&format!("delete {}", src.display()))
    }

    pub fn hide_path(path: &Path) -> Result<()> {
        Self::send_cmd(&format!("hide {}", path.display()))
    }

    pub fn inject_dir(dir: &Path) -> Result<()> {
        Self::send_cmd(&format!("inject {}", dir.display()))
    }

    pub fn inject_directory(target_base: &Path, module_dir: &Path) -> Result<()> {
        if !module_dir.exists() || !module_dir.is_dir() {
            return Ok(());
        }

        Self::inject_dir(target_base)?;

        for entry in WalkDir::new(module_dir).min_depth(1) {
            let entry = entry?;
            let current_path = entry.path();
            
            let relative_path = current_path.strip_prefix(module_dir)?;
            let target_path = target_base.join(relative_path);
            let file_type = entry.file_type();

            if file_type.is_file() {
                Self::add_rule(&target_path, current_path, Some(8))?;
            } else if file_type.is_symlink() {
                Self::add_rule(&target_path, current_path, Some(10))?;
            } else if file_type.is_char_device() {
                let metadata = entry.metadata()?;
                if metadata.rdev() == 0 {
                    Self::hide_path(&target_path)?;
                }
            } else if file_type.is_dir() {
                Self::inject_dir(&target_path)?;
            }
        }
        Ok(())
    }
}
