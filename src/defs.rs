// meta-hybrid_mount/src/defs.rs

// Hybrid Mount Constants

// NOTE: The actual content directory is now determined dynamically at runtime.
pub const FALLBACK_CONTENT_DIR: &str = "/data/adb/meta-hybrid/mnt/";

// The base directory for our own config and logs
pub const BASE_DIR: &str = "/data/adb/meta-hybrid/";

// Runtime state directory (for communicating mount points to CLI)
pub const RUN_DIR: &str = "/data/adb/meta-hybrid/run/";
pub const MOUNT_POINT_FILE: &str = "/data/adb/meta-hybrid/run/mount.point";
// Persist the decided storage mode (tmpfs/ext4) for CLI queries
pub const STORAGE_MODE_FILE: &str = "/data/adb/meta-hybrid/run/storage.mode";

// Log file path (Must match WebUI)
pub const DAEMON_LOG_FILE: &str = "/data/adb/meta-hybrid/daemon.log";

// Markers
pub const DISABLE_FILE_NAME: &str = "disable";
pub const REMOVE_FILE_NAME: &str = "remove";
pub const SKIP_MOUNT_FILE_NAME: &str = "skip_mount";

// OverlayFS Source Name
pub const OVERLAY_SOURCE: &str = "KSU";
pub const KSU_OVERLAY_SOURCE: &str = OVERLAY_SOURCE;

// Path for overlayfs workdir/upperdir (if needed in future)
#[allow(dead_code)]
pub const SYSTEM_RW_DIR: &str = "/data/adb/meta-hybrid/rw";

// LKM Paths
pub const MODULE_LKM_DIR: &str = "/data/adb/modules/meta-hybrid/lkm/binaries";

// Module Prop Path (for dynamic description updates)
pub const MODULE_PROP_FILE: &str = "/data/adb/modules/meta-hybrid/module.prop";