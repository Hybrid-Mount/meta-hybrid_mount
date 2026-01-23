use std::os::unix::io::AsRawFd;

use anyhow::Result;
use nix::ioctl_write_ptr;

const MAGIC: u8 = 0x43;

#[repr(C)]
pub struct IoctlSpoofArgs {
    pub name: [u8; 256],
    pub uid: u32,
    pub gid: u32,
    pub mode: u16,
    pub mtime: u64,
}

ioctl_write_ptr!(add_hide, MAGIC, 1, [u8; 256]);
ioctl_write_ptr!(del_hide, MAGIC, 2, [u8; 256]);
ioctl_write_ptr!(add_redirect, MAGIC, 4, [u8; 512]);
ioctl_write_ptr!(del_redirect, MAGIC, 5, [u8; 256]);
ioctl_write_ptr!(add_spoof, MAGIC, 7, IoctlSpoofArgs);
ioctl_write_ptr!(del_spoof, MAGIC, 8, [u8; 256]);
ioctl_write_ptr!(add_merge, MAGIC, 10, [u8; 512]);
ioctl_write_ptr!(del_merge, MAGIC, 11, [u8; 256]);
ioctl_write_ptr!(set_trusted_gid, MAGIC, 13, u32);

pub fn hide(fd: &impl AsRawFd, name: &str) -> Result<()> {
    let mut buf = [0u8; 256];
    let bytes = name.as_bytes();
    if bytes.len() >= 256 {
        anyhow::bail!("Name too long");
    }
    buf[..bytes.len()].copy_from_slice(bytes);
    unsafe { add_hide(fd.as_raw_fd(), &buf) }?;
    Ok(())
}

pub fn unhide(fd: &impl AsRawFd, name: &str) -> Result<()> {
    let mut buf = [0u8; 256];
    let bytes = name.as_bytes();
    if bytes.len() >= 256 {
        anyhow::bail!("Name too long");
    }
    buf[..bytes.len()].copy_from_slice(bytes);
    unsafe { del_hide(fd.as_raw_fd(), &buf) }?;
    Ok(())
}

pub fn redirect(fd: &impl AsRawFd, src: &str, target: &str) -> Result<()> {
    let mut buf = [0u8; 512];
    let payload = format!("{}|{}", src, target);
    let bytes = payload.as_bytes();
    if bytes.len() >= 512 {
        anyhow::bail!("Payload too long");
    }
    buf[..bytes.len()].copy_from_slice(bytes);
    unsafe { add_redirect(fd.as_raw_fd(), &buf) }?;
    Ok(())
}

pub fn unredirect(fd: &impl AsRawFd, src: &str) -> Result<()> {
    let mut buf = [0u8; 256];
    let bytes = src.as_bytes();
    if bytes.len() >= 256 {
        anyhow::bail!("Name too long");
    }
    buf[..bytes.len()].copy_from_slice(bytes);
    unsafe { del_redirect(fd.as_raw_fd(), &buf) }?;
    Ok(())
}

pub fn spoof(
    fd: &impl AsRawFd,
    name: &str,
    uid: u32,
    gid: u32,
    mode: u16,
    mtime: u64,
) -> Result<()> {
    let mut name_buf = [0u8; 256];
    let bytes = name.as_bytes();
    if bytes.len() >= 256 {
        anyhow::bail!("Name too long");
    }
    name_buf[..bytes.len()].copy_from_slice(bytes);
    let args = IoctlSpoofArgs {
        name: name_buf,
        uid,
        gid,
        mode,
        mtime,
    };
    unsafe { add_spoof(fd.as_raw_fd(), &args) }?;
    Ok(())
}

pub fn unspoof(fd: &impl AsRawFd, name: &str) -> Result<()> {
    let mut buf = [0u8; 256];
    let bytes = name.as_bytes();
    if bytes.len() >= 256 {
        anyhow::bail!("Name too long");
    }
    buf[..bytes.len()].copy_from_slice(bytes);
    unsafe { del_spoof(fd.as_raw_fd(), &buf) }?;
    Ok(())
}

pub fn merge(fd: &impl AsRawFd, src: &str, target: &str) -> Result<()> {
    let mut buf = [0u8; 512];
    let payload = format!("{}|{}", src, target);
    let bytes = payload.as_bytes();
    if bytes.len() >= 512 {
        anyhow::bail!("Payload too long");
    }
    buf[..bytes.len()].copy_from_slice(bytes);
    unsafe { add_merge(fd.as_raw_fd(), &buf) }?;
    Ok(())
}

pub fn unmerge(fd: &impl AsRawFd, src: &str) -> Result<()> {
    let mut buf = [0u8; 256];
    let bytes = src.as_bytes();
    if bytes.len() >= 256 {
        anyhow::bail!("Name too long");
    }
    buf[..bytes.len()].copy_from_slice(bytes);
    unsafe { del_merge(fd.as_raw_fd(), &buf) }?;
    Ok(())
}

pub fn set_trust(fd: &impl AsRawFd, gid: u32) -> Result<()> {
    unsafe { set_trusted_gid(fd.as_raw_fd(), &gid) }?;
    Ok(())
}
