use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use anyhow::Result;

use crate::defs::{DISABLE_FILE_NAME, REMOVE_FILE_NAME, SKIP_MOUNT_FILE_NAME};

#[derive(PartialEq, Eq, Hash, Clone, PartialOrd)]
pub struct ModuleInfo {
    pub magic_mount: bool,
    pub overlayfs: bool,
    pub id: String,
}

pub struct ModuleScanner {
    modules: HashSet<ModuleInfo>,
    extra: Vec<String>,
    path: PathBuf,
}

impl ModuleScanner {
    pub fn new<P>(path: P, extra: Vec<String>) -> Self
    where
        P: AsRef<Path>,
    {
        Self {
            modules: HashSet::new(),
            extra: extra,
            path: path.as_ref().to_path_buf(),
        }
    }

    pub fn scanner(&mut self) -> Result<HashSet<ModuleInfo>> {
        log::info!("Starting san modules!");
        let mut modules = HashSet::new();

        if let Ok(entries) = self.path.read_dir() {
            for p in entries.flatten() {
                let path = p.path();
                if !path.is_dir() {
                    continue;
                }

                let mut modified = false;
                let mut partitions = HashSet::new();
                partitions.insert("system".to_string());
                partitions.extend(self.extra.clone());

                for p in &partitions {
                    if path.join(p).is_dir() {
                        modified = true;
                        break;
                    }
                }

                if !modified {
                    continue;
                }

                let disabled =
                    path.join(DISABLE_FILE_NAME).exists() || path.join(REMOVE_FILE_NAME).exists();
                let skip = path.join(SKIP_MOUNT_FILE_NAME).exists();
                if disabled || skip {
                    continue;
                }

                modules.insert(ModuleInfo {
                    id: p
                        .file_name()
                        .to_str()
                        .unwrap_or_else(|| "unknown")
                        .to_string(),
                    magic_mount: path.join(".magic_mount").exists(),
                    overlayfs: path.join(".overlayfs").exists(),
                });
            }
        }

        Ok(modules)
    }
}
