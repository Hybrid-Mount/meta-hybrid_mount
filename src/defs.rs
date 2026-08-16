// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#[cfg(any(target_os = "linux", target_os = "android"))]
#[allow(dead_code)]
pub const HYBRID_MOUNT_DIR: &str = "/data/adb/hybrid-mount";
pub const MODULES_DIR: &str = "/data/adb/modules";
#[allow(dead_code)]
pub const HYBRID_MOUNT_MODULE_DIR: &str = "/data/adb/modules/hybrid_mount";
#[allow(dead_code)]
pub const HYBRID_MOUNT_MODULE_UPDATE_DIR: &str = "/data/adb/modules_update/hybrid_mount";

pub const MODULES_IMG_FILE: &str = "/data/adb/hybrid-mount/modules.img";
pub const RUN_DIR: &str = "/data/adb/hybrid-mount/run/";
pub const STATE_FILE: &str = "/data/adb/hybrid-mount/run/daemon_state.json";
pub const SOCKET_FILE: &str = "/data/adb/hybrid-mount/run/daemon.sock";
pub const PID_FILE: &str = "/data/adb/hybrid-mount/run/daemon.pid";
pub const SYSTEM_RW_DIR: &str = "/data/adb/hybrid-mount/rw";
pub const CONFIG_FILE: &str = "/data/adb/hybrid-mount/config.toml";
pub const MODULE_BLACKLIST_FILE: &str = "/data/adb/hybrid-mount/module_blacklist.toml";
pub const MODULE_PROP_FILE: &str = "/data/adb/modules/hybrid_mount/module.prop";

pub const DISABLE_FILE_NAME: &str = "disable";
pub const REMOVE_FILE_NAME: &str = "remove";
pub const MOUNT_ERROR_FILE_NAME: &str = "mount_error";
pub const SKIP_MOUNT_FILE_NAME: &str = "skip_mount";
pub const REPLACE_DIR_FILE_NAME: &str = ".replace";
#[cfg(any(target_os = "linux", target_os = "android"))]
pub const REPLACE_DIR_XATTR: &str = "trusted.overlay.opaque";

pub const IGNORE_UNMOUNT_PARTITIONS: &[&str] = &[
    "/vendor/lib",
    "/vendor/lib64",
    "/system/lib",
    "/system/lib64",
];

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

pub const MAX_MERGE_JSON_DEPTH: usize = 64;
