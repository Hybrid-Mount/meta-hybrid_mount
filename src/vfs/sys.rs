// SPDX-License-Identifier: GPL-3.0-only

//! keyring transport. The kernel reads the payload from offset 0 of a page, so the
//! buffer is page-aligned `mmap` memory rather than an ordinary `Vec`.

#[cfg(any(target_os = "linux", target_os = "android"))]
pub use platform::{PageBuffer, add_key};

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub use stub::{PageBuffer, add_key};

/// Keyring channel. Rules are only ever sent to `hybridmount`; the upstream
/// NoMount key type is probed purely to detect a foreign implementation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyringChannel {
    /// Hybrid Mount's own implementation.
    Hybridmount,
    /// Upstream NoMount, used only for probing.
    Nomount,
}

#[cfg(any(target_os = "linux", target_os = "android"))]
impl KeyringChannel {
    const fn as_cstr(self) -> &'static std::ffi::CStr {
        match self {
            Self::Hybridmount => c"hybridmount",
            Self::Nomount => c"nomount",
        }
    }
}

pub const LINUX_ECANCELED: i32 = 125;

/// The key type's preparse hook returns `-ECANCELED` once it has handled the payload, so
/// that no key is created; `-ECANCELED` here means the payload was delivered. The real
/// result is in the payload's status field, checked by the caller.
fn interpret_add_key(ret: i64, errno: i32) -> std::io::Result<()> {
    if ret < 0 && errno != LINUX_ECANCELED {
        return Err(std::io::Error::from_raw_os_error(errno));
    }
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
mod platform {
    use std::io;
    use std::ptr::NonNull;

    use super::KeyringChannel;

    pub struct PageBuffer {
        ptr: NonNull<u8>,
    }

    impl PageBuffer {
        pub fn new() -> io::Result<Self> {
            // SAFETY: the mapping stays valid until Drop; the length is fixed.
            let addr = unsafe {
                libc::mmap(
                    std::ptr::null_mut(),
                    crate::vfs::protocol::PAYLOAD_LEN,
                    libc::PROT_READ | libc::PROT_WRITE,
                    libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                    -1,
                    0,
                )
            };
            if addr == libc::MAP_FAILED {
                return Err(io::Error::last_os_error());
            }
            let ptr = NonNull::new(addr.cast::<u8>())
                .ok_or_else(|| io::Error::other("mmap returned null"))?;
            Ok(Self { ptr })
        }

        pub fn as_mut_slice(&mut self) -> &mut [u8] {
            // SAFETY: pointer from mmap, fixed length, exclusive via &mut self.
            unsafe {
                std::slice::from_raw_parts_mut(self.ptr.as_ptr(), crate::vfs::protocol::PAYLOAD_LEN)
            }
        }
    }

    impl Drop for PageBuffer {
        fn drop(&mut self) {
            // SAFETY: pointer and length come from the same mmap.
            unsafe {
                libc::munmap(self.ptr.as_ptr().cast(), crate::vfs::protocol::PAYLOAD_LEN);
            }
        }
    }

    #[cfg(target_arch = "aarch64")]
    const SYS_ADD_KEY: libc::c_long = 217;
    #[cfg(target_arch = "arm")]
    const SYS_ADD_KEY: libc::c_long = 309;
    #[cfg(target_arch = "x86_64")]
    const SYS_ADD_KEY: libc::c_long = 248;

    /// Sends one payload page. The kernel writes `status` back into the same page.
    pub fn add_key(page: &mut PageBuffer, channel: KeyringChannel) -> io::Result<()> {
        let ptr = page.ptr.as_ptr() as libc::c_ulong;
        // SAFETY: variadic syscall matching add_key(type, desc, &ptr, 8, -1).
        let ret = unsafe {
            libc::syscall(
                SYS_ADD_KEY,
                channel.as_cstr().as_ptr(),
                c"trigger".as_ptr(),
                std::ptr::addr_of!(ptr),
                std::mem::size_of::<libc::c_ulong>(),
                -1_i32,
            )
        };
        if ret < 0 {
            // -ECANCELED means the payload was handled; no key is created by design.
            let errno = io::Error::last_os_error().raw_os_error().unwrap_or(0);
            return super::interpret_add_key(ret as i64, errno);
        }
        Ok(())
    }
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
mod stub {
    use std::io;

    use super::KeyringChannel;

    pub struct PageBuffer {
        bytes: Vec<u8>,
    }

    impl PageBuffer {
        pub fn new() -> io::Result<Self> {
            Ok(Self {
                bytes: vec![0_u8; crate::vfs::protocol::PAYLOAD_LEN],
            })
        }

        pub fn as_mut_slice(&mut self) -> &mut [u8] {
            &mut self.bytes
        }
    }

    pub fn add_key(_page: &mut PageBuffer, _channel: KeyringChannel) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "vfs keyring is linux/android only",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_buffer_exposes_one_payload() {
        let mut page = PageBuffer::new().unwrap();
        assert_eq!(page.as_mut_slice().len(), crate::vfs::protocol::PAYLOAD_LEN);
    }

    #[test]
    fn interpret_add_key_treats_ecanceled_as_success() {
        assert!(interpret_add_key(-1, LINUX_ECANCELED).is_ok());
    }

    #[test]
    fn interpret_add_key_rejects_other_errno() {
        assert!(interpret_add_key(-1, 13).is_err());
    }

    #[test]
    fn interpret_add_key_accepts_non_negative_ret() {
        assert!(interpret_add_key(42, 0).is_ok());
    }
}
