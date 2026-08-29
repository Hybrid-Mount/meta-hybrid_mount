// SPDX-License-Identifier: GPL-3.0-only

//! ext4 sysfs concealment for the staging image.
//!
//! KernelSU installations use only the supported ioctl and remove the LKM
//! assets during installation. On APatch and other non-KSU environments, a
//! GPL-2.0-only compatibility LKM is selected from the module package. LKM
//! failures are best-effort: a mounted staging filesystem must not be rolled
//! back merely because concealment is unavailable.

use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use ::ksu::NukeExt4Sysfs;
use procfs::process::Process;

use crate::defs;
use crate::utils::ksu;

const KALLSYMS_PATH: &str = "/proc/kallsyms";
const KPTR_RESTRICT_PATH: &str = "/proc/sys/kernel/kptr_restrict";
const KERNEL_RELEASE_PATH: &str = "/proc/sys/kernel/osrelease";
const LKM_OVERRIDE_ENV: &str = "HYBRID_MOUNT_LKM_PATH";

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

fn try_lkm_nuke(path: &Path) -> Result<(), String> {
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
        let mut command = Command::new(program);
        if let Some(applet) = applet {
            command.arg(applet);
        }
        command
            .arg(&lkm_path)
            .arg(&mount_parameter)
            .arg(&symbol_parameter);

        match command.output() {
            Ok(output) => {
                // The LKM deliberately returns -EAGAIN from module_init so it
                // is not retained. A non-zero insmod status can therefore be
                // the expected successful path; disappearance is authoritative.
                if !procfs_node.exists() {
                    return Ok(());
                }
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(format!(
                    "{program} executed once but did not remove {}: status={}, stderr={}; unavailable candidates: {}",
                    procfs_node.display(),
                    output.status,
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
        if let Some(parent) = marker_path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                format!(
                    "create LKM boot-guard directory {}: {err}",
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

        Ok(guard)
    }
}

impl Drop for LkmAttemptGuard {
    fn drop(&mut self) {
        match fs::remove_file(&self.marker_path) {
            Ok(()) => {}
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

fn device_android_major() -> Option<u32> {
    for program in ["/system/bin/getprop", "getprop"] {
        let Ok(output) = Command::new(program)
            .arg("ro.build.version.release")
            .output()
        else {
            continue;
        };
        if output.status.success()
            && let Some(major) = parse_android_major(&String::from_utf8_lossy(&output.stdout))
        {
            return Some(major);
        }
    }
    None
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

fn parse_android_major(value: &str) -> Option<u32> {
    value
        .trim()
        .split(|ch: char| !ch.is_ascii_digit())
        .find(|part| !part.is_empty())?
        .parse()
        .ok()
}

fn android_major_from_kernel_release(release: &str) -> Option<u32> {
    let lower = release.to_ascii_lowercase();
    let suffix = lower.split_once("android")?.1;
    parse_android_major(suffix)
}

fn kernel_major_minor(release: &str) -> Option<(u32, u32)> {
    let mut parts = release
        .split(|ch: char| !ch.is_ascii_digit())
        .filter(|part| !part.is_empty());
    Some((parts.next()?.parse().ok()?, parts.next()?.parse().ok()?))
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
        assert_eq!(parse_android_major("14"), Some(14));
        assert_eq!(parse_android_major("15.0.0"), Some(15));
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
}
