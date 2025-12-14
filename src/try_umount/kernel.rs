use std::{
    ffi::CString,
    os::fd::RawFd,
    path::Path,
    sync::{Mutex, OnceLock},
    collections::HashSet,
};
use anyhow::Result;

const K: u32 = b'K' as u32;

const _IOC_NRBITS: u32 = 8;
const _IOC_TYPEBITS: u32 = 8;
const _IOC_SIZEBITS: u32 = 14;
const _IOC_DIRBITS: u32 = 2;

const _IOC_NRSHIFT: u32 = 0;
const _IOC_TYPESHIFT: u32 = _IOC_NRSHIFT + _IOC_NRBITS;
const _IOC_SIZESHIFT: u32 = _IOC_TYPESHIFT + _IOC_TYPEBITS;
const _IOC_DIRSHIFT: u32 = _IOC_SIZESHIFT + _IOC_SIZEBITS;

const _IOC_WRITE: u32 = 1;

const fn _ioc(dir: u32, type_: u32, nr: u32, size: u32) -> u32 {
    (dir << _IOC_DIRSHIFT) | (type_ << _IOC_TYPESHIFT) | (nr << _IOC_NRSHIFT) | (size << _IOC_SIZESHIFT)
}

const fn _iow(type_: u32, nr: u32, size: u32) -> u32 {
    _ioc(_IOC_WRITE, type_, nr, size)
}

const KSU_INSTALL_MAGIC1: u32 = 0xDEADBEEF;
const KSU_INSTALL_MAGIC2: u32 = 0xCAFEBABE;
const KSU_IOCTL_ADD_TRY_UMOUNT: i32 = _iow(K, 18, 0) as i32;

static DRIVER_FD: OnceLock<RawFd> = OnceLock::new();
static SENT_UNMOUNTS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

#[repr(C)]
struct KsuAddTryUmount {
    arg: u64,
    flags: u32,
    mode: u8,
}

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

pub(super) fn send_kernel_umount<P>(target: P) -> Result<()>
where
    P: AsRef<Path>,
{
    let path_ref = target.as_ref();
    let path_str = path_ref.to_string_lossy().to_string(); 
    if path_str.is_empty() { return Ok(()); }

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
    if fd < 0 { return Ok(()); }

    let ret = unsafe {
        #[cfg(target_env = "gnu")]
        {
            libc::ioctl(fd as libc::c_int, KSU_IOCTL_ADD_TRY_UMOUNT as u64, &cmd)
        }
        #[cfg(not(target_env = "gnu"))]
        {
            libc::ioctl(fd as libc::c_int, KSU_IOCTL_ADD_TRY_UMOUNT as i32, &cmd)
        }
    };

    if ret < 0 {
        log::error!(
            "umount {} failed: {}",
            target.as_ref().display(),
            std::io::Error::last_os_error()
        );
        return Ok(());
    }

    log::info!("umount {} successful!", target.as_ref().display());
    Ok(())
}
