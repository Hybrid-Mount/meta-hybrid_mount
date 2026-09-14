// SPDX-License-Identifier: GPL-3.0-only

//! keyring 发送通道。内核 `nm_key_preparse` 要求 payload 位于一页的偏移 0，
//! 因此这里用 `mmap` 分配页对齐缓冲，绝不复用普通 `Vec` 堆内存。

// add_key 由后续任务（任务 8 的发送侧）消费；接入前该重导出暂无使用点，
// 这里显式放行，避免 clippy -D warnings 因未使用导入而失败。
#[cfg(any(target_os = "linux", target_os = "android"))]
#[allow(unused_imports)]
pub use platform::{PageBuffer, add_key};

#[cfg(not(any(target_os = "linux", target_os = "android")))]
#[allow(unused_imports)]
pub use stub::{PageBuffer, add_key};

#[cfg(any(target_os = "linux", target_os = "android"))]
mod platform {
    use std::io;
    use std::ptr::NonNull;

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
    pub fn add_key(page: &mut PageBuffer) -> io::Result<()> {
        let ptr = page.ptr.as_ptr() as libc::c_ulong;
        // SAFETY: 变参 syscall，参数布局与内核 add_key(type, desc, &ptr, 8, -1) 一致。
        let ret = unsafe {
            libc::syscall(
                SYS_ADD_KEY,
                c"nomount".as_ptr(),
                c"trigger".as_ptr(),
                std::ptr::addr_of!(ptr),
                std::mem::size_of::<libc::c_ulong>(),
                -1_i32,
            )
        };
        if ret < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
mod stub {
    use std::io;

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

    pub fn add_key(_page: &mut PageBuffer) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "nomount keyring is linux/android only",
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
}
