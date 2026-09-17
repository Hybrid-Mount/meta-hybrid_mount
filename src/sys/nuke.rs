// SPDX-License-Identifier: GPL-3.0-only

//! ext4 sysfs concealment for the staging image.
//!
//! KernelSU installations use only the supported ioctl and remove the LKM
//! assets during installation. On APatch and other non-KSU environments, a
//! GPL-2.0-only compatibility LKM is selected from the module package. LKM
//! failures are best-effort: a mounted staging filesystem must not be rolled
//! back merely because concealment is unavailable.

use std::fs::{self, OpenOptions};
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use ::ksu::NukeExt4Sysfs;
use procfs::process::Process;

use crate::defs;
use crate::errors::{ContextError, Error};
use crate::sys::process::{CaptureMode, CommandSpec, run_command};
use crate::utils::ksu;
use crate::vfs::lkm_target::{
    android_major_from_kernel_release, device_android_major, kernel_major_minor,
};

const KALLSYMS_PATH: &str = "/proc/kallsyms";
const KPTR_RESTRICT_PATH: &str = "/proc/sys/kernel/kptr_restrict";
const KERNEL_RELEASE_PATH: &str = "/proc/sys/kernel/osrelease";
const LKM_OVERRIDE_ENV: &str = "HYBRID_MOUNT_LKM_PATH";
/// A single LKM insmod has a known, bounded worst case.
const INSMOD_TIMEOUT: Duration = Duration::from_secs(30);

/// Conceal the ext4 staging superblock from `/proc/fs/ext4`.
///
pub fn nuke_ext4_sysfs(path: &Path) {
    log::info!("ext4 sysfs nuke start: path={}", path.display());

    if ksu::is_active() {
        run_ksu_nuke(path);
        return;
    }

    log::info!(
        "ext4 sysfs nuke ioctl skipped: path={}, reason=non_ksu, fallback=lkm",
        path.display()
    );

    match try_lkm_nuke(path) {
        Ok(()) => log::info!(
            "ext4 sysfs nuke complete: backend=lkm, path={}",
            path.display()
        ),
        Err(err) => log::warn!(
            "ext4 sysfs nuke failed: backend=lkm, path={}, error={err}",
            path.display()
        ),
    }
}

fn run_ksu_nuke(path: &Path) {
    let mut nuke = NukeExt4Sysfs::new();
    nuke.add(path);
    match nuke.execute() {
        Ok(()) => {
            log::info!(
                "ext4 sysfs nuke complete: backend=ksu_ioctl, path={}",
                path.display()
            );
        }
        Err(err) => {
            log::warn!(
                "ext4 sysfs nuke ioctl failed: path={}, fallback=none, error={err}",
                path.display()
            );
        }
    }
}

fn try_lkm_nuke(path: &Path) -> crate::errors::Result<()> {
    try_lkm_nuke_inner(path).map_err(|message| {
        Error::Lkm(Box::new(ContextError::new(
            "LKM ext4 sysfs nuke",
            Some(path.to_path_buf()),
            message,
        )))
    })
}

fn try_lkm_nuke_inner(path: &Path) -> Result<(), String> {
    let procfs_node = ext4_procfs_node(path)?;
    if !procfs_node.exists() {
        log::info!(
            "ext4 sysfs node already absent: path={}",
            procfs_node.display()
        );
        return Ok(());
    }

    let lkm_path = select_lkm_path()?;
    if !lkm_path.is_file() {
        return Err(format!("selected LKM is missing: {}", lkm_path.display()));
    }

    let symbol = readable_symbol_address().ok_or_else(|| {
        "ext4_unregister_sysfs has no readable non-zero address in /proc/kallsyms".to_owned()
    })?;
    let mount_parameter = format!("mount_point={}", path.display());
    let symbol_parameter = format!("symaddr=0x{symbol}");
    let _attempt_guard = LkmAttemptGuard::arm(&lkm_path, path)?;

    let candidates: [(&str, Option<&str>); 4] = [
        ("/system/bin/insmod", None),
        ("/data/adb/ap/bin/busybox", Some("insmod")),
        ("/data/adb/ksu/bin/busybox", Some("insmod")),
        ("insmod", None),
    ];
    let mut attempts = Vec::new();

    for (program, applet) in candidates {
        let mut args = Vec::new();
        if let Some(applet) = applet {
            args.push(applet.to_owned());
        }
        args.push(lkm_path.display().to_string());
        args.push(mount_parameter.clone());
        args.push(symbol_parameter.clone());

        let spec = CommandSpec::new(program)
            .operation("load LKM for ext4 sysfs nuke")
            .args(args)
            .capture(CaptureMode::Stderr)
            // The LKM deliberately returns -EAGAIN from module_init so it
            // is not retained. A non-zero insmod status can therefore be
            // the expected successful path; disappearance is authoritative.
            .any_exit_status()
            .timeout(INSMOD_TIMEOUT);

        match run_command(&spec) {
            Ok(outcome) => {
                if !procfs_node.exists() {
                    return Ok(());
                }
                let stderr = outcome.stderr_text().unwrap_or_default();
                return Err(format!(
                    "{program} executed once but did not remove {}: status={}, stderr={}; unavailable candidates: {}",
                    procfs_node.display(),
                    outcome.status,
                    stderr.trim(),
                    attempts.join("; ")
                ));
            }
            Err(err) => attempts.push(format!("{program}: {err}")),
        }
    }

    Err(format!(
        "LKM did not remove {}; attempts: {}",
        procfs_node.display(),
        attempts.join("; ")
    ))
}

#[derive(Debug)]
struct LkmAttemptGuard {
    marker_path: PathBuf,
}

impl LkmAttemptGuard {
    fn arm(lkm_path: &Path, mount_path: &Path) -> Result<Self, String> {
        Self::arm_at(Path::new(defs::LKM_BOOT_GUARD_PATH), lkm_path, mount_path)
    }

    fn arm_at(marker_path: &Path, lkm_path: &Path, mount_path: &Path) -> Result<Self, String> {
        Self::arm_at_with_sync(
            marker_path,
            lkm_path,
            mount_path,
            crate::sys::fs::sync_parent_directory,
        )
    }

    fn arm_at_with_sync(
        marker_path: &Path,
        lkm_path: &Path,
        mount_path: &Path,
        mut sync_parent: impl FnMut(&Path) -> io::Result<()>,
    ) -> Result<Self, String> {
        if let Some(parent) = marker_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(|err| {
                format!(
                    "create LKM boot-guard directory {}: {err}",
                    parent.display()
                )
            })?;
            sync_parent(parent).map_err(|err| {
                format!(
                    "sync LKM boot-guard directory entry {}: {err}",
                    parent.display()
                )
            })?;
        }

        let mut marker = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(marker_path)
            .map_err(|err| {
                if err.kind() == ErrorKind::AlreadyExists {
                    format!(
                        "previous LKM attempt did not complete; refusing automatic retry. Verify the kernel ABI, then remove {} to retry",
                        marker_path.display()
                    )
                } else {
                    format!("create LKM boot guard {}: {err}", marker_path.display())
                }
            })?;
        let guard = Self {
            marker_path: marker_path.to_path_buf(),
        };
        if let Err(err) = writeln!(
            marker,
            "lkm={} mount={}",
            lkm_path.display(),
            mount_path.display()
        ) {
            drop(guard);
            return Err(format!(
                "write LKM boot guard {}: {err}",
                marker_path.display()
            ));
        }
        if let Err(err) = marker.sync_all() {
            drop(guard);
            return Err(format!(
                "sync LKM boot guard {}: {err}",
                marker_path.display()
            ));
        }
        if let Err(err) = sync_parent(marker_path) {
            drop(guard);
            return Err(format!(
                "sync LKM boot-guard parent for {}: {err}",
                marker_path.display()
            ));
        }

        Ok(guard)
    }
}

impl Drop for LkmAttemptGuard {
    fn drop(&mut self) {
        match fs::remove_file(&self.marker_path) {
            Ok(()) => {
                if let Err(err) = crate::sys::fs::sync_parent_directory(&self.marker_path) {
                    log::warn!(
                        "failed to persist LKM boot guard removal {}: {err}",
                        self.marker_path.display()
                    );
                }
            }
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            Err(err) => log::warn!(
                "failed to clear LKM boot guard {}: {err}",
                self.marker_path.display()
            ),
        }
    }
}

fn ext4_procfs_node(path: &Path) -> Result<PathBuf, String> {
    let process = Process::myself().map_err(|err| format!("read current process: {err}"))?;
    let mountinfo = process
        .mountinfo()
        .map_err(|err| format!("read /proc/self/mountinfo: {err}"))?;
    let entry = mountinfo
        .into_iter()
        .find(|entry| entry.mount_point == path)
        .ok_or_else(|| format!("mount point not found in mountinfo: {}", path.display()))?;

    if entry.fs_type != "ext4" {
        return Err(format!(
            "mount point is {}, not ext4: {}",
            entry.fs_type,
            path.display()
        ));
    }

    let source = entry
        .mount_source
        .ok_or_else(|| format!("ext4 mount has no source: {}", path.display()))?;
    let device = Path::new(&source)
        .file_name()
        .ok_or_else(|| format!("cannot derive ext4 device name from {source}"))?;
    Ok(Path::new("/proc/fs/ext4").join(device))
}

fn select_lkm_path() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os(LKM_OVERRIDE_ENV).filter(|path| !path.is_empty()) {
        return Ok(PathBuf::from(path));
    }

    if !is_bundled_lkm_arch_supported(std::env::consts::ARCH) {
        return Err(format!(
            "bundled LKM is aarch64-only, running architecture is {}",
            std::env::consts::ARCH
        ));
    }

    let release = fs::read_to_string(KERNEL_RELEASE_PATH)
        .map_err(|err| format!("read kernel release: {err}"))?;
    let android_major = android_major_from_kernel_release(&release).or_else(device_android_major);
    let file_name = select_lkm_filename(release.trim(), android_major).ok_or_else(|| {
        format!(
            "no bundled LKM for kernel={} android={}",
            release.trim(),
            android_major
                .map(|value| value.to_string())
                .unwrap_or_else(|| "unknown".to_owned())
        )
    })?;

    Ok(Path::new(defs::MODULE_LKM_DIR).join(file_name))
}

fn is_bundled_lkm_arch_supported(arch: &str) -> bool {
    arch == "aarch64"
}

fn readable_symbol_address() -> Option<String> {
    if let Some(address) = find_symbol_address() {
        return Some(address);
    }

    let _guard = KptrRestrictGuard::temporarily_set_one().ok()?;
    find_symbol_address()
}

fn find_symbol_address() -> Option<String> {
    let text = fs::read_to_string(KALLSYMS_PATH).ok()?;
    text.lines().find_map(|line| {
        let mut fields = line.split_whitespace();
        let address = fields.next()?;
        let _kind = fields.next()?;
        let name = fields.next()?;
        (name == "ext4_unregister_sysfs"
            && address.len() <= 16
            && address.chars().all(|ch| ch.is_ascii_hexdigit())
            && address.chars().any(|ch| ch != '0'))
        .then(|| address.to_owned())
    })
}

struct KptrRestrictGuard {
    original: String,
}

impl KptrRestrictGuard {
    fn temporarily_set_one() -> std::io::Result<Self> {
        let original = fs::read_to_string(KPTR_RESTRICT_PATH)?;
        fs::write(KPTR_RESTRICT_PATH, "1\n")?;
        Ok(Self { original })
    }
}

impl Drop for KptrRestrictGuard {
    fn drop(&mut self) {
        if let Err(err) = fs::write(KPTR_RESTRICT_PATH, &self.original) {
            log::warn!("failed to restore kptr_restrict: {err}");
        }
    }
}

fn select_lkm_filename(release: &str, android_major: Option<u32>) -> Option<&'static str> {
    let android = android_major.or_else(|| android_major_from_kernel_release(release));
    match (kernel_major_minor(release)?, android) {
        ((4, 14), _) => Some("nuke-android-4.14.ko"),
        ((5, 10), Some(12)) => Some("nuke-android12-5.10.ko"),
        ((5, 10), Some(13)) => Some("nuke-android13-5.10.ko"),
        ((5, 15), Some(13)) => Some("nuke-android13-5.15.ko"),
        ((5, 15), Some(14)) => Some("nuke-android14-5.15.ko"),
        ((6, 1), Some(14)) => Some("nuke-android14-6.1.ko"),
        ((6, 6), Some(15)) => Some("nuke-android15-6.6.ko"),
        ((6, 12), Some(16)) => Some("nuke-android16-6.12.ko"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_bundled_kernel_android_matrix() {
        assert_eq!(
            select_lkm_filename("5.10.198-android12-9", None),
            Some("nuke-android12-5.10.ko")
        );
        assert_eq!(
            select_lkm_filename("5.10.209-gki", Some(13)),
            Some("nuke-android13-5.10.ko")
        );
        assert_eq!(
            select_lkm_filename("5.15.153-android13", None),
            Some("nuke-android13-5.15.ko")
        );
        assert_eq!(
            select_lkm_filename("6.6.30-android15", None),
            Some("nuke-android15-6.6.ko")
        );
    }

    #[test]
    fn refuses_unknown_or_unsafe_matrix_entries() {
        assert_eq!(select_lkm_filename("5.15.153", None), None);
        assert_eq!(select_lkm_filename("5.10.209-gki", Some(14)), None);
        assert_eq!(select_lkm_filename("6.1.0-android13", None), None);
        assert_eq!(select_lkm_filename("6.8.0-android16", None), None);
    }

    #[test]
    fn parses_android_versions_without_assuming_semver() {
        assert_eq!(crate::vfs::lkm_target::parse_android_major("14"), Some(14));
        assert_eq!(
            crate::vfs::lkm_target::parse_android_major("15.0.0"),
            Some(15)
        );
        assert_eq!(
            android_major_from_kernel_release("6.6.1-android15-8"),
            Some(15)
        );
    }

    #[test]
    fn bundled_lkm_architecture_contract_is_explicit() {
        assert!(is_bundled_lkm_arch_supported("aarch64"));
        assert!(!is_bundled_lkm_arch_supported("arm"));
        assert!(!is_bundled_lkm_arch_supported("x86_64"));
    }

    #[test]
    fn lkm_attempt_guard_refuses_stale_marker_and_clears_on_drop() {
        let root =
            std::env::temp_dir().join(format!("hybrid-mount-lkm-guard-{}", std::process::id()));
        let marker = root.join("guard");
        let guard = LkmAttemptGuard::arm_at(
            &marker,
            Path::new("/module/nuke.ko"),
            Path::new("/mnt/staging"),
        )
        .unwrap();

        assert!(marker.is_file());
        assert!(
            LkmAttemptGuard::arm_at(
                &marker,
                Path::new("/module/nuke.ko"),
                Path::new("/mnt/staging")
            )
            .unwrap_err()
            .contains("refusing automatic retry")
        );

        drop(guard);
        assert!(!marker.exists());
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn lkm_attempt_guard_aborts_when_marker_parent_sync_fails() {
        use std::cell::Cell;

        let root = std::env::temp_dir().join(format!(
            "hybrid-mount-lkm-guard-sync-fail-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let marker = root.join("guard");
        let sync_calls = Cell::new(0);
        let marker_was_visible = Cell::new(false);

        let err = LkmAttemptGuard::arm_at_with_sync(
            &marker,
            Path::new("/module/nuke.ko"),
            Path::new("/mnt/staging"),
            |path| {
                let call = sync_calls.get() + 1;
                sync_calls.set(call);
                if call == 2 {
                    marker_was_visible.set(path == marker && marker.is_file());
                    return Err(io::Error::other("injected marker parent sync failure"));
                }
                Ok(())
            },
        )
        .unwrap_err();

        assert!(
            sync_calls.get() == 2
                && marker_was_visible.get()
                && !marker.exists()
                && err.contains("sync LKM boot-guard parent"),
            "{err}"
        );
        fs::remove_dir(root).unwrap();
    }
}
