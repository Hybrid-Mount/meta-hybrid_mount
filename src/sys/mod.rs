// ReHybrid-Mount
//
// SPDX-License-Identifier: GPL-3.0-only

//! 系统辅助层:文件系统、挂载与 nuke。
//!
//! Stage 3 脚手架:入口在 Stage 5 CLI 接入前暂未被二进制入口使用;
//! 接入完成后移除本豁免,恢复 dead_code 检查。
#![allow(dead_code)]

pub mod fs;
pub mod temp;

#[cfg(any(target_os = "linux", target_os = "android"))]
pub mod mount;

#[cfg(any(target_os = "linux", target_os = "android"))]
pub mod nuke;
