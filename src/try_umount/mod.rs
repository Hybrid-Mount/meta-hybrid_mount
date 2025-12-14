mod kernel;

use std::{
    fs::{self, read_dir},
    path::Path,
};
use anyhow::Result;
use crate::defs::{DISABLE_FILE_NAME, REMOVE_FILE_NAME, SKIP_MOUNT_FILE_NAME};

#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::mount::hymofs::HymoFs;

pub fn send_unmountable<P>(target: P) -> Result<()>
where
    P: AsRef<Path>,
{
    for entry in read_dir("/data/adb/modules")?.flatten() {
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        if !path.join("module.prop").exists() {
            continue;
        }

        let disabled =
            path.join(DISABLE_FILE_NAME).exists() || path.join(REMOVE_FILE_NAME).exists();
        let skip = path.join(SKIP_MOUNT_FILE_NAME).exists();
        if disabled || skip {
            continue;
        }

        if let Some(name) = path.file_name() {
            if name.to_string_lossy().to_string().contains("zygisksu") {
                let is_denylist_enforce_off = fs::read_to_string("/data/adb/zygisksu/denylist_enforce")
                    .map(|s| s.trim() == "0")
                    .unwrap_or(false);
                
                if is_denylist_enforce_off {
                    log::warn!("zn was detected, and try_umount was cancelled.");
                    return Ok(());
                }
            }
        }
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    if HymoFs::is_available() {
        if let Err(e) = HymoFs::hide_path(&target.as_ref().to_string_lossy()) {
            log::warn!("HymoFS hide failed for {}: {}", target.as_ref().display(), e);
        } else {
             log::info!("HymoFS hide {} successful!", target.as_ref().display());
             return Ok(());
        }
    }

    kernel::send_kernel_umount(target.as_ref())?;
    Ok(())
}
