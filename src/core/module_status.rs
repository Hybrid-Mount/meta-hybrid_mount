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
    fs,
    io::{BufRead, BufReader},
    path::Path,
};

use crate::{core::storage::StorageMode, defs, sys::fs::atomic_write};

pub fn update_description(
    storage_mode: StorageMode,
    kasumi_enabled: bool,
    overlay_count: usize,
    magic_count: usize,
    kasumi_count: usize,
    blacklisted_count: usize,
) {
    let prop_path = Path::new(defs::MODULE_PROP_FILE);

    if !prop_path.exists() {
        return;
    }

    let desc_text = running_description(
        storage_mode,
        kasumi_enabled,
        overlay_count,
        magic_count,
        kasumi_count,
        blacklisted_count,
    );

    set_description(prop_path, &desc_text);
}

fn running_description(
    storage_mode: StorageMode,
    kasumi_enabled: bool,
    overlay_count: usize,
    magic_count: usize,
    kasumi_count: usize,
    blacklisted_count: usize,
) -> String {
    let (mode_str, status_emoji) = match storage_mode {
        #[cfg(feature = "control-plane")]
        StorageMode::Tmpfs => ("Tmpfs", "🐾"),
        StorageMode::Ext4 => ("Ext4", "💿"),
    };

    let mut stats = Vec::new();
    if kasumi_enabled {
        stats.push(format!("Kasumi:{}", kasumi_count));
    }
    stats.push(format!("Overlay:{}", overlay_count));
    stats.push(format!("Magic:{}", magic_count));
    if blacklisted_count > 0 {
        stats.push(format!("Blacklist:{}", blacklisted_count));
    }

    let stats_str = stats.join("  ");

    format!(
        "😋 运行中喵～ ({}) {}  {}",
        mode_str, status_emoji, stats_str
    )
}

pub fn update_crash_description(reason: &str) {
    let prop_path = Path::new(defs::MODULE_PROP_FILE);

    if !prop_path.exists() {
        return;
    }

    let desc_text = format!("😭 崩溃了呜～ 原因: {}", reason);
    set_description(prop_path, &desc_text);
}

fn set_description(prop_path: &Path, desc_text: &str) {
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
    if let Err(err) = atomic_write(prop_path, format!("{}\n", content)) {
        crate::scoped_log!(
            warn,
            "module_status",
            "description update failed: path={}, error={}",
            prop_path.display(),
            err
        );
    }
}

#[cfg(test)]
mod tests {
    use super::running_description;
    use crate::core::storage::StorageMode;

    #[test]
    fn running_description_keeps_kasumi_zero_count_when_enabled() {
        #[cfg(feature = "control-plane")]
        let desc = running_description(StorageMode::Tmpfs, true, 2, 3, 0, 0);
        #[cfg(not(feature = "control-plane"))]
        let desc = running_description(StorageMode::Ext4, true, 2, 3, 0, 0);

        assert!(desc.contains("Kasumi:0"));
        assert!(desc.contains("Overlay:2"));
        assert!(desc.contains("Magic:3"));
    }

    #[test]
    fn running_description_hides_kasumi_count_when_disabled() {
        #[cfg(feature = "control-plane")]
        let desc = running_description(StorageMode::Tmpfs, false, 2, 3, 0, 0);
        #[cfg(not(feature = "control-plane"))]
        let desc = running_description(StorageMode::Ext4, false, 2, 3, 0, 0);

        assert!(!desc.contains("Kasumi:"));
        assert!(desc.contains("Overlay:2"));
        assert!(desc.contains("Magic:3"));
    }

    #[test]
    fn running_description_shows_blacklisted_count_when_nonzero() {
        #[cfg(feature = "control-plane")]
        let desc = running_description(StorageMode::Tmpfs, false, 2, 3, 0, 1);
        #[cfg(not(feature = "control-plane"))]
        let desc = running_description(StorageMode::Ext4, false, 2, 3, 0, 1);

        assert!(desc.contains("Blacklist:1"));
    }

    #[test]
    fn running_description_hides_blacklisted_count_when_zero() {
        #[cfg(feature = "control-plane")]
        let desc = running_description(StorageMode::Tmpfs, false, 2, 3, 0, 0);
        #[cfg(not(feature = "control-plane"))]
        let desc = running_description(StorageMode::Ext4, false, 2, 3, 0, 0);

        assert!(!desc.contains("Blacklist:"));
    }
}
