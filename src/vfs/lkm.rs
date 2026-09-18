// SPDX-License-Identifier: GPL-3.0-only

//! Loads Hybrid Mount's own VFS kernel module.
//!
//! The DDK builds one hybridmount-<android>-<kernel>.ko per Android/GKI target (see
//! .github/workflows/kernel-module.yml). Selection matches the kernel release and
//! Android version exactly; a boot guard is written before insmod, and whether the key
//! type answers is what decides success.
//!
//! A failed load is not fatal: the caller probes again and degrades or reports
//! according to vfs_strict, the same path taken when the module is absent entirely.

use std::path::Path;
use std::time::Duration;

use crate::defs;
use crate::errors::Result;
use crate::sys::lkm::{
    LoadAttemptGuard, describe_attempts, load_with_candidates, select_bundled_lkm_path, unload,
};
use crate::vfs::backend::{KeyringKernel, VfsKernel};
use crate::vfs::lkm_target::select_lkm_filename;
use crate::vfs::sys::KeyringChannel;

const MODULE_NAME: &str = "hybridmount";
/// Overrides the selection, for testing a target that is not packaged.
const LKM_OVERRIDE_ENV: &str = "HYBRID_MOUNT_VFS_LKM_PATH";
const INSMOD_TIMEOUT: Duration = Duration::from_secs(30);
const RMMOD_TIMEOUT: Duration = Duration::from_secs(30);

/// Selects, loads and verifies the bundled `hybridmount` module. A failure is logged and left to
/// the caller's degradation policy rather than aborting the boot here.
pub fn load_hm_vfs() -> Result<()> {
    if let Err(err) = load() {
        log::warn!("vfs kernel module was not loaded: {err}");
    }
    Ok(())
}

fn load() -> std::result::Result<(), String> {
    let lkm_path = select_bundled_lkm_path(
        defs::VFS_LKM_DIR,
        LKM_OVERRIDE_ENV,
        "VFS module",
        select_lkm_filename,
    )?;
    if !lkm_path.is_file() {
        return Err(format!(
            "no bundled VFS module for this kernel (expected {})",
            lkm_path.display()
        ));
    }

    let _attempt = LoadAttemptGuard::arm(
        Path::new(defs::VFS_LKM_BOOT_GUARD_PATH),
        "VFS module load",
        &format!("lkm={}", lkm_path.display()),
    )?;

    match load_with_candidates(
        &lkm_path,
        "load the Hybrid Mount VFS kernel module",
        &[],
        INSMOD_TIMEOUT,
        module_responds,
    ) {
        Ok(()) => Ok(()),
        Err(attempts) => {
            // A failed probe can leave a half-initialised module behind.
            unload(
                MODULE_NAME,
                "unload the Hybrid Mount VFS kernel module",
                RMMOD_TIMEOUT,
            );
            Err(format!(
                "{MODULE_NAME} did not become available; attempts: {}",
                describe_attempts(&attempts)
            ))
        }
    }
}

/// Whether the module answers, decided by the key type rather than the insmod exit code.
fn module_responds() -> bool {
    match KeyringKernel::new(KeyringChannel::Hybridmount) {
        Ok(mut kernel) => kernel.version().is_ok(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The module name must match `MODULE_NAME := hybridmount` in the kernel Makefile, since
    /// `rmmod` is keyed on it after a failed probe.
    #[test]
    fn module_name_matches_the_kernel_object() {
        let makefile = include_str!("../../module/vfs/src/Makefile");
        assert!(
            makefile.contains(&format!("MODULE_NAME := {MODULE_NAME}")),
            "the kernel Makefile no longer builds {MODULE_NAME}.o"
        );
    }

    /// The packaged directory must be where the loader looks for the prebuilt modules.
    #[test]
    fn loader_path_matches_the_packaged_location() {
        assert_eq!(
            defs::VFS_LKM_DIR,
            "/data/adb/modules/hybrid_mount/vfs/binaries"
        );
    }
}
