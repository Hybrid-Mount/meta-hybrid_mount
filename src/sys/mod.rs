// SPDX-License-Identifier: GPL-3.0-only

//! 系统辅助层:文件系统、挂载与 nuke。

pub mod fs;
pub mod temp;

#[cfg(any(target_os = "linux", target_os = "android"))]
pub mod mount;

#[cfg(any(target_os = "linux", target_os = "android"))]
pub mod nuke;
