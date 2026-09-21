// SPDX-License-Identifier: GPL-3.0-only

//! ext4 sysfs concealment for the staging image.
//!
//! KernelSU installations use only the supported ioctl and remove the LKM
//! assets during installation. On APatch and other non-KSU environments, a
//! GPL-2.0-only compatibility LKM is selected from the module package. LKM
//! failures are best-effort: a mounted staging filesystem must not be rolled
//! back merely because concealment is unavailable.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use ::ksu::NukeExt4Sysfs;

use crate::defs;
use crate::errors::{ContextError, Error};
use crate::sys::lkm::{
    LoadAttemptGuard, describe_attempts, load_with_candidates, select_bundled_lkm_path,
};
use crate::utils::ksu;
use crate::vfs::lkm_target::{android_major_from_kernel_release, kernel_major_minor};

const KALLSYMS_PATH: &str = "/proc/kallsyms";
const KPTR_RESTRICT_PATH: &str = "/proc/sys/kernel/kptr_restrict";
const LKM_OVERRIDE_ENV: &str = "HYBRID_MOUNT_LKM_PATH";
/// A single LKM insmod has a known, bounded worst case.
const INSMOD_TIMEOUT: Duration = Duration::from_secs(30);

/// Conceal the ext4 staging superblock from `/proc/fs/ext4`.
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
    let params = vec![
        format!("mount_point={}", path.display()),
        format!("symaddr=0x{symbol}"),
    ];
    let _attempt_guard = LoadAttemptGuard::arm(
        Path::new(defs::LKM_BOOT_GUARD_PATH),
        "LKM",
        &format!("lkm={} mount={}", lkm_path.display(), path.display()),
    )?;

    // The node disappearing is the only success signal: the LKM deliberately returns
    // -EAGAIN from module_init so it is not retained, which makes a non-zero insmod
    // status an expected part of the successful path.
    match load_with_candidates(
        crate::sys::lkm::INSMOD_CANDIDATES.iter().copied(),
        &lkm_path,
        "load LKM for ext4 sysfs nuke",
        &params,
        INSMOD_TIMEOUT,
        || !procfs_node.exists(),
    ) {
        Ok(()) => Ok(()),
        Err(attempts) => Err(format!(
            "LKM did not remove {}; attempts: {}",
            procfs_node.display(),
            describe_attempts(&attempts)
        )),
    }
}

/// Observe the current mounted ext4 result without loading an LKM or issuing a nuke ioctl.
/// An unreadable/missing procfs root or an absent staging mount cannot prove support.
pub fn concealment_status(path: &Path) -> Option<bool> {
    let node = ext4_procfs_node(path).ok()?;
    concealed_node_status(&node)
}

fn concealed_node_status(node: &Path) -> Option<bool> {
    // Enumerating the parent distinguishes a removed node from unavailable procfs.
    let name = node.file_name()?;
    let entries = fs::read_dir(node.parent()?).ok()?;
    for entry in entries {
        if entry.ok()?.file_name() == name {
            return Some(false);
        }
    }
    Some(true)
}

fn ext4_procfs_node(path: &Path) -> Result<PathBuf, String> {
    let entry = crate::sys::mountinfo::mount_entry_at(path)
        .map_err(|err| format!("read /proc/self/mountinfo: {err}"))?
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
    select_bundled_lkm_path(
        defs::MODULE_LKM_DIR,
        LKM_OVERRIDE_ENV,
        "LKM",
        select_lkm_filename,
    )
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
    fn concealment_observation_distinguishes_hidden_visible_and_unavailable_procfs() {
        let root = crate::test_support::Fixture::new("nuke-status");
        let procfs = root.join("ext4");
        let node = procfs.join("loop7");
        assert_eq!(concealed_node_status(&node), None);
        fs::create_dir(&procfs).unwrap();
        fs::create_dir(procfs.join("loop8")).unwrap();
        assert_eq!(concealed_node_status(&node), Some(true));
        fs::create_dir(&node).unwrap();
        assert_eq!(concealed_node_status(&node), Some(false));
        assert!(node.is_dir(), "querying support must not hide anything");
    }

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
        assert!(crate::sys::lkm::bundled_lkm_arch_supported("aarch64"));
        assert!(!crate::sys::lkm::bundled_lkm_arch_supported("arm"));
        assert!(!crate::sys::lkm::bundled_lkm_arch_supported("x86_64"));
    }

    /// Every filename the selector can return must be one the package actually ships.
    /// A stale arm here would silently lose ext4 sysfs concealment on APatch.
    #[test]
    fn selection_matrix_matches_the_shipped_manifest() {
        let manifest = include_str!("../../module/lkm/binaries/list.txt");
        let shipped: Vec<&str> = manifest
            .lines()
            .filter_map(|line| line.split_whitespace().nth(1))
            .collect();

        let selectable = [
            ("4.14.336-gki", None),
            ("5.10.198-android12-9", None),
            ("5.10.198-android13-8", None),
            ("5.15.137-android13-11", None),
            ("5.15.149-android14-12", None),
            ("6.1.75-android14-13", None),
            ("6.6.30-android15-8", None),
            ("6.12.30-android16-6", None),
        ];

        let mut selected: Vec<&str> = selectable
            .iter()
            .map(|(release, android)| {
                select_lkm_filename(release, *android)
                    .unwrap_or_else(|| panic!("{release} is not selectable"))
            })
            .collect();
        selected.sort_unstable();

        let mut expected = shipped.clone();
        expected.sort_unstable();

        assert_eq!(
            selected, expected,
            "the selector matrix and module/lkm/binaries/list.txt disagree"
        );
    }

    /// The nuke module is not retained (`module_init` returns -EAGAIN), so the object
    /// name must keep matching what the Makefile builds.
    #[test]
    fn the_module_object_name_is_stable() {
        let makefile = include_str!("../../module/lkm/src/Makefile");
        assert!(
            makefile.contains("obj-m += nuke.o"),
            "the nuke Makefile no longer builds nuke.o"
        );
    }
}
