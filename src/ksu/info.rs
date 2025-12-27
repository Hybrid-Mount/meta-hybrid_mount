use crate::ksu::{get_fd, magic::KSU_IOCTL_GET_INFO};

#[repr(C)]
struct GetInfoCmd {
    version: u32,
    flags: u32,
}

fn info() -> Option<GetInfoCmd> {
    let mut cmd = GetInfoCmd {
        version: 0,
        flags: 0,
    };

    let fd = get_fd();
    let ret = unsafe { libc::ioctl(fd as libc::c_int, KSU_IOCTL_GET_INFO, &mut cmd) };

    if ret < 0 { None } else { Some(cmd) }
}

fn version() -> Option<u32> {
    info().map(|info| info.version)
}

pub fn check() {
    let status = version().is_some_and(|v| {
        log::info!("KernelSU Version: {v}");
        true
    });

    if !status {
        log::error!("only support KernelSU!!");
        panic!();
    }
}
