// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

pub const MODULES_IMG_FILE: &str = "/data/adb/hybrid-mount/modules.img";
pub const RUN_DIR: &str = "/data/adb/hybrid-mount/run/";
pub const STATE_FILE: &str = "/data/adb/hybrid-mount/run/daemon_state.json";
pub const DISABLE_FILE_NAME: &str = "disable";
pub const REMOVE_FILE_NAME: &str = "remove";
pub const SKIP_MOUNT_FILE_NAME: &str = "skip_mount";
pub const SYSTEM_RW_DIR: &str = "/data/adb/hybrid-mount/rw";
pub const MODULE_PROP_FILE: &str = "/data/adb/modules/hybrid_mount/module.prop";
pub const MODULES_DIR: &str = "/data/adb/modules";
pub const CONFIG_FILE: &str = "/data/adb/hybrid-mount/config.toml";
pub const MKFS_EROFS_PATH: &str = "/data/adb/metamodule/tools/mkfs.erofs";

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

pub const IGNORE_UNOUNT_PARTITIONS: &[&str] = &[
    "/vendor/lib",
    "/vendor/lib64",
    "/system/lib",
    "/system/lib64",
];

pub const REPLACE_DIR_FILE_NAME: &str = ".replace";
pub const REPLACE_DIR_XATTR: &str = "trusted.overlay.opaque";
