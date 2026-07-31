// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::{
    fs,
    io::Write,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::Path,
    process::Command,
};

use anyhow::{Context, Result};
use rustix::fs::{Gid, Uid, chown};

use crate::{
    core::storage::StorageMode,
    defs,
    sys::fs::{lgetfilecon, lsetfilecon},
};

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

    let description = running_description(
        storage_mode,
        kasumi_enabled,
        overlay_count,
        magic_count,
        kasumi_count,
        blacklisted_count,
    );
    set_description(prop_path, &description);
}

fn running_description(
    storage_mode: StorageMode,
    _kasumi_enabled: bool,
    overlay_count: usize,
    magic_count: usize,
    _kasumi_count: usize,
    blacklisted_count: usize,
) -> String {
    let mode = match storage_mode {
        #[cfg(feature = "control-plane")]
        StorageMode::Tmpfs => "Tmpfs",
        StorageMode::Ext4 => "Ext4",
    };

    let mut stats = Vec::new();
    #[cfg(feature = "kasumi")]
    if _kasumi_enabled {
        stats.push(format!("Kasumi:{}", _kasumi_count));
    }
    stats.push(format!("Overlay:{}", overlay_count));
    stats.push(format!("Magic:{}", magic_count));
    if blacklisted_count > 0 {
        stats.push(format!("Blacklist:{}", blacklisted_count));
    }

    format!("Running ({mode}) | {}", stats.join(" | "))
}

fn set_description(prop_path: &Path, description: &str) {
    let command = if crate::utils::KSU.load(std::sync::atomic::Ordering::Relaxed) {
        "ksud"
    } else {
        "apd"
    };
    let result = Command::new(command)
        .args([
            "module",
            "config",
            "set",
            "override.description",
            description,
            "--temp",
        ])
        .envs([
            ("KSU_MODULE", env!("MODULE_ID")),
            ("AP_MODULE", env!("MODULE_ID")),
        ])
        .output();

    match result {
        Ok(output) if output.status.success() => return,
        Ok(output) => crate::scoped_log!(
            warn,
            "module_status",
            "override failed: command={}, error={}",
            command,
            String::from_utf8_lossy(&output.stderr).trim()
        ),
        Err(error) => crate::scoped_log!(
            warn,
            "module_status",
            "override failed: command={}, error={}",
            command,
            error
        ),
    }

    if let Err(error) = rewrite_description(prop_path, description) {
        crate::scoped_log!(
            warn,
            "module_status",
            "description update failed: path={}, error={}",
            prop_path.display(),
            error
        );
    }
}

fn rewrite_description(prop_path: &Path, description: &str) -> Result<()> {
    let metadata = fs::metadata(prop_path)
        .with_context(|| format!("failed to inspect {}", prop_path.display()))?;
    let content = fs::read_to_string(prop_path)
        .with_context(|| format!("failed to read {}", prop_path.display()))?;
    let content = content
        .lines()
        .map(|line| {
            if line.starts_with("description=") {
                format!("description={description}")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let parent = prop_path.parent().unwrap_or_else(|| Path::new("."));
    let mut tempfile = tempfile::Builder::new()
        .tempfile_in(parent)
        .with_context(|| format!("failed to create temporary file in {}", parent.display()))?;
    tempfile
        .write_all(format!("{content}\n").as_bytes())
        .with_context(|| format!("failed to write replacement for {}", prop_path.display()))?;
    tempfile
        .as_file()
        .set_permissions(fs::Permissions::from_mode(metadata.mode()))
        .with_context(|| format!("failed to preserve permissions for {}", prop_path.display()))?;
    chown(
        tempfile.path(),
        Some(Uid::from_raw(metadata.uid())),
        Some(Gid::from_raw(metadata.gid())),
    )
    .with_context(|| format!("failed to preserve ownership for {}", prop_path.display()))?;
    if let Ok(context) = lgetfilecon(prop_path) {
        lsetfilecon(tempfile.path(), &context).with_context(|| {
            format!(
                "failed to preserve SELinux context for {}",
                prop_path.display()
            )
        })?;
    }
    tempfile
        .as_file()
        .sync_all()
        .with_context(|| format!("failed to sync replacement for {}", prop_path.display()))?;
    tempfile
        .persist(prop_path)
        .map(|_| ())
        .with_context(|| format!("failed to atomically replace {}", prop_path.display()))
}

#[cfg(test)]
mod tests {
    use std::{fs, os::unix::fs::PermissionsExt};

    use super::{rewrite_description, running_description};
    use crate::core::storage::StorageMode;

    #[test]
    fn running_description_reports_mount_modes() {
        let description = running_description(StorageMode::Ext4, false, 9, 2, 0, 1);

        assert!(description.contains("Overlay:9"));
        assert!(description.contains("Magic:2"));
        assert!(description.contains("Blacklist:1"));
    }

    #[test]
    fn description_rewrite_preserves_file_mode_and_other_properties() {
        let temp = tempfile::tempdir().unwrap();
        let prop = temp.path().join("module.prop");
        fs::write(
            &prop,
            "id=hybrid_mount\ndescription=Old description\nversion=1\n",
        )
        .unwrap();
        fs::set_permissions(&prop, fs::Permissions::from_mode(0o644)).unwrap();

        rewrite_description(&prop, "Running").unwrap();

        assert_eq!(
            fs::read_to_string(&prop).unwrap(),
            "id=hybrid_mount\ndescription=Running\nversion=1\n"
        );
        assert_eq!(
            fs::metadata(&prop).unwrap().permissions().mode() & 0o777,
            0o644
        );
    }

    #[test]
    fn description_rewrite_preserves_invalid_utf8_file() {
        let temp = tempfile::tempdir().unwrap();
        let prop = temp.path().join("module.prop");
        let original = b"id=hybrid_mount\ndescription=Old\ninvalid=\xff\n";
        fs::write(&prop, original).unwrap();

        assert!(rewrite_description(&prop, "Running").is_err());
        assert_eq!(fs::read(&prop).unwrap(), original);
    }
}
