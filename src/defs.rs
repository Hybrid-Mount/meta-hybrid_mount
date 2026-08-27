// SPDX-License-Identifier: GPL-3.0-only

//! 全工程共享的路径与常量。

pub const MODULE_ID: &str = "hybrid_mount";

/// 运行目录与持久化产物。
pub const DEFAULT_MODULE_DIR: &str = "/data/adb/modules";
pub const SELF_MODULE_DIR: &str = "/data/adb/modules/hybrid_mount";
pub const SELF_MODULE_PROP: &str = "/data/adb/modules/hybrid_mount/module.prop";

pub const CONFIG_PATH: &str = "/data/adb/hybrid-mount/config.toml";
pub const MODULE_BLACKLIST_FILE_NAME: &str = "module_blacklist.toml";
pub const MODULE_BLACKLIST_PATH: &str = "/data/adb/hybrid-mount/module_blacklist.toml";
pub const BUNDLED_MODULE_BLACKLIST_PATH: &str =
    "/data/adb/modules/hybrid_mount/module_blacklist.toml";
pub const SCAN_RET_PATH: &str = "/data/adb/hybrid-mount/scan.ret";
pub const STATE_PATH: &str = "/data/adb/hybrid-mount/run/state.json";

/// ext4 staging 镜像(v4.2.0 行为)。
pub const MODULES_IMG_FILE: &str = "/data/adb/hybrid-mount/modules.img";

/// 不注册进内核尝试卸载列表的分区(pairip 完整性校验规避,v4.2.0 行为)。
pub const IGNORE_UNMOUNT_PARTITIONS: &[&str] = &[
    "/vendor/lib",
    "/vendor/lib64",
    "/system/lib",
    "/system/lib64",
];

/// Partition roots supported by both the installer and the mount pipeline.
/// Runtime discovery still filters this list to roots that exist on-device.
pub const MANAGED_PARTITIONS: &[&str] = &[
    "odm",
    "product",
    "system_ext",
    "vendor",
    "apex",
    "mi_ext",
    "my_bigball",
    "my_carrier",
    "my_company",
    "my_engineering",
    "my_heytap",
    "my_manifest",
    "my_preload",
    "my_product",
    "my_region",
    "my_reserve",
    "my_stock",
    "oem",
    "optics",
    "prism",
];

pub const DEFAULT_MOUNT_SOURCE: &str = "KSU";

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
