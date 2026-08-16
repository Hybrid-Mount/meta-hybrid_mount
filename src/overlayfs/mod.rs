// ReHybrid-Mount
//
// SPDX-License-Identifier: GPL-3.0-only

//! OverlayFS 后端(行为参考 v4.2.0 tag `e20f9c19`,新写实现)。
//!
//! 纯算法(转义、层拆分、子挂载相对路径)跨平台可测;
//! fsopen / mount / bind / mountinfo 执行部分仅 Linux/Android。

#[allow(clippy::module_inception)]
pub mod overlayfs;

#[cfg(any(target_os = "linux", target_os = "android"))]
pub mod utils;
