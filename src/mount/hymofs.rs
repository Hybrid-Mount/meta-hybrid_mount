use std::{
    ffi::{CStr, CString},
    fs::File,
    os::unix::{
        fs::{FileTypeExt, MetadataExt},
        io::AsRawFd,
    },
    path::Path,
};

use anyhow::{Context, Result};
use log::{debug, warn};
use nix::{ioctl_none, ioctl_read, ioctl_readwrite, ioctl_write_ptr};
use serde::Serialize;
use walkdir::WalkDir;

const DEV_PATH: &str = "/dev/hymo_ctl";
const HYMO_IOC_MAGIC: u8 = 0xE0;

#[repr(C)]
pub struct HymoIoctlArg {
    pub src: *const std::ffi::c_char,
    pub target: *const std::ffi::c_char,
    pub type_: std::ffi::c_int,
}

#[repr(C)]
pub struct HymoIoctlListArg {
    pub buf: *mut std::ffi::c_char,
    pub size: usize,
}

ioctl_write_ptr!(ioc_add_rule, HYMO_IOC_MAGIC, 1, HymoIoctlArg);
ioctl_write_ptr!(ioc_del_rule, HYMO_IOC_MAGIC, 2, HymoIoctlArg);
ioctl_write_ptr!(ioc_hide_rule, HYMO_IOC_MAGIC, 3, HymoIoctlArg);
ioctl_none!(ioc_clear_all, HYMO_IOC_MAGIC, 5);
ioctl_read!(ioc_get_version, HYMO_IOC_MAGIC, 6, i32);
ioctl_readwrite!(ioc_list_rules, HYMO_IOC_MAGIC, 7, HymoIoctlListArg);
ioctl_write_ptr!(ioc_set_debug, HYMO_IOC_MAGIC, 8, i32);
ioctl_none!(ioc_reorder_mnt_id, HYMO_IOC_MAGIC, 9);
ioctl_write_ptr!(ioc_set_stealth, HYMO_IOC_MAGIC, 10, i32);
ioctl_write_ptr!(ioc_hide_overlay_xattrs, HYMO_IOC_MAGIC, 11, HymoIoctlArg);

#[derive(Debug, PartialEq)]
#[allow(dead_code)]
pub enum HymoFsStatus {
    Available,
    NotPresent,
    KernelTooOld,
    ModuleTooOld,
}

#[derive(Serialize, Default, Debug)]
pub struct HymoRuleRedirect {
    pub src: String,
    pub target: String,
    pub type_: i32,
}

#[derive(Serialize, Default, Debug)]
pub struct HymoRules {
    pub redirects: Vec<HymoRuleRedirect>,
    pub hides: Vec<String>,
    pub injects: Vec<String>,
    pub xattr_sbs: Vec<String>,
}

#[derive(Serialize, Default, Debug)]
pub struct HymoKernelStatus {
    pub available: bool,
    pub protocol_version: i32,
    pub config_version: i32,
    pub rules: HymoRules,
    pub stealth_active: bool,
    pub debug_active: bool,
}

pub struct HymoFs;

impl HymoFs {
    fn open_dev() -> Result<File> {
        File::open(DEV_PATH).with_context(|| format!("Failed to open {}", DEV_PATH))
    }

    pub fn check_status() -> HymoFsStatus {
        if Path::new(DEV_PATH).exists() {
            HymoFsStatus::Available
        } else {
            HymoFsStatus::NotPresent
        }
    }

    pub fn is_available() -> bool {
        Self::check_status() == HymoFsStatus::Available
    }

    pub fn get_version() -> Option<i32> {
        let file = Self::open_dev().ok()?;
        let mut version: i32 = 0;
        let ret = unsafe { ioc_get_version(file.as_raw_fd(), &mut version) };
        if ret.is_err() { None } else { Some(version) }
    }

    pub fn clear() -> Result<()> {
        debug!("HymoFS: Clearing all rules");
        let file = Self::open_dev()?;
        unsafe { ioc_clear_all(file.as_raw_fd()) }.context("HymoFS clear failed")?;

        let debug_val: i32 = 1;
        unsafe { ioc_set_debug(file.as_raw_fd(), &debug_val) }.ok();

        Ok(())
    }

    pub fn add_rule(src: &str, target: &str, type_val: i32) -> Result<()> {
        debug!(
            "HymoFS: ADD_RULE src='{}' target='{}' type={}",
            src, target, type_val
        );
        let file = Self::open_dev()?;
        let c_src = CString::new(src)?;
        let c_target = CString::new(target)?;

        let arg = HymoIoctlArg {
            src: c_src.as_ptr(),
            target: c_target.as_ptr(),
            type_: type_val as std::ffi::c_int,
        };

        unsafe { ioc_add_rule(file.as_raw_fd(), &arg) }.context("HymoFS add_rule failed")?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn delete_rule(src: &str) -> Result<()> {
        debug!("HymoFS: DEL_RULE src='{}'", src);
        let file = Self::open_dev()?;
        let c_src = CString::new(src)?;

        let arg = HymoIoctlArg {
            src: c_src.as_ptr(),
            target: std::ptr::null(),
            type_: 0,
        };

        unsafe { ioc_del_rule(file.as_raw_fd(), &arg) }.context("HymoFS delete_rule failed")?;
        Ok(())
    }

    pub fn hide_path(path: &str) -> Result<()> {
        debug!("HymoFS: HIDE_RULE path='{}'", path);
        let file = Self::open_dev()?;
        let c_path = CString::new(path)?;

        let arg = HymoIoctlArg {
            src: c_path.as_ptr(),
            target: std::ptr::null(),
            type_: 0,
        };

        unsafe { ioc_hide_rule(file.as_raw_fd(), &arg) }.context("HymoFS hide_path failed")?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn list_active_rules() -> Result<String> {
        let file = Self::open_dev()?;
        let capacity = 32 * 1024;
        let mut buffer = vec![0u8; capacity];
        let mut arg = HymoIoctlListArg {
            buf: buffer.as_mut_ptr() as *mut std::ffi::c_char,
            size: capacity,
        };

        unsafe { ioc_list_rules(file.as_raw_fd(), &mut arg) }
            .context("HymoFS list_rules failed")?;

        let c_str = unsafe { CStr::from_ptr(buffer.as_ptr() as *const std::ffi::c_char) };
        Ok(c_str.to_string_lossy().into_owned())
    }

    pub fn get_kernel_status() -> Result<HymoKernelStatus> {
        if !Self::is_available() {
            return Ok(HymoKernelStatus {
                available: false,
                ..Default::default()
            });
        }

        let mut status = HymoKernelStatus {
            available: true,
            stealth_active: false,
            debug_active: false,
            ..Default::default()
        };

        if let Some(v) = Self::get_version() {
            status.protocol_version = v;
        }

        let raw_info = match Self::list_active_rules() {
            Ok(info) => info,
            Err(e) => {
                warn!("HymoFS list rules failed: {}", e);
                return Ok(status);
            }
        };

        for line in raw_info.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }

            match parts[0] {
                "HymoFS" => {
                    if parts.len() >= 3 && parts[1] == "Protocol:" && status.protocol_version == 0 {
                        status.protocol_version = parts[2].parse().unwrap_or(0);
                    } else if parts[1] == "Config"
                        && parts.get(2) == Some(&"Version:")
                        && let Some(ver_str) = parts.get(3)
                    {
                        status.config_version = ver_str.parse().unwrap_or(0);
                    }
                }
                "add" => {
                    if parts.len() >= 4 {
                        status.rules.redirects.push(HymoRuleRedirect {
                            src: parts[1].to_string(),
                            target: parts[2].to_string(),
                            type_: parts[3].parse().unwrap_or(0),
                        });
                    }
                }
                "hide" => {
                    if parts.len() >= 2 {
                        status.rules.hides.push(parts[1].to_string());
                    }
                }
                "inject" => {
                    if parts.len() >= 2 {
                        status.rules.injects.push(parts[1].to_string());
                    }
                }
                "hide_xattr_sb" => {
                    if parts.len() >= 2 {
                        status.rules.xattr_sbs.push(parts[1].to_string());
                    }
                }
                _ => {}
            }
        }

        Ok(status)
    }

    pub fn inject_directory(target_base: &Path, module_dir: &Path) -> Result<()> {
        if !module_dir.exists() || !module_dir.is_dir() {
            return Ok(());
        }

        debug!(
            "HymoFS: Scanning module dir: {} -> {}",
            module_dir.display(),
            target_base.display()
        );

        let mut pending_ops = Vec::new();

        for entry in WalkDir::new(module_dir).min_depth(1) {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    warn!("HymoFS walk error: {}", e);
                    continue;
                }
            };

            let current_path = entry.path().to_path_buf();
            let relative_path = match current_path.strip_prefix(module_dir) {
                Ok(p) => p,
                Err(_) => continue,
            };
            let target_path = target_base.join(relative_path);
            let file_type = entry.file_type();

            if file_type.is_file() || file_type.is_symlink() {
                pending_ops.push((true, target_path, current_path));
            } else if file_type.is_char_device()
                && let Ok(metadata) = entry.metadata()
                && metadata.rdev() == 0
            {
                pending_ops.push((false, target_path, current_path));
            }
        }

        for (is_add, target_path, current_path) in pending_ops {
            if is_add {
                if let Err(e) = Self::add_rule(
                    &target_path.to_string_lossy(),
                    &current_path.to_string_lossy(),
                    0,
                ) {
                    warn!("Failed to add rule for {}: {}", target_path.display(), e);
                }
            } else if let Err(e) = Self::hide_path(&target_path.to_string_lossy()) {
                warn!("Failed to hide path {}: {}", target_path.display(), e);
            }
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub fn delete_directory_rules(target_base: &Path, module_dir: &Path) -> Result<()> {
        if !module_dir.exists() || !module_dir.is_dir() {
            return Ok(());
        }

        for entry in WalkDir::new(module_dir).min_depth(1) {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    warn!("HymoFS walk error: {}", e);
                    continue;
                }
            };

            let current_path = entry.path();
            let relative_path = match current_path.strip_prefix(module_dir) {
                Ok(p) => p,
                Err(_) => continue,
            };
            let target_path = target_base.join(relative_path);
            let file_type = entry.file_type();

            if file_type.is_file() || file_type.is_symlink() {
                if let Err(e) = Self::delete_rule(&target_path.to_string_lossy()) {
                    warn!("Failed to delete rule for {}: {}", target_path.display(), e);
                }
            } else if file_type.is_char_device()
                && let Ok(metadata) = entry.metadata()
                && metadata.rdev() == 0
                && let Err(e) = Self::delete_rule(&target_path.to_string_lossy())
            {
                warn!(
                    "Failed to delete hidden rule for {}: {}",
                    target_path.display(),
                    e
                );
            }
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub fn set_debug(enable: bool) -> Result<()> {
        let file = Self::open_dev()?;
        let val: i32 = if enable { 1 } else { 0 };
        unsafe { ioc_set_debug(file.as_raw_fd(), &val) }.context("HymoFS set_debug failed")?;
        Ok(())
    }

    pub fn set_stealth(enable: bool) -> Result<()> {
        let file = Self::open_dev()?;
        let val: i32 = if enable { 1 } else { 0 };
        unsafe { ioc_set_stealth(file.as_raw_fd(), &val) }.context("HymoFS set_stealth failed")?;
        Ok(())
    }

    pub fn hide_overlay_xattrs(path: &str) -> Result<()> {
        debug!("HymoFS: HIDE_XATTRS path='{}'", path);
        let file = Self::open_dev()?;
        let c_path = CString::new(path)?;

        let arg = HymoIoctlArg {
            src: c_path.as_ptr(),
            target: std::ptr::null(),
            type_: 0,
        };

        unsafe { ioc_hide_overlay_xattrs(file.as_raw_fd(), &arg) }
            .context("HymoFS hide_overlay_xattrs failed")?;
        Ok(())
    }

    pub fn reorder_mnt_id() -> Result<()> {
        let file = Self::open_dev()?;
        unsafe { ioc_reorder_mnt_id(file.as_raw_fd()) }.context("HymoFS reorder_mnt_id failed")?;
        Ok(())
    }
}
