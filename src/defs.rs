pub const MODULES_IMG_FILE: &str = "/data/adb/meta-hybrid/modules.img";
pub const RUN_DIR: &str = "/data/adb/meta-hybrid/run/";
pub const STATE_FILE: &str = "/data/adb/meta-hybrid/run/daemon_state.json";
pub const DISABLE_FILE_NAME: &str = "disable";
pub const REMOVE_FILE_NAME: &str = "remove";
pub const SKIP_MOUNT_FILE_NAME: &str = "skip_mount";
pub const SYSTEM_RW_DIR: &str = "/data/adb/meta-hybrid/rw";
pub const MODULE_PROP_FILE: &str = "/data/adb/modules/meta-hybrid/module.prop";
pub const MODULES_DIR: &str = "/data/adb/modules";
pub const CONFIG_FILE: &str = "/data/adb/meta-hybrid/config.toml";
pub const MKFS_EROFS_PATH: &str = "/data/adb/metamodule/tools/mkfs.erofs";
pub const POACEAE_MOUNT_POINT: &str = "/data/adb/poaceaefs_mount";
pub const ZYGISKSU_DENYLIST_FILE: &str = "/data/adb/zygisksu/denylist_enforce";

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

pub const REPLACE_DIR_FILE_NAME: &str = ".replace";
pub const REPLACE_DIR_XATTR: &str = "trusted.overlay.opaque";
