pub const DEFAULT_HYBRID_MNT_DIR: &str = "/debug_ramdisk";
pub const BASE_DIR: &str = "/data/adb/meta-hybrid/";
pub const MODULES_IMG_FILE: &str = "/data/adb/meta-hybrid/modules.img";
pub const RUN_DIR: &str = "/data/adb/meta-hybrid/run/";
pub const STATE_FILE: &str = "/data/adb/meta-hybrid/run/daemon_state.json";
pub const DAEMON_LOG_FILE: &str = "/data/adb/meta-hybrid/daemon.log";
pub const DISABLE_FILE_NAME: &str = "disable";
pub const REMOVE_FILE_NAME: &str = "remove";
pub const SKIP_MOUNT_FILE_NAME: &str = "skip_mount";
pub const SYSTEM_RW_DIR: &str = "/data/adb/meta-hybrid/rw";
pub const MODULE_PROP_FILE: &str = "/data/adb/modules/meta-hybrid/module.prop";
pub const BUILTIN_PARTITIONS: &[&str] = &[
    "system",
    "vendor",
    "product",
    "system_ext",
    "odm",
    "oem",
    "apex",
];
pub const REPLACE_DIR_FILE_NAME: &str = ".replace";
pub const REPLACE_DIR_XATTR: &str = "trusted.overlay.opaque";
pub const TMPFS_CANDIDATES: &[&str] = &["/debug_ramdisk", "/patch_hw", "/oem", "/root", "/sbin"];
