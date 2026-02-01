use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::{defs, utils::fs::xattr};

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct RuntimeState {
    pub timestamp: u64,
    pub pid: u32,
    pub storage_mode: String,
    pub mount_point: PathBuf,
    pub overlay_modules: Vec<String>,
    pub magic_modules: Vec<String>,
    #[serde(default)]
    pub active_mounts: Vec<String>,
    #[serde(default)]
    pub storage_total: u64,
    #[serde(default)]
    pub storage_used: u64,
    #[serde(default)]
    pub storage_percent: u8,
    #[serde(default)]
    pub zygisksu_enforce: bool,
    #[serde(default)]
    pub tmpfs_xattr_supported: bool,
}

impl RuntimeState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        storage_mode: String,
        mount_point: PathBuf,
        overlay_modules: Vec<String>,
        magic_modules: Vec<String>,
        active_mounts: Vec<String>,
        storage_info: (u64, u64, u8),
    ) -> Self {
        let start = SystemTime::now();

        let timestamp = start
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let pid = std::process::id();

        let zygisksu_enforce = crate::utils::check_zygisksu_enforce_status();
        let tmpfs_xattr_supported = xattr::is_overlay_xattr_supported().unwrap_or(false);

        Self {
            timestamp,
            pid,
            storage_mode,
            mount_point,
            overlay_modules,
            magic_modules,
            active_mounts,
            storage_total: storage_info.0,
            storage_used: storage_info.1,
            storage_percent: storage_info.2,
            zygisksu_enforce,
            tmpfs_xattr_supported,
        }
    }

    pub fn save(&self) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;

        fs::write(defs::STATE_FILE, json)?;

        Ok(())
    }

    pub fn load() -> Result<Self> {
        if !std::path::Path::new(defs::STATE_FILE).exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(defs::STATE_FILE)?;

        let state = serde_json::from_str(&content)?;

        Ok(state)
    }
}
