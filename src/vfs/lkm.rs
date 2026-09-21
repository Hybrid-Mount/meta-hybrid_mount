// SPDX-License-Identifier: GPL-3.0-only

//! Loads Hybrid Mount's own VFS kernel module.
//!
//! The DDK builds one hybridmount-<android>-<kernel>.ko per Android/GKI target (see
//! .github/workflows/kernel-module.yml). Selection matches the kernel release and
//! GKI label first, then other builds for the same kernel line. A boot guard is written
//! before loading; a supported key-type response decides success. Try ksud first,
//! then the built-in compatibility loader, then ordinary insmod.
//!
//! A failed load is not fatal: the caller probes again and degrades or reports
//! according to vfs_strict, the same path taken when the module is absent entirely.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::defs;
use crate::errors::Result;
use crate::sys::lkm::{
    LoadAttemptGuard, bundled_lkm_arch_supported, describe_attempts, kernel_release,
    load_with_candidates, unload,
};
use crate::vfs::lkm_target::lkm_candidates;

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
    crate::vfs::doctor::ensure_provider_absent(crate::vfs::doctor::presence_on_device())?;
    let candidates = bundled_candidates()?;
    let mut failures = Vec::new();
    for lkm_path in candidates {
        log::info!("trying VFS module candidate: {}", lkm_path.display());
        // A crash leaves the exact failing candidate in the persistent boot guard. Guard
        // failures abort the whole search; they must never become a fallback to another .ko.
        let _attempt = LoadAttemptGuard::arm(
            Path::new(defs::VFS_LKM_BOOT_GUARD_PATH),
            "VFS module load",
            &format!("lkm={}", lkm_path.display()),
        )?;
        match load_with_candidates(
            crate::sys::lkm::INSMOD_CANDIDATES.iter().copied(),
            &lkm_path,
            "load the Hybrid Mount VFS kernel module",
            &[],
            INSMOD_TIMEOUT,
            module_responds,
        ) {
            Ok(()) => return Ok(()),
            Err(attempts) => {
                failures.push(format!(
                    "{}: {}",
                    lkm_path.display(),
                    describe_attempts(&attempts)
                ));
                // Only unload our failed candidate, never a pre-existing or built-in provider.
                if crate::vfs::doctor::presence_on_device()
                    == crate::vfs::doctor::ModulePresence::Loadable
                {
                    unload(
                        MODULE_NAME,
                        "unload failed Hybrid Mount VFS candidate",
                        RMMOD_TIMEOUT,
                    );
                }
                crate::vfs::doctor::ensure_provider_absent(crate::vfs::doctor::presence_on_device())
                    .map_err(|err| format!("stop VFS candidate fallback: {err}; {}", failures.join("; ")))?;
            }
        }
    }
    Err(format!(
        "no VFS candidate became available: {}",
        failures.join("; ")
    ))
}

fn bundled_candidates() -> std::result::Result<Vec<PathBuf>, String> {
    if !bundled_lkm_arch_supported(std::env::consts::ARCH) {
        return Err(format!(
            "bundled VFS modules are aarch64-only, running {}",
            std::env::consts::ARCH
        ));
    }
    if let Some(path) = std::env::var_os(LKM_OVERRIDE_ENV).filter(|path| !path.is_empty()) {
        let path = PathBuf::from(path);
        return if path.is_file() {
            Ok(vec![path])
        } else {
            Err(format!("VFS override module missing: {}", path.display()))
        };
    }
    let release = kernel_release()?;
    let candidates: Vec<_> = lkm_candidates(&release)
        .into_iter()
        .map(|name| Path::new(defs::VFS_LKM_DIR).join(name))
        .filter(|path| path.is_file())
        .collect();
    if candidates.is_empty() {
        return Err(format!("no bundled VFS module for kernel={release}"));
    }
    Ok(candidates)
}

/// Whether the module answers, decided by the key type rather than the insmod exit code.
fn module_responds() -> bool {
    crate::vfs::available()
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
