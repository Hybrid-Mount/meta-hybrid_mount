// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

use const_format::concatcp;

pub const ADB_DIR: &str = "/data/adb";
pub const HYBRID_MOUNT_DIR: &str = concatcp!(ADB_DIR, "/hybrid-mount");
pub const MODULES_DIR: &str = concatcp!(ADB_DIR, "/modules");
pub const HYBRID_MOUNT_MODULE_DIR: &str = concatcp!(MODULES_DIR, "/hybrid_mount");

pub const MODULES_IMG_FILE: &str = concatcp!(HYBRID_MOUNT_DIR, "/modules.img");
pub const HYMOFS_IMG_FILE: &str = concatcp!(HYBRID_MOUNT_DIR, "/hymofs.img");
pub const RUN_DIR: &str = concatcp!(HYBRID_MOUNT_DIR, "/run/");
pub const STATE_FILE: &str = concatcp!(RUN_DIR, "daemon_state.json");
pub const DAEMON_LOG_FILE: &str = concatcp!(HYBRID_MOUNT_DIR, "/daemon.log");
pub const SYSTEM_RW_DIR: &str = concatcp!(HYBRID_MOUNT_DIR, "/rw");
pub const CONFIG_FILE: &str = concatcp!(HYBRID_MOUNT_DIR, "/config.toml");
pub const USER_HIDE_RULES_FILE: &str = concatcp!(HYBRID_MOUNT_DIR, "/user_hide_rules.json");
pub const MODULE_PROP_FILE: &str = concatcp!(HYBRID_MOUNT_MODULE_DIR, "/module.prop");
pub const HYMOFS_MIRROR_DIR: &str = "/dev/hymo_mirror";
pub const HYMOFS_LKM_DIR: &str = concatcp!(HYBRID_MOUNT_MODULE_DIR, "/hymofs_lkm");
pub const HYMOFS_LKM_FILE: &str = concatcp!(HYBRID_MOUNT_MODULE_DIR, "/hymofs_lkm.ko");
pub const HYMOFS_LKM_MODULE_NAME: &str = "hymofs_lkm";

pub const DISABLE_FILE_NAME: &str = "disable";
pub const REMOVE_FILE_NAME: &str = "remove";
pub const MOUNT_ERROR_FILE_NAME: &str = "mount_error";
// Legacy marker kept for backward compatibility with existing installations.
pub const SKIP_MOUNT_FILE_NAME: &str = "skip_mount";
pub const REPLACE_DIR_FILE_NAME: &str = ".replace";
#[cfg(any(target_os = "linux", target_os = "android"))]
pub const REPLACE_DIR_XATTR: &str = "trusted.overlay.opaque";

pub const BUILTIN_PARTITIONS: &[&str] = &[
    "system",
    "vendor",
    "product",
    "system_ext",
    "odm",
    "oem",
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
    "optics",
    "prism",
];

pub const SENSITIVE_PARTITIONS: &[&str] = &[
    "vendor",
    "product",
    "system_ext",
    "odm",
    "oem",
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
    "optics",
    "prism",
];

pub const IGNORE_UNMOUNT_PARTITIONS: &[&str] = &[
    "/vendor/lib",
    "/vendor/lib64",
    "/system/lib",
    "/system/lib64",
];
