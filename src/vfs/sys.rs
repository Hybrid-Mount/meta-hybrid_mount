// SPDX-License-Identifier: GPL-3.0-only

//! keyring 发送通道。内核 `nm_key_preparse` 要求 payload 位于一页的偏移 0，
//! 因此这里用 `mmap` 分配页对齐缓冲，绝不复用普通 `Vec` 堆内存。

#[cfg(any(target_os = "linux", target_os = "android"))]
pub use platform::{PageBuffer, add_key};

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub use stub::{PageBuffer, add_key};

/// keyring 通道。K2（Hybrid Mount 自有 VFS 内核子系统）是唯一被驱动的实现；
/// 上游 NoMount 的 key type 只用于探测设备上是否已存在外来实现（单向守卫），
/// 绝不用它下发规则。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyringChannel {
    /// Hybrid Mount 自有实现（K2）。
    Hybridmount,
    /// 上游 NoMount，仅用于探测。
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

/// Linux/Android 的 ECANCELED。NoMount 的 key type preparse 在处理完 payload 后
/// 故意返回 -ECANCELED（使 key 不被创建），因此该 errno 表示成功。
pub const LINUX_ECANCELED: i32 = 125;

/// 解释 `add_key` 的原始返回值。
///
/// NoMount 的 `nm_key_preparse` 在处理器处理完 payload 后总是返回 -ECANCELED，
/// 这是它“不创建 key”的正常机制；上游 `nm.c` 完全忽略 `add_key` 的返回值，只读回
/// payload 里的 `status`。因此当 syscall 返回 -1/ECANCELED 时这里是成功，payload 的
/// 真实结果由调用方的 `ensure_status` / `ensure_consumed` 判定。
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
            // SAFETY: mmap 返回的映射在本结构 Drop 前保持有效，长度固定为 PAYLOAD_LEN。
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
            // SAFETY: 指针来自 mmap，长度固定，且 &mut self 保证独占访问。
            unsafe {
                std::slice::from_raw_parts_mut(self.ptr.as_ptr(), crate::vfs::protocol::PAYLOAD_LEN)
            }
        }
    }

    impl Drop for PageBuffer {
        fn drop(&mut self) {
            // SAFETY: 指针与长度来自同一 mmap。
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

    /// 发送一页 payload。内核通过同一页回写 `status`，调用方随后自行解析。
    pub fn add_key(page: &mut PageBuffer, channel: KeyringChannel) -> io::Result<()> {
        let ptr = page.ptr.as_ptr() as libc::c_ulong;
        // SAFETY: 变参 syscall，参数布局与内核 add_key(type, desc, &ptr, 8, -1) 一致。
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
            // ECANCELED 表示内核已处理 payload 且刻意不创建 key，属正常完成。
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
