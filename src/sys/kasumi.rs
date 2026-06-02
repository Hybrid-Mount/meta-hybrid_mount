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
    ffi::{CString, c_char, c_int, c_ulong, c_void},
    fs,
    os::{
        fd::BorrowedFd,
        unix::{
            ffi::OsStrExt,
            fs::{FileTypeExt, MetadataExt},
        },
    },
    path::Path,
    sync::{LazyLock, Mutex},
};
#[cfg(any(target_os = "linux", target_os = "android"))]
use std::{thread, time::Duration};

use anyhow::{Context, Result, bail};
use rustix::{
    io::Errno,
    ioctl::{self, Ioctl, IoctlOutput, Opcode},
};
use walkdir::WalkDir;

#[allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    dead_code
)]
mod uapi {
    include!(concat!(env!("OUT_DIR"), "/kasumi_uapi.rs"));
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub const KSM_MAGIC1: c_int = uapi::KSM_MAGIC1 as c_int;
#[cfg(any(target_os = "linux", target_os = "android"))]
pub const KSM_MAGIC2: c_int = uapi::KSM_MAGIC2 as c_int;
pub const KSM_PROTOCOL_VERSION: c_int = uapi::KSM_PROTOCOL_VERSION as c_int;

#[cfg(any(target_os = "linux", target_os = "android"))]
pub const KSM_SYSCALL_NR: libc::c_long = uapi::KSM_SYSCALL_NR as libc::c_long;
#[cfg(any(target_os = "linux", target_os = "android"))]
pub const KSM_CMD_GET_FD: c_int = uapi::KSM_CMD_GET_FD as c_int;
#[cfg(any(target_os = "linux", target_os = "android"))]
pub const KSM_PRCTL_GET_FD: c_int = uapi::KSM_PRCTL_GET_FD as c_int;

pub const KSM_FEATURE_KSTAT_SPOOF: c_int = uapi::KSM_FEATURE_KSTAT_SPOOF as c_int;
pub const KSM_FEATURE_UNAME_SPOOF: c_int = uapi::KSM_FEATURE_UNAME_SPOOF as c_int;
pub const KSM_FEATURE_CMDLINE_SPOOF: c_int = uapi::KSM_FEATURE_CMDLINE_SPOOF as c_int;
pub const KSM_FEATURE_SELINUX_BYPASS: c_int = uapi::KSM_FEATURE_SELINUX_BYPASS as c_int;
pub const KSM_FEATURE_MERGE_DIR: c_int = uapi::KSM_FEATURE_MERGE_DIR as c_int;
pub const KSM_FEATURE_MOUNT_HIDE: c_int = uapi::KSM_FEATURE_MOUNT_HIDE as c_int;
pub const KSM_FEATURE_MAPS_SPOOF: c_int = uapi::KSM_FEATURE_MAPS_SPOOF as c_int;
pub const KSM_FEATURE_STATFS_SPOOF: c_int = uapi::KSM_FEATURE_STATFS_SPOOF as c_int;
pub const KSM_FEATURE_FAKE_MOUNTINFO: c_int = uapi::KSM_FEATURE_FAKE_MOUNTINFO as c_int;
pub const KSM_FEATURE_SELINUX_FIX: c_int = uapi::KSM_FEATURE_SELINUX_FIX as c_int;

#[allow(clippy::unnecessary_cast)]
const KSM_IOC_MAGIC: u8 = uapi::KSM_IOC_MAGIC as u8;

type KasumiIoctlRequest = Opcode;

pub type KasumiSyscallArg = uapi::kasumi_syscall_arg;

impl uapi::kasumi_syscall_arg {
    fn new(src: &CString, target: Option<&CString>, type_: c_int) -> Self {
        Self {
            src: src.as_ptr(),
            target: target.map_or(std::ptr::null(), |value| value.as_ptr()),
            type_,
        }
    }
}

pub type KasumiSyscallListArg = uapi::kasumi_syscall_list_arg;

/// Zero-initialized `Default` for `#[repr(C)]` bindgen ioctl structs.
///
/// # Safety
///
/// Only used with C-compatible structs whose fields are exclusively integers
/// and `c_char` arrays. The all-zeroes bit pattern is a valid representation
/// for every field of these types.
macro_rules! impl_zeroed_default {
    ($t:ty) => {
        impl Default for $t {
            fn default() -> Self {
                unsafe { std::mem::MaybeUninit::zeroed().assume_init() }
            }
        }
    };
}

macro_rules! uname_setter {
    ($method:ident, $field:ident, $label:literal) => {
        pub fn $method(&mut self, value: &str) -> Result<()> {
            write_str_into_c_buf(&mut self.$field, value, concat!("Kasumi uname ", $label))
        }
    };
}

pub type KasumiUidListArg = uapi::kasumi_uid_list_arg;

impl uapi::kasumi_uid_list_arg {
    pub fn from_slice(uids: &[u32]) -> Self {
        Self {
            count: uids.len() as u32,
            reserved: 0,
            uids: if uids.is_empty() {
                0
            } else {
                uids.as_ptr() as usize as u64
            },
        }
    }
}

pub type KasumiSpoofKstat = uapi::kasumi_spoof_kstat;

impl_zeroed_default!(uapi::kasumi_spoof_kstat);

impl uapi::kasumi_spoof_kstat {
    pub fn new(target_ino: c_ulong, target_pathname: impl AsRef<Path>) -> Result<Self> {
        let mut value = Self {
            target_ino,
            ..Self::default()
        };
        value.set_target_pathname(target_pathname)?;
        Ok(value)
    }

    pub fn set_target_pathname(&mut self, target_pathname: impl AsRef<Path>) -> Result<()> {
        write_path_into_c_buf(
            &mut self.target_pathname,
            target_pathname.as_ref(),
            "Kasumi kstat target pathname",
        )
    }
}

pub type KasumiSpoofUname = uapi::kasumi_spoof_uname;

impl_zeroed_default!(uapi::kasumi_spoof_uname);

impl uapi::kasumi_spoof_uname {
    uname_setter!(set_sysname, sysname, "sysname");
    uname_setter!(set_nodename, nodename, "nodename");
    uname_setter!(set_release, release, "release");
    uname_setter!(set_version, version, "version");
    uname_setter!(set_machine, machine, "machine");
    uname_setter!(set_domainname, domainname, "domainname");
}

pub type KasumiSpoofCmdline = uapi::kasumi_spoof_cmdline;

impl_zeroed_default!(uapi::kasumi_spoof_cmdline);

impl uapi::kasumi_spoof_cmdline {
    pub fn new(cmdline: &str) -> Result<Self> {
        let mut value = Self::default();
        value.set_cmdline(cmdline)?;
        Ok(value)
    }

    pub fn set_cmdline(&mut self, cmdline: &str) -> Result<()> {
        write_str_into_c_buf(&mut self.cmdline, cmdline, "Kasumi cmdline")
    }
}

pub type KasumiMapsRule = uapi::kasumi_maps_rule;

impl_zeroed_default!(uapi::kasumi_maps_rule);

impl uapi::kasumi_maps_rule {
    pub fn new(
        target_ino: c_ulong,
        target_dev: c_ulong,
        spoofed_ino: c_ulong,
        spoofed_dev: c_ulong,
        spoofed_pathname: impl AsRef<Path>,
    ) -> Result<Self> {
        let mut value = Self {
            target_ino,
            target_dev,
            spoofed_ino,
            spoofed_dev,
            ..Self::default()
        };
        value.set_spoofed_pathname(spoofed_pathname)?;
        Ok(value)
    }

    pub fn set_spoofed_pathname(&mut self, spoofed_pathname: impl AsRef<Path>) -> Result<()> {
        write_path_into_c_buf(
            &mut self.spoofed_pathname,
            spoofed_pathname.as_ref(),
            "Kasumi maps spoofed pathname",
        )
    }
}

pub type KasumiMountHideArg = uapi::kasumi_mount_hide_arg;

impl_zeroed_default!(uapi::kasumi_mount_hide_arg);

impl uapi::kasumi_mount_hide_arg {
    pub fn new(enable: bool, path_pattern: Option<&Path>) -> Result<Self> {
        let mut value = Self {
            enable: if enable { 1 } else { 0 },
            ..Self::default()
        };
        if let Some(path_pattern) = path_pattern {
            value.set_path_pattern(path_pattern)?;
        }
        Ok(value)
    }

    pub fn set_path_pattern(&mut self, path_pattern: impl AsRef<Path>) -> Result<()> {
        write_path_into_c_buf(
            &mut self.path_pattern,
            path_pattern.as_ref(),
            "Kasumi mount_hide path_pattern",
        )
    }
}

pub type KasumiMapsSpoofArg = uapi::kasumi_maps_spoof_arg;

impl_zeroed_default!(uapi::kasumi_maps_spoof_arg);

impl uapi::kasumi_maps_spoof_arg {
    pub fn new(enable: bool) -> Self {
        Self {
            enable: if enable { 1 } else { 0 },
            ..Self::default()
        }
    }
}

pub type KasumiStatfsSpoofArg = uapi::kasumi_statfs_spoof_arg;

impl_zeroed_default!(uapi::kasumi_statfs_spoof_arg);

impl uapi::kasumi_statfs_spoof_arg {
    pub fn new(enable: bool) -> Self {
        Self {
            enable: if enable { 1 } else { 0 },
            ..Self::default()
        }
    }

    pub fn with_path_and_f_type(
        enable: bool,
        path: impl AsRef<Path>,
        spoof_f_type: c_ulong,
    ) -> Result<Self> {
        let mut value = Self::new(enable);
        value.set_path(path)?;
        value.set_spoof_f_type(spoof_f_type);
        Ok(value)
    }

    pub fn set_path(&mut self, path: impl AsRef<Path>) -> Result<()> {
        write_path_into_c_buf(&mut self.path, path.as_ref(), "Kasumi statfs path")
    }

    pub fn set_spoof_f_type(&mut self, spoof_f_type: c_ulong) {
        self.spoof_f_type = spoof_f_type;
    }
}

pub const KSM_IOC_ADD_RULE: KasumiIoctlRequest =
    ioctl::opcode::write::<KasumiSyscallArg>(KSM_IOC_MAGIC, 1);
pub const KSM_IOC_DEL_RULE: KasumiIoctlRequest =
    ioctl::opcode::write::<KasumiSyscallArg>(KSM_IOC_MAGIC, 2);
pub const KSM_IOC_HIDE_RULE: KasumiIoctlRequest =
    ioctl::opcode::write::<KasumiSyscallArg>(KSM_IOC_MAGIC, 3);
pub const KSM_IOC_CLEAR_ALL: KasumiIoctlRequest = ioctl::opcode::none(KSM_IOC_MAGIC, 5);
pub const KSM_IOC_GET_VERSION: KasumiIoctlRequest = ioctl::opcode::read::<c_int>(KSM_IOC_MAGIC, 6);
pub const KSM_IOC_LIST_RULES: KasumiIoctlRequest =
    ioctl::opcode::read_write::<KasumiSyscallListArg>(KSM_IOC_MAGIC, 7);
pub const KSM_IOC_SET_DEBUG: KasumiIoctlRequest = ioctl::opcode::write::<c_int>(KSM_IOC_MAGIC, 8);
pub const KSM_IOC_REORDER_MNT_ID: KasumiIoctlRequest = ioctl::opcode::none(KSM_IOC_MAGIC, 9);
pub const KSM_IOC_SET_STEALTH: KasumiIoctlRequest =
    ioctl::opcode::write::<c_int>(KSM_IOC_MAGIC, 10);
pub const KSM_IOC_HIDE_OVERLAY_XATTRS: KasumiIoctlRequest =
    ioctl::opcode::write::<KasumiSyscallArg>(KSM_IOC_MAGIC, 11);
pub const KSM_IOC_MERGE_RULE: KasumiIoctlRequest =
    ioctl::opcode::write::<KasumiSyscallArg>(KSM_IOC_MAGIC, 12);
pub const KSM_IOC_ADD_MERGE_RULE: KasumiIoctlRequest = KSM_IOC_MERGE_RULE;
pub const KSM_IOC_SET_MIRROR_PATH: KasumiIoctlRequest =
    ioctl::opcode::write::<KasumiSyscallArg>(KSM_IOC_MAGIC, 14);
pub const KSM_IOC_ADD_SPOOF_KSTAT: KasumiIoctlRequest =
    ioctl::opcode::write::<KasumiSpoofKstat>(KSM_IOC_MAGIC, 15);
pub const KSM_IOC_UPDATE_SPOOF_KSTAT: KasumiIoctlRequest =
    ioctl::opcode::write::<KasumiSpoofKstat>(KSM_IOC_MAGIC, 16);
pub const KSM_IOC_SET_UNAME: KasumiIoctlRequest =
    ioctl::opcode::write::<KasumiSpoofUname>(KSM_IOC_MAGIC, 17);
pub const KSM_IOC_SET_CMDLINE: KasumiIoctlRequest =
    ioctl::opcode::write::<KasumiSpoofCmdline>(KSM_IOC_MAGIC, 18);
pub const KSM_IOC_GET_FEATURES: KasumiIoctlRequest =
    ioctl::opcode::read::<c_int>(KSM_IOC_MAGIC, 19);
pub const KSM_IOC_SET_ENABLED: KasumiIoctlRequest =
    ioctl::opcode::write::<c_int>(KSM_IOC_MAGIC, 20);
pub const KSM_IOC_SET_HIDE_UIDS: KasumiIoctlRequest =
    ioctl::opcode::write::<KasumiUidListArg>(KSM_IOC_MAGIC, 21);
pub const KSM_IOC_GET_HOOKS: KasumiIoctlRequest =
    ioctl::opcode::read_write::<KasumiSyscallListArg>(KSM_IOC_MAGIC, 22);
pub const KSM_IOC_ADD_MAPS_RULE: KasumiIoctlRequest =
    ioctl::opcode::write::<KasumiMapsRule>(KSM_IOC_MAGIC, 23);
pub const KSM_IOC_CLEAR_MAPS_RULES: KasumiIoctlRequest = ioctl::opcode::none(KSM_IOC_MAGIC, 24);
pub const KSM_IOC_SET_MOUNT_HIDE: KasumiIoctlRequest =
    ioctl::opcode::write::<KasumiMountHideArg>(KSM_IOC_MAGIC, 25);
pub const KSM_IOC_SET_MAPS_SPOOF: KasumiIoctlRequest =
    ioctl::opcode::write::<KasumiMapsSpoofArg>(KSM_IOC_MAGIC, 26);
pub const KSM_IOC_SET_STATFS_SPOOF: KasumiIoctlRequest =
    ioctl::opcode::write::<KasumiStatfsSpoofArg>(KSM_IOC_MAGIC, 27);
pub const KSM_IOC_SET_UNAME_GLOBAL: KasumiIoctlRequest =
    ioctl::opcode::write::<KasumiSpoofUname>(KSM_IOC_MAGIC, 28);
pub const KSM_IOC_SELINUX_FIX: KasumiIoctlRequest =
    ioctl::opcode::write::<c_int>(KSM_IOC_MAGIC, 29);

struct KasumiIoctlNoArg {
    request: KasumiIoctlRequest,
}

impl KasumiIoctlNoArg {
    const fn new(request: KasumiIoctlRequest) -> Self {
        Self { request }
    }
}

unsafe impl Ioctl for KasumiIoctlNoArg {
    type Output = ();

    const IS_MUTATING: bool = false;

    fn opcode(&self) -> Opcode {
        self.request
    }

    fn as_ptr(&mut self) -> *mut c_void {
        std::ptr::null_mut()
    }

    unsafe fn output_from_ptr(_: IoctlOutput, _: *mut c_void) -> rustix::io::Result<Self::Output> {
        Ok(())
    }
}

struct KasumiIoctlArg<'a, T> {
    request: KasumiIoctlRequest,
    arg: &'a mut T,
}

impl<'a, T> KasumiIoctlArg<'a, T> {
    fn new(request: KasumiIoctlRequest, arg: &'a mut T) -> Self {
        Self { request, arg }
    }
}

unsafe impl<T> Ioctl for KasumiIoctlArg<'_, T> {
    type Output = ();

    const IS_MUTATING: bool = true;

    fn opcode(&self) -> Opcode {
        self.request
    }

    fn as_ptr(&mut self) -> *mut c_void {
        (self.arg as *mut T).cast()
    }

    unsafe fn output_from_ptr(_: IoctlOutput, _: *mut c_void) -> rustix::io::Result<Self::Output> {
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KasumiStatus {
    Available,
    #[default]
    NotPresent,
    KernelNotSupported,
    ModuleTooOld,
}

pub fn status_name(status: KasumiStatus) -> &'static str {
    match status {
        KasumiStatus::Available => "available",
        KasumiStatus::NotPresent => "not_present",
        KasumiStatus::KernelNotSupported => "kernel_not_supported",
        KasumiStatus::ModuleTooOld => "module_too_old",
    }
}

const FEATURE_NAMES: &[(c_int, &str)] = &[
    (KSM_FEATURE_KSTAT_SPOOF, "kstat_spoof"),
    (KSM_FEATURE_UNAME_SPOOF, "uname_spoof"),
    (KSM_FEATURE_CMDLINE_SPOOF, "cmdline_spoof"),
    (KSM_FEATURE_SELINUX_BYPASS, "selinux_bypass"),
    (KSM_FEATURE_MERGE_DIR, "merge_dir"),
    (KSM_FEATURE_MOUNT_HIDE, "mount_hide"),
    (KSM_FEATURE_MAPS_SPOOF, "maps_spoof"),
    (KSM_FEATURE_STATFS_SPOOF, "statfs_spoof"),
    (KSM_FEATURE_FAKE_MOUNTINFO, "fake_mountinfo"),
    (KSM_FEATURE_SELINUX_FIX, "selinux_fix"),
];

pub fn feature_names(bits: c_int) -> Vec<String> {
    FEATURE_NAMES
        .iter()
        .filter(|(bit, _)| bits & *bit != 0)
        .map(|(_, name)| (*name).to_string())
        .collect()
}

#[derive(Debug, Default)]
struct StatusCache {
    checked: bool,
    status: KasumiStatus,
}

static STATUS_CACHE: LazyLock<Mutex<StatusCache>> =
    LazyLock::new(|| Mutex::new(StatusCache::default()));
static FD_CACHE: LazyLock<Mutex<Option<c_int>>> = LazyLock::new(|| Mutex::new(None));

fn cstring_from_path(path: &Path) -> Result<CString> {
    CString::new(path.as_os_str().as_bytes())
        .with_context(|| format!("path contains interior NUL byte: {}", path.display()))
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn lock_error(name: &str) -> anyhow::Error {
    anyhow::anyhow!("failed to lock Kasumi {name} mutex")
}

fn write_bytes_into_c_buf(buf: &mut [c_char], bytes: &[u8], field_name: &str) -> Result<()> {
    if bytes.len() >= buf.len() {
        bail!("{field_name} exceeds {} bytes", buf.len() - 1);
    }

    buf.fill(0);
    for (dst, src) in buf.iter_mut().zip(bytes.iter().copied()) {
        *dst = src as c_char;
    }

    Ok(())
}

fn write_str_into_c_buf(buf: &mut [c_char], value: &str, field_name: &str) -> Result<()> {
    write_bytes_into_c_buf(buf, value.as_bytes(), field_name)
}

fn write_path_into_c_buf(buf: &mut [c_char], path: &Path, field_name: &str) -> Result<()> {
    write_bytes_into_c_buf(buf, path.as_os_str().as_bytes(), field_name)
}

fn module_loaded() -> bool {
    let Ok(content) = fs::read_to_string("/proc/modules") else {
        return false;
    };

    content.lines().any(|line| {
        line.starts_with("kasumi_lkm ")
            || line.starts_with("kasumi_lkm\t")
            || line.starts_with("kasumi ")
            || line.starts_with("kasumi\t")
    })
}

/// Returns `true` when the running kernel version matches one of the supported
/// versions for which a prebuilt Kasumi LKM is available.
pub fn kernel_is_supported() -> bool {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        const SUPPORTED: &[&str] = &["5.10", "5.15", "6.1", "6.6", "6.12"];
        let uts = unsafe {
            let mut uts = std::mem::MaybeUninit::<libc::utsname>::uninit();
            if libc::uname(uts.as_mut_ptr()) != 0 {
                return false;
            }
            uts.assume_init()
        };
        let release = unsafe { std::ffi::CStr::from_ptr(uts.release.as_ptr()) }
            .to_string_lossy()
            .into_owned();

        // Extract "major.minor" prefix, e.g. "5.10" from "5.10.123-something".
        let version_prefix = match release.find('.') {
            Some(dot1) => {
                let rest = &release[dot1 + 1..];
                let dot2 = rest
                    .find(|c: char| !c.is_ascii_digit())
                    .unwrap_or(rest.len());
                release[..dot1 + 1 + dot2].to_string()
            }
            None => String::new(),
        };

        SUPPORTED.contains(&version_prefix.as_str())
    }

    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        // Non-Linux hosts (macOS dev, etc.) — don't gate.
        true
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn fetch_anon_fd() -> Result<c_int> {
    {
        let cache = FD_CACHE.lock().map_err(|_| lock_error("fd"))?;
        if let Some(fd) = *cache {
            crate::scoped_log!(debug, "kasumi:fd", "complete: source=cache, fd={}", fd);
            return Ok(fd);
        }
    }

    crate::scoped_log!(debug, "kasumi:fd", "start: source=kernel_query");

    // Bail immediately when Kasumi LKM isn't loaded — avoids a ~4.6 s
    // retry loop that can never succeed on unsupported kernels.
    // Use module_loaded() instead of check_status() to avoid recursion:
    // check_status() → get_protocol_version() → ioctl_call() → fetch_anon_fd() → check_status()
    if !module_loaded() {
        bail!("Kasumi LKM is not loaded");
    }

    let mut fd = -1;
    const WAIT_ATTEMPTS: usize = 4;
    const SHORT_RETRIES: usize = 2;

    for wait_round in 0..WAIT_ATTEMPTS {
        if wait_round > 0 {
            thread::sleep(Duration::from_secs(1));
        }

        unsafe {
            libc::prctl(
                KSM_PRCTL_GET_FD,
                &mut fd as *mut c_int as libc::c_ulong,
                0,
                0,
                0,
            );
        }

        if fd >= 0 {
            crate::scoped_log!(
                debug,
                "kasumi:fd",
                "complete: source=prctl, round={}",
                wait_round
            );
            break;
        }

        for retry in 0..SHORT_RETRIES {
            if retry > 0 {
                thread::sleep(Duration::from_millis(80));
            }
            unsafe {
                libc::syscall(
                    KSM_SYSCALL_NR,
                    KSM_MAGIC1 as libc::c_long,
                    KSM_MAGIC2 as libc::c_long,
                    KSM_CMD_GET_FD as libc::c_long,
                    &mut fd as *mut c_int,
                );
            }

            if fd >= 0 {
                crate::scoped_log!(
                    debug,
                    "kasumi:fd",
                    "complete: source=syscall, round={}, retry={}",
                    wait_round,
                    retry
                );
                break;
            }
        }

        if fd >= 0 {
            break;
        }
    }

    if fd < 0 {
        crate::scoped_log!(
            warn,
            "kasumi:fd",
            "failed: reason=obtain_fd_failed, attempts={}, short_retries={}",
            WAIT_ATTEMPTS,
            SHORT_RETRIES
        );
        bail!("failed to obtain Kasumi anonymous fd");
    }

    let mut cache = FD_CACHE.lock().map_err(|_| lock_error("fd"))?;
    *cache = Some(fd);
    Ok(fd)
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn fetch_anon_fd() -> Result<c_int> {
    bail!("Kasumi is only supported on linux/android")
}

fn ioctl_error_context(name: &str, request: KasumiIoctlRequest, err: Errno) -> String {
    let hint = match err.raw_os_error() {
        libc::EINVAL => "invalid payload or protocol mismatch",
        libc::EOPNOTSUPP | libc::ENOTTY => "unsupported by the current kernel/module build",
        _ => "kernel call failed",
    };

    format!(
        "Kasumi ioctl failed: name={name}, opcode=0x{:x}, errno={} ({hint})",
        request,
        err.raw_os_error()
    )
}

fn ioctl_call(
    name: &str,
    request: KasumiIoctlRequest,
    has_arg: bool,
    run: impl FnOnce(BorrowedFd<'_>) -> rustix::io::Result<()>,
) -> Result<()> {
    crate::scoped_log!(
        debug,
        "kasumi:ioctl",
        "start: name={}, opcode=0x{:x}, has_arg={}",
        name,
        request,
        has_arg
    );
    let fd = unsafe { BorrowedFd::borrow_raw(fetch_anon_fd()?) };
    match run(fd) {
        Ok(()) => {
            crate::scoped_log!(
                debug,
                "kasumi:ioctl",
                "complete: name={}, opcode=0x{:x}",
                name,
                request
            );
            Ok(())
        }
        Err(err) => {
            let context = ioctl_error_context(name, request, err);
            crate::scoped_log!(
                error,
                "kasumi:ioctl",
                "failed: name={}, opcode=0x{:x}, errno={}",
                name,
                request,
                err.raw_os_error()
            );
            Err(anyhow::Error::new(err).context(context))
        }
    }
}

fn ioctl_noarg(name: &str, request: KasumiIoctlRequest) -> Result<()> {
    ioctl_call(name, request, false, |fd| unsafe {
        ioctl::ioctl(fd, KasumiIoctlNoArg::new(request))
    })
}

fn ioctl_with_arg<T>(name: &str, request: KasumiIoctlRequest, arg: &mut T) -> Result<()> {
    ioctl_call(name, request, true, |fd| unsafe {
        ioctl::ioctl(fd, KasumiIoctlArg::new(request, arg))
    })
}

fn ioctl_with_bool(name: &str, request: KasumiIoctlRequest, value: bool) -> Result<()> {
    let mut raw: c_int = if value { 1 } else { 0 };
    ioctl_with_arg(name, request, &mut raw)
}

fn single_path_ioctl(name: &str, request: KasumiIoctlRequest, path: &Path) -> Result<()> {
    let src = cstring_from_path(path)?;
    let mut arg = KasumiSyscallArg::new(&src, None, 0);
    ioctl_with_arg(name, request, &mut arg)
}

fn ensure_kernel_err(context: &str, kernel_err: c_int) -> Result<()> {
    if kernel_err != 0 {
        bail!("{context} kernel err={kernel_err}");
    }
    Ok(())
}

fn list_ioctl(request: KasumiIoctlRequest, capacity: usize, description: &str) -> Result<String> {
    crate::scoped_log!(
        debug,
        "kasumi:list_ioctl",
        "start: description={}, opcode=0x{:x}, capacity={}",
        description,
        request,
        capacity
    );
    let mut buf = vec![0u8; capacity];
    let mut arg = KasumiSyscallListArg {
        buf: buf.as_mut_ptr() as *mut c_char,
        size: buf.len(),
    };
    ioctl_with_arg(description, request, &mut arg)
        .with_context(|| format!("failed to query Kasumi {description}"))?;

    let len = buf.iter().position(|byte| *byte == 0).unwrap_or_else(|| {
        crate::scoped_log!(
            warn,
            "kasumi:list_ioctl",
            "truncated: description={}, capacity={} (no NUL terminator found)",
            description,
            capacity
        );
        buf.len()
    });
    let output = String::from_utf8_lossy(&buf[..len]).into_owned();
    crate::scoped_log!(
        debug,
        "kasumi:list_ioctl",
        "complete: description={}, bytes={}, capacity={}",
        description,
        len,
        capacity
    );
    Ok(output)
}

pub fn get_protocol_version() -> Result<c_int> {
    let mut version = 0;
    ioctl_with_arg("get_version", KSM_IOC_GET_VERSION, &mut version)?;
    Ok(version)
}

pub fn check_status() -> KasumiStatus {
    if let Ok(cache) = STATUS_CACHE.lock()
        && cache.checked
    {
        crate::scoped_log!(
            debug,
            "kasumi:status",
            "complete: source=cache, status={}",
            status_name(cache.status)
        );
        return cache.status;
    }

    let status = if !kernel_is_supported() {
        crate::scoped_log!(
            debug,
            "kasumi:status",
            "kernel version not in supported list — forcing KernelNotSupported"
        );
        KasumiStatus::KernelNotSupported
    } else if !module_loaded() {
        KasumiStatus::NotPresent
    } else {
        match get_protocol_version() {
            Ok(version) if version < KSM_PROTOCOL_VERSION => KasumiStatus::KernelNotSupported,
            Ok(version) if version > KSM_PROTOCOL_VERSION => KasumiStatus::ModuleTooOld,
            Ok(_) => KasumiStatus::Available,
            Err(_) => KasumiStatus::NotPresent,
        }
    };

    if let Ok(mut cache) = STATUS_CACHE.lock() {
        cache.checked = true;
        cache.status = status;
    }

    crate::scoped_log!(
        debug,
        "kasumi:status",
        "complete: source=probe, status={}",
        status_name(status)
    );

    status
}

pub fn can_operate() -> bool {
    let operable = matches!(check_status(), KasumiStatus::Available);
    crate::scoped_log!(debug, "kasumi:status", "complete: can_operate={}", operable);
    operable
}

pub fn clear_rules() -> Result<()> {
    ioctl_noarg("clear_rules", KSM_IOC_CLEAR_ALL)
}

pub fn add_rule(virtual_path: &Path, backing_path: &Path, file_type: c_int) -> Result<()> {
    let src = cstring_from_path(virtual_path)?;
    let target = cstring_from_path(backing_path)?;
    let mut arg = KasumiSyscallArg::new(&src, Some(&target), file_type);
    ioctl_with_arg("add_rule", KSM_IOC_ADD_RULE, &mut arg)
}

pub fn add_merge_rule(virtual_path: &Path, backing_path: &Path) -> Result<()> {
    let src = cstring_from_path(virtual_path)?;
    let target = cstring_from_path(backing_path)?;
    let mut arg = KasumiSyscallArg::new(&src, Some(&target), 0);
    ioctl_with_arg("add_merge_rule", KSM_IOC_ADD_MERGE_RULE, &mut arg)
}

pub fn delete_rule(virtual_path: &Path) -> Result<()> {
    single_path_ioctl("delete_rule", KSM_IOC_DEL_RULE, virtual_path)
}

pub fn hide_path(virtual_path: &Path) -> Result<()> {
    single_path_ioctl("hide_path", KSM_IOC_HIDE_RULE, virtual_path)
}

fn helper_rule_dtype(path: &Path) -> Result<Option<c_int>> {
    let metadata = fs::symlink_metadata(path).with_context(|| {
        format!(
            "failed to read Kasumi helper metadata for {}",
            path.display()
        )
    })?;
    let file_type = metadata.file_type();

    if file_type.is_file() {
        Ok(Some(libc::DT_REG as c_int))
    } else if file_type.is_symlink() {
        Ok(Some(libc::DT_LNK as c_int))
    } else if file_type.is_char_device() && metadata.rdev() == 0 {
        Ok(None)
    } else {
        bail!(
            "unsupported helper entry type for {} (expected regular file, symlink, or whiteout)",
            path.display()
        );
    }
}

pub fn list_rules() -> Result<String> {
    list_ioctl(KSM_IOC_LIST_RULES, 16 * 1024, "rule list")
}

fn for_each_helper_rule(
    target_base: &Path,
    module_dir: &Path,
    mut handle: impl FnMut(&Path, &Path, Option<c_int>) -> Result<()>,
) -> Result<()> {
    if !module_dir.exists() || !module_dir.is_dir() {
        bail!(
            "Kasumi helper source is not a directory: {}",
            module_dir.display()
        );
    }

    for entry_result in WalkDir::new(module_dir).follow_links(false) {
        let entry = entry_result.with_context(|| {
            format!(
                "failed to walk Kasumi helper directory {}",
                module_dir.display()
            )
        })?;

        if entry.depth() == 0 || entry.file_type().is_dir() {
            continue;
        }

        let path = entry.path();
        let relative = path.strip_prefix(module_dir).with_context(|| {
            format!(
                "failed to compute relative path for Kasumi helper entry {}",
                path.display()
            )
        })?;
        let target_path = target_base.join(relative);

        handle(&target_path, path, helper_rule_dtype(path)?)?;
    }

    Ok(())
}

pub fn add_rules_from_directory(target_base: &Path, module_dir: &Path) -> Result<()> {
    for_each_helper_rule(
        target_base,
        module_dir,
        |target_path, path, dtype| match dtype {
            Some(file_type) => add_rule(target_path, path, file_type),
            None => hide_path(target_path),
        },
    )
}

pub fn remove_rules_from_directory(target_base: &Path, module_dir: &Path) -> Result<()> {
    for_each_helper_rule(target_base, module_dir, |target_path, _, _| {
        delete_rule(target_path)
    })
}

pub fn set_mirror_path(path: &Path) -> Result<()> {
    single_path_ioctl("set_mirror_path", KSM_IOC_SET_MIRROR_PATH, path)
}

pub fn set_debug(enable: bool) -> Result<()> {
    ioctl_with_bool("set_debug", KSM_IOC_SET_DEBUG, enable)
}

pub fn set_stealth(enable: bool) -> Result<()> {
    ioctl_with_bool("set_stealth", KSM_IOC_SET_STEALTH, enable)
}

pub fn set_enabled(enable: bool) -> Result<()> {
    ioctl_with_bool("set_enabled", KSM_IOC_SET_ENABLED, enable)
}

pub fn add_spoof_kstat(rule: &KasumiSpoofKstat) -> Result<()> {
    let mut rule = *rule;
    ioctl_with_arg("add_spoof_kstat", KSM_IOC_ADD_SPOOF_KSTAT, &mut rule)?;
    ensure_kernel_err("Kasumi add_spoof_kstat", rule.err)
}

pub fn update_spoof_kstat(rule: &KasumiSpoofKstat) -> Result<()> {
    let mut rule = *rule;
    ioctl_with_arg("update_spoof_kstat", KSM_IOC_UPDATE_SPOOF_KSTAT, &mut rule)?;
    ensure_kernel_err("Kasumi update_spoof_kstat", rule.err)
}

pub fn set_uname(uname: &KasumiSpoofUname) -> Result<()> {
    let mut uname = *uname;
    ioctl_with_arg("set_uname", KSM_IOC_SET_UNAME, &mut uname)?;
    ensure_kernel_err("Kasumi set_uname", uname.err)
}

pub fn set_uname_global(uname: &KasumiSpoofUname) -> Result<()> {
    let mut uname = *uname;
    ioctl_with_arg("set_uname_global", KSM_IOC_SET_UNAME_GLOBAL, &mut uname)?;
    ensure_kernel_err("Kasumi set_uname_global", uname.err)
}

pub fn restore_uname_global() -> Result<()> {
    let uname = KasumiSpoofUname::default();
    set_uname_global(&uname)
}

pub fn set_cmdline(cmdline: &KasumiSpoofCmdline) -> Result<()> {
    let mut cmdline = *cmdline;
    ioctl_with_arg("set_cmdline", KSM_IOC_SET_CMDLINE, &mut cmdline)?;
    ensure_kernel_err("Kasumi set_cmdline", cmdline.err)
}

pub fn set_cmdline_str(cmdline: &str) -> Result<()> {
    let cmdline = KasumiSpoofCmdline::new(cmdline)?;
    set_cmdline(&cmdline)
}

pub fn set_hide_uids(uids: &[u32]) -> Result<()> {
    let mut arg = KasumiUidListArg::from_slice(uids);
    ioctl_with_arg("set_hide_uids", KSM_IOC_SET_HIDE_UIDS, &mut arg)
}

pub fn fix_mounts() -> Result<()> {
    ioctl_noarg("fix_mounts", KSM_IOC_REORDER_MNT_ID)
}

pub fn hide_overlay_xattrs(path: &Path) -> Result<()> {
    single_path_ioctl("hide_overlay_xattrs", KSM_IOC_HIDE_OVERLAY_XATTRS, path)
}

pub fn get_features() -> Result<c_int> {
    let mut features = 0;
    ioctl_with_arg("get_features", KSM_IOC_GET_FEATURES, &mut features)?;
    Ok(features)
}

pub fn get_hooks() -> Result<String> {
    list_ioctl(KSM_IOC_GET_HOOKS, 4 * 1024, "hook list")
}

pub fn add_maps_rule(rule: &KasumiMapsRule) -> Result<()> {
    let mut rule = *rule;
    ioctl_with_arg("add_maps_rule", KSM_IOC_ADD_MAPS_RULE, &mut rule)?;
    ensure_kernel_err("Kasumi add_maps_rule", rule.err)
}

pub fn clear_maps_rules() -> Result<()> {
    ioctl_noarg("clear_maps_rules", KSM_IOC_CLEAR_MAPS_RULES)
}

pub fn set_mount_hide(enable: bool) -> Result<()> {
    let config = KasumiMountHideArg::new(enable, None)?;
    set_mount_hide_config(&config)
}

pub fn set_mount_hide_config(config: &KasumiMountHideArg) -> Result<()> {
    let mut config = *config;
    ioctl_with_arg("set_mount_hide", KSM_IOC_SET_MOUNT_HIDE, &mut config)?;
    ensure_kernel_err("Kasumi mount_hide", config.err)
}

pub fn set_maps_spoof(enable: bool) -> Result<()> {
    let config = KasumiMapsSpoofArg::new(enable);
    set_maps_spoof_config(&config)
}

pub fn set_maps_spoof_config(config: &KasumiMapsSpoofArg) -> Result<()> {
    let mut config = *config;
    ioctl_with_arg("set_maps_spoof", KSM_IOC_SET_MAPS_SPOOF, &mut config)?;
    ensure_kernel_err("Kasumi maps_spoof", config.err)
}

pub fn set_statfs_spoof(enable: bool) -> Result<()> {
    let config = KasumiStatfsSpoofArg::new(enable);
    set_statfs_spoof_config(&config)
}

pub fn set_statfs_spoof_config(config: &KasumiStatfsSpoofArg) -> Result<()> {
    let mut config = *config;
    ioctl_with_arg("set_statfs_spoof", KSM_IOC_SET_STATFS_SPOOF, &mut config)?;
    ensure_kernel_err("Kasumi statfs_spoof", config.err)
}

pub fn set_selinux_fix(enable: bool) -> Result<()> {
    ioctl_with_bool("selinux_fix", KSM_IOC_SELINUX_FIX, enable)
}

pub fn release_connection() {
    if let Ok(mut cache) = FD_CACHE.lock()
        && let Some(fd) = cache.take()
    {
        unsafe {
            libc::close(fd);
        }
    }
    invalidate_status_cache();
}

pub fn invalidate_status_cache() {
    if let Ok(mut cache) = STATUS_CACHE.lock() {
        cache.checked = false;
        cache.status = KasumiStatus::NotPresent;
    }
}
