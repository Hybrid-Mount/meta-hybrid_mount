// SPDX-License-Identifier: GPL-3.0-only

//! Magic Mount 后端，兼容 meta-magic_mount-rs `8b85c9e` 的行为。
//!
//! 节点类型、模块贡献和后端选择统一位于 [`crate::mount_tree`]；本模块只保留
//! Magic Mount 执行器，不再二次扫描模块目录或构建私有节点树。

#[cfg(any(target_os = "linux", target_os = "android"))]
pub mod exec;
