// SPDX-License-Identifier: GPL-3.0-only

//! Dynamic module-manager description for the completed mount plan.

use std::fs;
use std::path::Path;
#[cfg(any(target_os = "linux", target_os = "android"))]
use std::time::Duration;

#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::defs;
use crate::errors::{Error, IoError, Result};
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::sys::process::{CaptureMode, CommandSpec, ProcessErrorKind, run_command};

/// Updating the ksud/apd description is a best-effort side effect: only a short total timeout, never slowing the boot.
#[cfg(any(target_os = "linux", target_os = "android"))]
const DESCRIPTION_OVERRIDE_TIMEOUT: Duration = Duration::from_secs(15);

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn update_description(mode: &str, overlay_count: usize, magic_count: usize, vfs_count: usize) {
    let prop_path = Path::new(defs::SELF_MODULE_PROP);
    if !prop_path.exists() {
        log::warn!(
            "module description update skipped: {} does not exist",
            prop_path.display()
        );
        return;
    }

    let description = running_description(mode, overlay_count, magic_count, vfs_count);
    if set_temporary_override(&description) {
        log::debug!("temporary module description override updated");
        return;
    }

    if let Err(err) = replace_description(prop_path, &description) {
        log::warn!(
            "module description fallback failed: path={}, error={err}",
            prop_path.display()
        );
    }
}

fn running_description(
    mode: &str,
    overlay_count: usize,
    magic_count: usize,
    vfs_count: usize,
) -> String {
    // Only the two real overlay staging backends may claim a tag. A VFS-only or
    // Magic-only boot starts neither, so it must report no storage backend instead of
    // falling through to Ext4.
    let (mode_name, mode_icon) = match mode {
        "tmpfs" => ("Tmpfs", "🐾"),
        "ext4" => ("Ext4", "💿"),
        _ => ("", ""),
    };

    let mode_tag = if mode_name.is_empty() {
        String::new()
    } else {
        format!(" ({mode_name}) {mode_icon}")
    };

    format!(
        "😋 运行中喵～{mode_tag} | OverlayFS: {overlay_count} | Magic Mount: {magic_count} | VFS: {vfs_count}"
    )
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn set_temporary_override(description: &str) -> bool {
    let (program, module_env) = if crate::utils::ksu::is_active() {
        ("ksud", "KSU_MODULE")
    } else {
        ("apd", "AP_MODULE")
    };

    let spec = CommandSpec::new(program)
        .operation("update temporary module description")
        .args([
            "module",
            "config",
            "set",
            "override.description",
            description,
            "--temp",
        ])
        .env(module_env, defs::MODULE_ID)
        .capture(CaptureMode::None)
        .timeout(DESCRIPTION_OVERRIDE_TIMEOUT);

    match run_command(&spec) {
        Ok(_) => true,
        Err(err) if matches!(&err.kind, ProcessErrorKind::Spawn { .. }) => {
            log::debug!("{program} description override unavailable: {err}");
            false
        }
        Err(err) => {
            log::warn!("{program} description override failed: {err}");
            false
        }
    }
}

fn replace_description(prop_path: &Path, description: &str) -> Result<()> {
    let content = fs::read_to_string(prop_path)?;
    let mut found = false;
    let mut lines = content
        .lines()
        .map(|line| {
            if line.starts_with("description=") {
                found = true;
                format!("description={description}")
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>();

    if !found {
        lines.push(format!("description={description}"));
    }

    let updated = format!("{}\n", lines.join("\n"));
    crate::sys::fs::atomic_write(prop_path, updated.as_bytes()).map_err(|err| match err {
        Error::Io(source) => Error::IoContext(Box::new(IoError::new(
            "atomically replace module description",
            Some(prop_path.to_path_buf()),
            source,
        ))),
        other => other,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn running_description_reports_all_backend_counts() {
        let description = running_description("ext4", 2, 3, 4);

        assert!(description.contains("(Ext4)"));
        assert!(description.contains("OverlayFS: 2"));
        assert!(description.contains("Magic Mount: 3"));
        assert!(description.contains("VFS: 4"));
    }

    #[test]
    fn running_description_reports_tmpfs_mode() {
        let description = running_description("tmpfs", 0, 1, 2);

        assert!(description.contains("(Tmpfs)"));
        assert!(description.contains("OverlayFS: 0"));
        assert!(description.contains("Magic Mount: 1"));
        assert!(description.contains("VFS: 2"));
    }

    /// HM-RUST-014：Magic-only 运行时 storage_mode="none"，不能假装在跑 Ext4。
    #[test]
    fn running_description_with_none_reports_no_storage_backend() {
        let description = running_description("none", 0, 1, 0);

        assert!(description.contains("OverlayFS: 0"));
        assert!(description.contains("Magic Mount: 1"));
        assert!(description.contains("VFS: 0"));
        assert!(
            !description.contains("Ext4"),
            "should not show Ext4 for Magic-only run"
        );
    }

    /// VFS-only 启动既不建 tmpfs 也不建 ext4，描述必须只报 VFS 计数。
    #[test]
    fn running_description_without_overlay_storage_reports_vfs_only() {
        let description = running_description("none", 0, 0, 3);

        assert!(description.contains("OverlayFS: 0"));
        assert!(description.contains("Magic Mount: 0"));
        assert!(description.contains("VFS: 3"));
        assert!(
            !description.contains("Ext4"),
            "VFS-only run must not claim Ext4"
        );
        assert!(
            !description.contains("Tmpfs"),
            "VFS-only run must not claim Tmpfs"
        );
    }

    /// 空字符串是旧快照的遗留值，同样不能回落到 Ext4。
    #[test]
    fn running_description_with_empty_mode_reports_no_storage_backend() {
        let description = running_description("", 0, 0, 2);

        assert!(description.contains("VFS: 2"));
        assert!(!description.contains("Ext4"));
        assert!(!description.contains("Tmpfs"));
    }

    #[test]
    fn fallback_replaces_only_description_and_keeps_trailing_newline() {
        let dir =
            std::env::temp_dir().join(format!("hybrid-mount-description-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let prop = dir.join("module.prop");
        fs::write(&prop, "id=hybrid_mount\ndescription=waiting\nversion=6\n").unwrap();

        replace_description(&prop, "OverlayFS: 4 | Magic Mount: 5").unwrap();

        assert_eq!(
            fs::read_to_string(&prop).unwrap(),
            "id=hybrid_mount\ndescription=OverlayFS: 4 | Magic Mount: 5\nversion=6\n"
        );
        fs::remove_dir_all(dir).ok();
    }
}
