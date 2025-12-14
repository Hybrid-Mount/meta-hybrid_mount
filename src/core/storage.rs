use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::ffi::CString;
use anyhow::{Context, Result, bail};
use rustix::fs::Mode;
use rustix::mount::{unmount, UnmountFlags};
use serde::Serialize;
use crate::{defs, utils, mount::hymofs::HymoFs};
use crate::core::state::RuntimeState;

const DEFAULT_SELINUX_CONTEXT: &str = "u:object_r:system_file:s0";
const SELINUX_XATTR_KEY: &str = "security.selinux";

pub struct StorageHandle {
    pub mount_point: PathBuf,
    pub mode: String,
}

#[derive(Serialize)]
struct StorageStatus {
    #[serde(rename = "type")]
    mode: String,
    mount_point: String,
    usage_percent: u8,
    total_size: u64,
    used_size: u64,
    hymofs_available: bool,
    hymofs_version: Option<i32>,
}

pub fn get_usage(path: &Path) -> (u64, u64, u8) {
    if let Ok(stat) = rustix::fs::statvfs(path) {
        let total = stat.f_blocks * stat.f_frsize;
        let free = stat.f_bfree * stat.f_frsize;
        let used = total - free;
        let percent = if total > 0 { (used * 100 / total) as u8 } else { 0 };
        (total, used, percent)
    } else {
        (0, 0, 0)
    }
}

pub fn is_hymofs_active() -> bool {
    HymoFs::is_available()
}

pub fn setup(mnt_base: &Path, img_path: &Path, force_ext4: bool, mount_source: &str) -> Result<StorageHandle> {
    if utils::is_mounted(mnt_base) {
        let _ = unmount(mnt_base, UnmountFlags::DETACH);
    }
    
    fs::create_dir_all(mnt_base)?;

    if !force_ext4 {
        if try_setup_tmpfs(mnt_base, mount_source)? {
            return Ok(StorageHandle {
                mount_point: mnt_base.to_path_buf(),
                mode: "tmpfs".to_string(),
            });
        }
    }

    setup_ext4_image(mnt_base, img_path)
}

fn try_setup_tmpfs(target: &Path, mount_source: &str) -> Result<bool> {
    if utils::mount_tmpfs(target, mount_source).is_ok() {
        if utils::is_xattr_supported(target) {
            return Ok(true);
        } else {
            let _ = unmount(target, UnmountFlags::DETACH);
        }
    }
    Ok(false)
}

fn setup_ext4_image(target: &Path, img_path: &Path) -> Result<StorageHandle> {
    if !img_path.exists() {
        if let Some(parent) = img_path.parent() {
            fs::create_dir_all(parent)?;
        }
        create_image(img_path).context("Failed to create modules.img")?;
    }

    if let Err(_) = utils::mount_image(img_path, target) {
        if utils::repair_image(img_path).is_ok() {
            utils::mount_image(img_path, target).context("Failed to mount modules.img after repair")?;
        } else {
            bail!("Failed to repair modules.img");
        }
    }

    Ok(StorageHandle {
        mount_point: target.to_path_buf(),
        mode: "ext4".to_string(),
    })
}

fn create_image(path: &Path) -> Result<()> {
    let status = Command::new("truncate")
        .arg("-s").arg("2G")
        .arg(path)
        .status()?;
    if !status.success() { bail!("Failed to allocate image file"); }
    
    let status = Command::new("mkfs.ext4")
        .arg("-O").arg("^has_journal")
        .arg(path)
        .status()?;
    if !status.success() { bail!("Failed to format image file"); }

    Ok(())
}

#[allow(dead_code)]
pub fn finalize_storage_permissions(target: &Path) {
    if let Err(e) = rustix::fs::chmod(target, Mode::from(0o755)) {
        log::warn!("Failed to chmod storage root: {}", e);
    }
    if let Err(e) = rustix::fs::chown(target, Some(rustix::fs::Uid::from_raw(0)), Some(rustix::fs::Gid::from_raw(0))) {
        log::warn!("Failed to chown storage root: {}", e);
    }
    if let Err(e) = set_selinux_context(target, DEFAULT_SELINUX_CONTEXT) {
        log::warn!("Failed to set SELinux context: {}", e);
    }
}

fn set_selinux_context(path: &Path, context: &str) -> Result<()> {
    let c_path = CString::new(path.as_os_str().as_encoded_bytes())?;
    let c_val = CString::new(context)?;
    
    unsafe {
        let ret = libc::lsetxattr(
            c_path.as_ptr(),
            SELINUX_XATTR_KEY.as_ptr() as *const libc::c_char,
            c_val.as_ptr() as *const libc::c_void,
            c_val.as_bytes().len(),
            0
        );
        if ret != 0 {
            bail!("lsetxattr failed");
        }
    }
    Ok(())
}

pub fn print_status() -> Result<()> {
    let state = RuntimeState::load().ok();
    let (mnt_base, expected_mode) = if let Some(ref s) = state {
        (s.mount_point.clone(), s.storage_mode.clone())
    } else {
        (PathBuf::from(defs::FALLBACK_CONTENT_DIR), "unknown".to_string())
    };

    let mut mode = "unknown".to_string();
    let mut total = 0;
    let mut used = 0;
    let mut percent = 0;

    if utils::is_mounted(&mnt_base) {
        if let Ok(stat) = rustix::fs::statvfs(&mnt_base) {
            mode = if expected_mode != "unknown" {
                expected_mode
            } else {
                "active".to_string()
            };
            total = stat.f_blocks * stat.f_frsize;
            let free = stat.f_bfree * stat.f_frsize;
            used = total - free;
            if total > 0 {
                percent = (used * 100 / total) as u8;
            }
        }
    }

    let status = StorageStatus {
        mode,
        mount_point: mnt_base.to_string_lossy().to_string(),
        usage_percent: percent,
        total_size: total,
        used_size: used,
        hymofs_available: HymoFs::is_available(),
        hymofs_version: HymoFs::get_version(),
    };

    println!("{}", serde_json::to_string(&status)?);
    Ok(())
}
