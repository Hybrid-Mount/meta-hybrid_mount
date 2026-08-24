// ReHybrid-Mount
//
// SPDX-License-Identifier: GPL-3.0-only

//! Dynamic module-manager description for the completed mount plan.

use std::fs;
use std::path::Path;
#[cfg(any(target_os = "linux", target_os = "android"))]
use std::process::Command;

#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::defs;
use crate::errors::{Error, Result};

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn update_description(mode: &str, overlay_count: usize, magic_count: usize) {
    let prop_path = Path::new(defs::SELF_MODULE_PROP);
    if !prop_path.exists() {
        log::warn!(
            "module description update skipped: {} does not exist",
            prop_path.display()
        );
        return;
    }

    let description = running_description(mode, overlay_count, magic_count);
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

fn running_description(mode: &str, overlay_count: usize, magic_count: usize) -> String {
    let (mode_name, mode_icon) = match mode {
        "tmpfs" => ("Tmpfs", "🐾"),
        _ => ("Ext4", "💿"),
    };

    format!(
        "😋 运行中喵～ ({mode_name}) {mode_icon} | OverlayFS: {overlay_count} | Magic Mount: {magic_count}"
    )
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn set_temporary_override(description: &str) -> bool {
    let (program, module_env) = if crate::utils::ksu::is_active() {
        ("ksud", "KSU_MODULE")
    } else {
        ("apd", "AP_MODULE")
    };

    match Command::new(program)
        .args([
            "module",
            "config",
            "set",
            "override.description",
            description,
            "--temp",
        ])
        .env(module_env, defs::MODULE_ID)
        .output()
    {
        Ok(output) if output.status.success() => true,
        Ok(output) => {
            log::warn!(
                "{program} description override failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
            false
        }
        Err(err) => {
            log::debug!("{program} description override unavailable: {err}");
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
    crate::sys::fs::atomic_write(prop_path, updated.as_bytes())
        .map_err(|err| Error::msg(format!("atomically replace {}: {err}", prop_path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn running_description_reports_both_backend_counts() {
        let description = running_description("ext4", 2, 3);

        assert!(description.contains("(Ext4)"));
        assert!(description.contains("OverlayFS: 2"));
        assert!(description.contains("Magic Mount: 3"));
    }

    #[test]
    fn running_description_reports_tmpfs_mode() {
        let description = running_description("tmpfs", 0, 1);

        assert!(description.contains("(Tmpfs)"));
        assert!(description.contains("OverlayFS: 0"));
        assert!(description.contains("Magic Mount: 1"));
    }

    #[test]
    fn fallback_replaces_only_description_and_keeps_trailing_newline() {
        let dir =
            std::env::temp_dir().join(format!("rehybrid-mount-description-{}", std::process::id()));
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
