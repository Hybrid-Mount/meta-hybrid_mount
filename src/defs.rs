// SPDX-License-Identifier: GPL-3.0-only

//! 全工程共享的路径与常量。

pub const MODULE_ID: &str = "hybrid_mount";

/// 运行目录与持久化产物。
pub const DEFAULT_MODULE_DIR: &str = "/data/adb/modules";
pub const SELF_MODULE_DIR: &str = "/data/adb/modules/hybrid_mount";
pub const SELF_MODULE_PROP: &str = "/data/adb/modules/hybrid_mount/module.prop";
pub const MODULE_LKM_DIR: &str = "/data/adb/modules/hybrid_mount/lkm/binaries";
pub const LKM_BOOT_GUARD_PATH: &str = "/data/adb/hybrid-mount/lkm_boot_guard";

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
/// `/apex` is intentionally excluded because apexd owns its activation tree.
pub const MANAGED_PARTITIONS: &[&str] = &[
    "odm",
    "product",
    "system_ext",
    "vendor",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apex_is_excluded_from_managed_partition_policy() {
        assert!(!MANAGED_PARTITIONS.contains(&"apex"));

        let metainstall = include_str!("../module/metainstall.sh");
        let Some(partitions) = metainstall
            .lines()
            .find(|line| line.starts_with("MANAGED_PARTITIONS="))
        else {
            panic!("metainstall.sh is missing MANAGED_PARTITIONS");
        };
        let Some((_, partitions)) = partitions.split_once('=') else {
            panic!("metainstall.sh has an invalid MANAGED_PARTITIONS assignment");
        };
        let installer_partitions = partitions
            .trim_matches('"')
            .split_whitespace()
            .collect::<Vec<_>>();

        assert_eq!(installer_partitions, MANAGED_PARTITIONS);
    }
}
