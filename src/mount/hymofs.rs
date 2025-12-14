use std::ffi::CString;
use std::fs::File;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::os::fd::AsRawFd;
use std::path::Path;
use anyhow::{Context, Result};
use walkdir::WalkDir;
use crate::defs::HYMO_PROTOCOL_VERSION;

const HYMO_DEV: &str = "/dev/hymo_ctl";
const HYMO_IOC_MAGIC: u8 = 0xE0;

const _IOC_NRBITS: u32 = 8;
const _IOC_TYPEBITS: u32 = 8;
const _IOC_SIZEBITS: u32 = 14;
const _IOC_DIRBITS: u32 = 2;

const _IOC_NRSHIFT: u32 = 0;
const _IOC_TYPESHIFT: u32 = _IOC_NRSHIFT + _IOC_NRBITS;
const _IOC_SIZESHIFT: u32 = _IOC_TYPESHIFT + _IOC_TYPEBITS;
const _IOC_DIRSHIFT: u32 = _IOC_SIZESHIFT + _IOC_SIZEBITS;

const _IOC_NONE: u32 = 0;
const _IOC_WRITE: u32 = 1;
const _IOC_READ: u32 = 2;

const fn _ioc(dir: u32, type_: u32, nr: u32, size: u32) -> u32 {
    (dir << _IOC_DIRSHIFT) | (type_ << _IOC_TYPESHIFT) | (nr << _IOC_NRSHIFT) | (size << _IOC_SIZESHIFT)
}

const fn _io(type_: u32, nr: u32) -> u32 {
    _ioc(_IOC_NONE, type_, nr, 0)
}

const fn _ior(type_: u32, nr: u32, size: usize) -> u32 {
    _ioc(_IOC_READ, type_, nr, size as u32)
}

const fn _iow(type_: u32, nr: u32, size: usize) -> u32 {
    _ioc(_IOC_WRITE, type_, nr, size as u32)
}

const HYMO_IOC_ADD_RULE: u64 = _iow(HYMO_IOC_MAGIC as u32, 1, std::mem::size_of::<HymoIoctlArg>()) as u64;
const HYMO_IOC_DEL_RULE: u64 = _iow(HYMO_IOC_MAGIC as u32, 2, std::mem::size_of::<HymoIoctlArg>()) as u64;
const HYMO_IOC_HIDE_RULE: u64 = _iow(HYMO_IOC_MAGIC as u32, 3, std::mem::size_of::<HymoIoctlArg>()) as u64;
const HYMO_IOC_CLEAR_ALL: u64 = _io(HYMO_IOC_MAGIC as u32, 5) as u64;
const HYMO_IOC_GET_VERSION: u64 = _ior(HYMO_IOC_MAGIC as u32, 6, std::mem::size_of::<i32>()) as u64;
const HYMO_IOC_SET_DEBUG: u64 = _iow(HYMO_IOC_MAGIC as u32, 8, std::mem::size_of::<i32>()) as u64;

#[repr(C)]
struct HymoIoctlArg {
    src: *const libc::c_char,
    target: *const libc::c_char,
    type_: i32,
}

#[derive(Debug, PartialEq)]
pub enum HymoFsStatus {
    Available,
    NotPresent,
    ProtocolMismatch,
    KernelTooOld,
    ModuleTooOld,
}

pub struct HymoFs;

impl HymoFs {
    pub fn is_available() -> bool {
        Path::new(HYMO_DEV).exists()
    }

    pub fn check_status() -> HymoFsStatus {
        if !Self::is_available() {
            return HymoFsStatus::NotPresent;
        }

        match Self::get_version() {
            Some(v) => {
                if v == HYMO_PROTOCOL_VERSION {
                    HymoFsStatus::Available
                } else if v < HYMO_PROTOCOL_VERSION {
                    HymoFsStatus::KernelTooOld
                } else {
                    HymoFsStatus::ModuleTooOld
                }
            },
            None => HymoFsStatus::ProtocolMismatch
        }
    }

    pub fn get_version() -> Option<i32> {
        let file = File::open(HYMO_DEV).ok()?;
        let mut version: i32 = 0;
        let ret = unsafe {
            libc::ioctl(file.as_raw_fd(), HYMO_IOC_GET_VERSION, &mut version)
        };
        if ret == 0 {
            Some(version)
        } else {
            None
        }
    }

    pub fn set_debug(enable: bool) -> Result<()> {
        let file = File::open(HYMO_DEV)?;
        let val: i32 = if enable { 1 } else { 0 };
        let ret = unsafe {
            libc::ioctl(file.as_raw_fd(), HYMO_IOC_SET_DEBUG, &val)
        };
        if ret != 0 {
            anyhow::bail!("Failed to set debug mode, ioctl ret: {}", ret);
        }
        Ok(())
    }

    pub fn clear() -> Result<()> {
        let file = File::open(HYMO_DEV).context("Failed to open HymoFS control device")?;
        let ret = unsafe {
            libc::ioctl(file.as_raw_fd(), HYMO_IOC_CLEAR_ALL)
        };
        if ret != 0 {
            anyhow::bail!("Failed to clear rules, ioctl ret: {}", ret);
        }
        Ok(())
    }

    pub fn add_rule(src: &str, target: &str, type_: i32) -> Result<()> {
        let file = File::open(HYMO_DEV)?;
        let c_src = CString::new(src)?;
        let c_target = CString::new(target)?;

        let arg = HymoIoctlArg {
            src: c_src.as_ptr(),
            target: c_target.as_ptr(),
            type_,
        };

        let ret = unsafe {
            libc::ioctl(file.as_raw_fd(), HYMO_IOC_ADD_RULE, &arg)
        };
        if ret != 0 {
            anyhow::bail!("Failed to add rule, ioctl ret: {}", ret);
        }
        Ok(())
    }

    pub fn hide_path(target: &str) -> Result<()> {
        let file = File::open(HYMO_DEV)?;
        let c_target = CString::new(target)?;

        let arg = HymoIoctlArg {
            src: c_target.as_ptr(),
            target: std::ptr::null(),
            type_: 0,
        };

        let ret = unsafe {
            libc::ioctl(file.as_raw_fd(), HYMO_IOC_HIDE_RULE, &arg)
        };
        if ret != 0 {
            anyhow::bail!("Failed to hide path, ioctl ret: {}", ret);
        }
        Ok(())
    }

    pub fn inject_directory(target_base: &Path, module_dir: &Path) -> Result<()> {
        if !module_dir.exists() || !module_dir.is_dir() {
            return Ok(());
        }

        for entry in WalkDir::new(module_dir).min_depth(1) {
            let entry = entry?;
            let current_path = entry.path();
            
            let rel_path = current_path.strip_prefix(module_dir)?;
            
            let target_path = target_base.join(rel_path);
            let target_str = target_path.to_string_lossy();
            let src_str = current_path.to_string_lossy();

            let metadata = entry.metadata()?;
            let file_type = metadata.file_type();

            if file_type.is_file() || file_type.is_symlink() {
                if let Err(e) = Self::add_rule(&src_str, &target_str, 0) {
                    log::warn!("Failed to add rule for {}: {}", target_str, e);
                }
            } else if file_type.is_char_device() {
                if metadata.rdev() == 0 {
                    if let Err(e) = Self::hide_path(&target_str) {
                        log::warn!("Failed to hide path {}: {}", target_str, e);
                    }
                }
            }
        }
        Ok(())
    }
}
