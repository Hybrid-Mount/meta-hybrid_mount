// ReHybrid-Mount
//
// SPDX-License-Identifier: GPL-3.0-only

//! 全工程共享的路径与常量。
//!
//! 铁律:本文件只定义路径/常量,不引用任何已删除的组件或品牌符号。
//!
//! Stage 1 脚手架:常量在 Stage 2-5 各子系统接入前暂未全部使用;
//! 接入完成后移除本豁免,恢复 dead_code 检查。
#![allow(dead_code)]

pub const PROJECT_NAME: &str = "ReHybrid-Mount";
pub const MODULE_ID: &str = "hybrid_mount";
pub const MODULE_NAME: &str = "Hybrid Mount";

/// 运行目录与持久化产物(见 REHYBRID_MOUNT_PLAN.md 第 1 节目标架构)。
pub const ADB_DIR: &str = "/data/adb";
pub const DEFAULT_MODULE_DIR: &str = "/data/adb/modules";
pub const SELF_MODULE_DIR: &str = "/data/adb/modules/hybrid_mount";
pub const RUN_DIR: &str = "/data/adb/hybrid-mount";
pub const STATE_DIR: &str = "/data/adb/hybrid-mount/run";

pub const CONFIG_PATH: &str = "/data/adb/hybrid-mount/config.toml";
pub const SCAN_RET_PATH: &str = "/data/adb/hybrid-mount/scan.ret";
pub const STATE_PATH: &str = "/data/adb/hybrid-mount/run/state.json";

pub const DEFAULT_MOUNT_SOURCE: &str = "KSU";
