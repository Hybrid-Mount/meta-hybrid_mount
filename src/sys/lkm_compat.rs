// SPDX-License-Identifier: GPL-3.0-only

//! Built-in equivalent of NoMount's lkmloader fallback; executed as a child command.

use std::ffi::CString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, Read, Seek, SeekFrom, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use super::lkm_image::{ModuleImage, load_with_vermagic_retry};
use crate::errors::{Error, Result};

const KPTR_RESTRICT: &str = "/proc/sys/kernel/kptr_restrict";

pub fn handle(args: &[String]) -> Result<()> {
    let path = args
        .first()
        .ok_or_else(|| Error::msg("usage: lkm-load <module.ko> [parameters...]"))?;
    if std::env::consts::ARCH != "aarch64" {
        return Err(Error::msg("built-in LKM loading is aarch64-only"));
    }
    let parameters = args[1..].join(" ");
    if parameters.len() > 4095 {
        return Err(Error::msg("LKM parameters exceed 4095 bytes"));
    }
    let parameters = CString::new(parameters).map_err(|_| Error::msg("NUL in LKM parameters"))?;
    let mut bytes = Vec::new();
    File::open(path)?
        .take(64 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 64 * 1024 * 1024 {
        return Err(Error::msg("LKM image exceeds 64 MiB"));
    }
    let mut image = ModuleImage::parse(bytes).map_err(Error::msg)?;
    // Restore visibility before entering the kernel, including when symbol parsing fails.
    {
        let _visibility = KernelPointerVisibility::relax_for_root();
        match File::open("/proc/kallsyms") {
            Ok(symbols) => {
                let count = image
                    .resolve_symbols(BufReader::new(symbols))
                    .map_err(Error::msg)?;
                log::info!(
                    "built-in LKM loader: resolved {count} symbols for {}",
                    image.name
                );
            }
            Err(err) => {
                log::warn!("cannot read kallsyms; retaining kernel symbol resolution: {err}")
            }
        }
    }
    let mut kmsg = open_kernel_log();
    load_with_vermagic_retry(
        &mut image,
        |bytes| insert_module(bytes, &parameters),
        |image| read_required_vermagic(kmsg.as_mut(), image),
    )
    .map_err(Error::msg)
}

struct KernelPointerVisibility(Option<String>);

impl KernelPointerVisibility {
    fn relax_for_root() -> Self {
        let original = fs::read_to_string(KPTR_RESTRICT)
            .ok()
            .filter(|value| value.trim() == "2");
        if let Some(original) = original
            && fs::write(KPTR_RESTRICT, "1\n").is_ok()
        {
            return Self(Some(original));
        }
        Self(None)
    }
}

impl Drop for KernelPointerVisibility {
    fn drop(&mut self) {
        if let Some(original) = &self.0
            && let Err(err) = fs::write(KPTR_RESTRICT, original)
        {
            log::warn!("restore kptr_restrict after symbol lookup: {err}");
        }
    }
}

fn insert_module(bytes: &[u8], parameters: &CString) -> io::Result<()> {
    // SAFETY: both input buffers remain valid for the duration of this synchronous syscall.
    let status = unsafe {
        libc::syscall(
            libc::SYS_init_module,
            bytes.as_ptr(),
            bytes.len(),
            parameters.as_ptr(),
        )
    };
    if status == 0 {
        return Ok(());
    }
    let error = io::Error::last_os_error();
    if !matches!(error.raw_os_error(), Some(libc::ENOSYS | libc::EPERM)) {
        return Err(error);
    }

    // Some kernels only permit finit_module. Supply the exact adapted image, without padding.
    // SAFETY: the name is a terminated static C string; ownership transfers once to File below.
    let fd = unsafe {
        libc::syscall(
            libc::SYS_memfd_create,
            c"hybridmount-lkm".as_ptr(),
            libc::MFD_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: fd was just allocated above and has no other owner.
    let mut file = unsafe { File::from_raw_fd(fd as i32) };
    file.write_all(bytes)?;
    // SAFETY: the fd and parameter buffer remain valid throughout this syscall.
    let status = unsafe {
        libc::syscall(
            libc::SYS_finit_module,
            file.as_raw_fd(),
            parameters.as_ptr(),
            0,
        )
    };
    if status == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

fn open_kernel_log() -> Option<File> {
    for path in ["/dev/kmsg", "/kmsg"] {
        if let Ok(mut file) = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(Path::new(path))
            && file.seek(SeekFrom::End(0)).is_ok()
        {
            return Some(file);
        }
    }
    None
}

fn read_required_vermagic(log: Option<&mut File>, image: &ModuleImage) -> Option<String> {
    let log = log?;
    let mut buffer = [0_u8; 8192];
    // Bound work even if other drivers continuously produce kernel messages.
    for _ in 0..256 {
        match log.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => {
                if let Some(required) =
                    image.requested_vermagic(&String::from_utf8_lossy(&buffer[..count]))
                {
                    return Some(required);
                }
            }
            Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
    None
}
