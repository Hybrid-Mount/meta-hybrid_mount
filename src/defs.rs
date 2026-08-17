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

/// 文件级 overlay 规则的 shallow staging 目录(只写运行目录)。
pub const SHALLOW_STAGING_DIR: &str = "/data/adb/hybrid-mount/run/shallow";

/// ext4 staging 镜像(v4.2.0 行为)。
pub const MODULES_IMG_FILE: &str = "/data/adb/hybrid-mount/modules.img";

/// 不注册进内核尝试卸载列表的分区(pairip 完整性校验规避,v4.2.0 行为)。
pub const IGNORE_UNMOUNT_PARTITIONS: &[&str] = &[
    "/vendor/lib",
    "/vendor/lib64",
    "/system/lib",
    "/system/lib64",
];

pub const DEFAULT_MOUNT_SOURCE: &str = "KSU";

/// 挂载临时区(参考项目行为:启动期挂在 RAM 上,不触碰模块目录)。
pub const TMP_ROOT: &str = "/debug_ramdisk";
pub const TMP_WORK_DIR: &str = "/debug_ramdisk/workdir";

/// 模块状态标记文件名与目录标记文件。
pub const MODULE_PROP_FILE_NAME: &str = "module.prop";
pub const DISABLE_FILE_NAME: &str = "disable";
pub const REMOVE_FILE_NAME: &str = "remove";
pub const SKIP_MOUNT_FILE_NAME: &str = "skip_mount";
pub const MOUNT_ERROR_FILE_NAME: &str = "mount_error";
pub const REPLACE_DIR_FILE_NAME: &str = ".replace";

/// 扩展属性名(目录替换标记与 SELinux 上下文)。
pub const REPLACE_DIR_XATTR: &str = "trusted.overlay.opaque";
pub const SELINUX_XATTR: &str = "security.selinux";
