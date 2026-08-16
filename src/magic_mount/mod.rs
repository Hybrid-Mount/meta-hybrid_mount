// ReHybrid-Mount
//
// SPDX-License-Identifier: GPL-3.0-only

//! Magic Mount 后端(行为参考 meta-magic_mount-rs master `8b85c9e`,新写实现)。
//!
//! 分层:
//! - [`node`]:跨平台的 Node 树与纯算法(合并、分区提升、skip 判定)。
//! - [`scan`]:Unix 侧只读模块扫描,收集 planner 判定为 magic 的模块/路径。
//! - [`exec`]:Linux/Android 侧的挂载执行(tmpfs skeleton、mirror、只读 remount)。

pub mod node;

#[cfg(unix)]
pub mod scan;

#[cfg(any(target_os = "linux", target_os = "android"))]
pub mod exec;
