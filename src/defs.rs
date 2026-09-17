// SPDX-License-Identifier: GPL-3.0-only

pub const MODULE_ID: &str = "hybrid_mount";

/// Runtime directory and persisted artifacts.
pub const DEFAULT_MODULE_DIR: &str = "/data/adb/modules";
pub const SELF_MODULE_DIR: &str = "/data/adb/modules/hybrid_mount";
pub const SELF_MODULE_PROP: &str = "/data/adb/modules/hybrid_mount/module.prop";
pub const MODULE_LKM_DIR: &str = "/data/adb/modules/hybrid_mount/lkm/binaries";
pub const LKM_BOOT_GUARD_PATH: &str = "/data/adb/hybrid-mount/lkm_boot_guard";

/// Hybrid Mount's own VFS kernel modules (K2), prebuilt per Android/GKI target and
/// named hybridmount-<android>-<kernel>.ko.
pub const VFS_LKM_DIR: &str = "/data/adb/modules/hybrid_mount/vfs/binaries";
pub const VFS_LKM_BOOT_GUARD_PATH: &str = "/data/adb/hybrid-mount/vfs_lkm_boot_guard";

/// VFS boot guard: written before rules are applied and cleared on success, so a hard
/// crash leaves it behind and the next boot skips the VFS backend.
pub const VFS_BOOT_GUARD_PATH: &str = "/data/adb/hybrid-mount/vfs_boot_guard";

pub const CONFIG_PATH: &str = "/data/adb/hybrid-mount/config.toml";
pub const MODULE_BLACKLIST_FILE_NAME: &str = "module_blacklist.toml";
pub const MODULE_BLACKLIST_PATH: &str = "/data/adb/hybrid-mount/module_blacklist.toml";
pub const BUNDLED_MODULE_BLACKLIST_PATH: &str =
    "/data/adb/modules/hybrid_mount/module_blacklist.toml";
pub const SCAN_RET_PATH: &str = "/data/adb/hybrid-mount/scan.ret";
pub const STATE_PATH: &str = "/data/adb/hybrid-mount/run/state.json";

/// ext4 staging images (v4.2.0 behaviour).
pub const MODULES_IMG_FILE: &str = "/data/adb/hybrid-mount/modules.img";

/// Partitions kept out of the kernel try-umount list (pairip integrity-check workaround, v4.2.0 behaviour).
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

/// Module status marker filenames and directory markers.
pub const MODULE_PROP_FILE_NAME: &str = "module.prop";
pub const DISABLE_FILE_NAME: &str = "disable";
pub const REMOVE_FILE_NAME: &str = "remove";
pub const SKIP_MOUNT_FILE_NAME: &str = "skip_mount";
pub const MOUNT_ERROR_FILE_NAME: &str = "mount_error";
pub const REPLACE_DIR_FILE_NAME: &str = ".replace";

/// Extended attribute names: the directory replace marker and the SELinux context.
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

    #[test]
    fn vfs_boot_guard_lives_under_run_directory() {
        assert!(VFS_BOOT_GUARD_PATH.starts_with("/data/adb/hybrid-mount/"));
        assert_ne!(VFS_BOOT_GUARD_PATH, LKM_BOOT_GUARD_PATH);
    }
}
