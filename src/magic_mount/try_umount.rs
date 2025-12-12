#[cfg(any(target_os = "linux", target_os = "android"))]
use std::{ffi::CString, io, os::fd::RawFd, path::Path, sync::OnceLock};

#[cfg(any(target_os = "linux", target_os = "android"))]
use anyhow::Result;

const KSU_INSTALL_MAGIC1: u32 = 0xDEAD_BEEF;
const KSU_IOCTL_ADD_TRY_UMOUNT: u32 = 0x4000_4b12;
const KSU_INSTALL_MAGIC2: u32 = 0xCAFE_BABE;
#[cfg(any(target_os = "linux", target_os = "android"))]
static DRIVER_FD: OnceLock<RawFd> = OnceLock::new();

#[repr(C)]
struct KsuAddTryUmount {
    arg: u64,
    flags: u32,
    mode: u8,
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn send_unmountable<P>(target: P) -> Result<()>
where
    P: AsRef<Path>,
{
    use rustix::path::Arg;

    let path = CString::new(target.as_ref().as_str()?)?;
    let cmd = KsuAddTryUmount {
        arg: path.as_ptr() as u64,
        flags: 2,
        mode: 1,
    };

    let fd = *DRIVER_FD.get_or_init(|| {
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
    });

    unsafe {
        #[cfg(target_env = "gnu")]
        let ret = libc::ioctl(fd as libc::c_int, KSU_IOCTL_ADD_TRY_UMOUNT as u64, &cmd);

        #[cfg(not(target_env = "gnu"))]
        let ret = libc::ioctl(fd as libc::c_int, KSU_IOCTL_ADD_TRY_UMOUNT as i32, &cmd);

        if ret < 0 {
            log::error!(
                "umount {} failed: {}",
                target.as_ref().display(),
                io::Error::last_os_error()
            );

            return Ok(());
        }

        log::info!("umount {} successful!", target.as_ref().display());
    };

    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn send_unmountable() {
    unimplemented!()
}
