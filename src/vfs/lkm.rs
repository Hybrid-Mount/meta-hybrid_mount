// SPDX-License-Identifier: GPL-3.0-only

//! Loads Hybrid Mount's own VFS kernel module (K2).
//!
//! The DDK builds one hybridmount-<android>-<kernel>.ko per Android/GKI target (see
//! .github/workflows/kernel-module.yml). Selection matches the kernel release and
//! Android version exactly; a boot guard is written before insmod, and whether the key
//! type answers is what decides success.
//!
//! A failed load is not fatal: the caller probes again and degrades or reports
//! according to vfs_strict, the same path taken when no K2 is present at all.

use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::defs;
use crate::errors::Result;
use crate::sys::process::{CaptureMode, CommandSpec, run_command};
use crate::vfs::backend::{KeyringKernel, LkmLoader, VfsKernel};
use crate::vfs::lkm_target::{
    android_major_from_kernel_release, device_android_major, select_lkm_filename,
};
use crate::vfs::sys::KeyringChannel;

const KERNEL_RELEASE_PATH: &str = "/proc/sys/kernel/osrelease";
const MODULE_NAME: &str = "hybridmount";
/// Overrides the selection, for testing a target that is not packaged.
const LKM_OVERRIDE_ENV: &str = "HYBRID_MOUNT_VFS_LKM_PATH";
const INSMOD_TIMEOUT: Duration = Duration::from_secs(30);
const RMMOD_TIMEOUT: Duration = Duration::from_secs(30);

/// Production loader: selects, loads and verifies the bundled K2 module.
pub struct VfsLkmLoader;

impl LkmLoader for VfsLkmLoader {
    fn load_hm_vfs(&self) -> Result<()> {
        if let Err(err) = load() {
            log::warn!("vfs kernel module was not loaded: {err}");
        }
        Ok(())
    }
}

fn load() -> std::result::Result<(), String> {
    let lkm_path = select_lkm_path()?;
    if !lkm_path.is_file() {
        return Err(format!(
            "no bundled VFS module for this kernel (expected {})",
            lkm_path.display()
        ));
    }

    let _attempt = LkmAttemptGuard::arm(&lkm_path)?;
    let mut attempts = Vec::new();
    for (program, applet) in insmod_candidates() {
        let mut args = Vec::new();
        if let Some(applet) = applet {
            args.push(applet.to_owned());
        }
        args.push(lkm_path.display().to_string());

        let spec = CommandSpec::new(program)
            .operation("load the Hybrid Mount VFS kernel module")
            .args(args)
            .capture(CaptureMode::Stderr)
            .timeout(INSMOD_TIMEOUT);

        match run_command(&spec) {
            Ok(outcome) => {
                // The insmod exit code is not authoritative: the module may already
                // be loaded from an earlier boot.
                if module_responds() {
                    return Ok(());
                }
                let stderr = outcome.stderr_text().unwrap_or_default();
                attempts.push(format!(
                    "{program}: status={}, stderr={}",
                    outcome.status,
                    stderr.trim()
                ));
            }
            Err(err) => attempts.push(format!("{program}: {err}")),
        }
    }

    // A failed probe can leave a half-initialised module behind.
    unload();
    Err(format!(
        "{MODULE_NAME} did not become available; attempts: {}",
        attempts.join("; ")
    ))
}

fn insmod_candidates() -> [(&'static str, Option<&'static str>); 4] {
    [
        ("/system/bin/insmod", None),
        ("/data/adb/ap/bin/busybox", Some("insmod")),
        ("/data/adb/ksu/bin/busybox", Some("insmod")),
        ("insmod", None),
    ]
}

fn rmmod_candidates() -> [(&'static str, Option<&'static str>); 4] {
    [
        ("/system/bin/rmmod", None),
        ("/data/adb/ap/bin/busybox", Some("rmmod")),
        ("/data/adb/ksu/bin/busybox", Some("rmmod")),
        ("rmmod", None),
    ]
}

/// Whether K2 answers, decided by the key type rather than the insmod exit code.
fn module_responds() -> bool {
    match KeyringKernel::new(KeyringChannel::Hybridmount) {
        Ok(mut kernel) => kernel.version().is_ok(),
        Err(_) => false,
    }
}

fn unload() {
    for (program, applet) in rmmod_candidates() {
        let mut args = Vec::new();
        if let Some(applet) = applet {
            args.push(applet.to_owned());
        }
        args.push(MODULE_NAME.to_owned());

        let spec = CommandSpec::new(program)
            .operation("unload the Hybrid Mount VFS kernel module")
            .args(args)
            .capture(CaptureMode::Stderr)
            .any_exit_status()
            .timeout(RMMOD_TIMEOUT);

        if run_command(&spec).is_ok() {
            return;
        }
    }
}

fn select_lkm_path() -> std::result::Result<PathBuf, String> {
    if let Some(path) = std::env::var_os(LKM_OVERRIDE_ENV).filter(|path| !path.is_empty()) {
        return Ok(PathBuf::from(path));
    }

    if std::env::consts::ARCH != "aarch64" {
        return Err(format!(
            "the bundled VFS module is aarch64-only, running architecture is {}",
            std::env::consts::ARCH
        ));
    }

    let release = fs::read_to_string(KERNEL_RELEASE_PATH)
        .map_err(|err| format!("read kernel release: {err}"))?;
    let android_major = android_major_from_kernel_release(&release).or_else(device_android_major);
    let file_name = select_lkm_filename(release.trim(), android_major).ok_or_else(|| {
        format!(
            "no bundled VFS module for kernel={} android={}",
            release.trim(),
            android_major
                .map(|value| value.to_string())
                .unwrap_or_else(|| "unknown".to_owned())
        )
    })?;

    Ok(Path::new(defs::VFS_LKM_DIR).join(file_name))
}

/// Boot guard written before loading: cleared on Drop, left behind by a hard crash so
/// the next boot refuses to retry automatically.
#[derive(Debug)]
struct LkmAttemptGuard {
    marker_path: PathBuf,
}

impl LkmAttemptGuard {
    fn arm(lkm_path: &Path) -> std::result::Result<Self, String> {
        let marker_path = Path::new(defs::VFS_LKM_BOOT_GUARD_PATH);
        if let Some(parent) = marker_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)
                .map_err(|err| format!("create VFS LKM guard directory: {err}"))?;
        }

        let mut marker = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(marker_path)
            .map_err(|err| {
                if err.kind() == ErrorKind::AlreadyExists {
                    format!(
                        "previous VFS module load did not complete; refusing automatic retry. Verify the kernel ABI, then remove {}",
                        marker_path.display()
                    )
                } else {
                    format!("create VFS LKM boot guard: {err}")
                }
            })?;

        let guard = Self {
            marker_path: marker_path.to_path_buf(),
        };
        writeln!(marker, "lkm={}", lkm_path.display())
            .map_err(|err| format!("write VFS LKM boot guard: {err}"))?;
        marker
            .sync_all()
            .map_err(|err| format!("sync VFS LKM boot guard: {err}"))?;
        Ok(guard)
    }
}

impl Drop for LkmAttemptGuard {
    fn drop(&mut self) {
        if let Err(err) = fs::remove_file(&self.marker_path) {
            log::warn!("failed to clear the VFS LKM boot guard: {err}");
        }
    }
}
