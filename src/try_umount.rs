use std::{
    collections::HashSet,
    ffi::CString,
    os::fd::RawFd,
    path::Path,
    sync::{Mutex, OnceLock},
};

use anyhow::{Context, Result, bail};
use nix::ioctl_write_ptr_bad;

const KSU_INSTALL_MAGIC1: u32 = 0xDEADBEEF;

const KSU_INSTALL_MAGIC2: u32 = 0xCAFEBABE;

const KSU_IOCTL_NUKE_EXT4_SYSFS: u32 = 0x40004b11;

const KSU_IOCTL_ADD_TRY_UMOUNT: u32 = 0x40004b12;

static DRIVER_FD: OnceLock<RawFd> = OnceLock::new();

static SENT_UNMOUNTS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

#[repr(C)]

struct KsuAddTryUmount {
    arg: u64,
    flags: u32,
    mode: u8,
}

#[repr(C)]

struct NukeExt4SysfsCmd {
    arg: u64,
}

ioctl_write_ptr_bad!(
    ksu_add_try_umount,
    KSU_IOCTL_ADD_TRY_UMOUNT,
    KsuAddTryUmount
);

ioctl_write_ptr_bad!(
    ksu_nuke_ext4_sysfs,
    KSU_IOCTL_NUKE_EXT4_SYSFS,
    NukeExt4SysfsCmd
);

fn grab_fd() -> i32 {
    let mut fd = -1;

    unsafe {
        libc::syscall(
            libc::SYS_reboot,
            KSU_INSTALL_MAGIC1,
            KSU_INSTALL_MAGIC2,
            0,
            &mut fd,
        );
    };

    fd
}

pub fn send_unmountable<P>(target: P) -> Result<()>
where
    P: AsRef<Path>,
{
    let path_ref = target.as_ref();

    let path_str = path_ref.to_string_lossy().to_string();

    if path_str.is_empty() {
        return Ok(());
    }

    let cache = SENT_UNMOUNTS.get_or_init(|| Mutex::new(HashSet::new()));

    let mut set = cache.lock().unwrap();

    if set.contains(&path_str) {
        log::debug!("Unmount skipped (dedup): {}", path_str);

        return Ok(());
    }

    set.insert(path_str.clone());

    let path = CString::new(path_str)?;

    let cmd = KsuAddTryUmount {
        arg: path.as_ptr() as u64,
        flags: 2,
        mode: 1,
    };

    let fd = *DRIVER_FD.get_or_init(grab_fd);

    if fd < 0 {
        return Ok(());
    }

    unsafe {
        ksu_add_try_umount(fd, &cmd)?;
    }

    Ok(())
}

pub fn ksu_nuke_sysfs(target: &str) -> Result<()> {
    let c_path = CString::new(target)?;

    let cmd = NukeExt4SysfsCmd {
        arg: c_path.as_ptr() as u64,
    };

    let fd = *DRIVER_FD.get_or_init(grab_fd);

    if fd < 0 {
        bail!("KSU driver not available");
    }

    unsafe {
        ksu_nuke_ext4_sysfs(fd, &cmd).context("KSU Nuke Sysfs ioctl failed")?;
    }

    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]

pub fn ksu_nuke_sysfs(_target: &str) -> Result<()> {
    bail!("Not supported on this OS")
}
