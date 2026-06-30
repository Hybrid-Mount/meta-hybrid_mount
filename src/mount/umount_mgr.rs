// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::path::Path;
#[cfg(any(target_os = "linux", target_os = "android"))]
use std::{
    collections::HashSet,
    sync::{LazyLock, Mutex},
};

use anyhow::Result;
#[cfg(any(target_os = "linux", target_os = "android"))]
use ksu::{TryUmount, TryUmountFlags};
#[cfg(any(target_os = "linux", target_os = "android"))]
use rustix::path::Arg;

#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::defs;

#[cfg(any(target_os = "linux", target_os = "android"))]
pub static LIST: LazyLock<Mutex<TryUmount>> = LazyLock::new(|| Mutex::new(TryUmount::new()));
#[cfg(any(target_os = "linux", target_os = "android"))]
static HISTORY: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

#[cfg(any(target_os = "linux", target_os = "android"))]
fn is_ignored_partition(path: &str) -> bool {
    // pairip-protected APKs (Play Integrity, etc.) verify native library backing
    // after zygote forks; if KSU detaches our overlay over /system/lib*,
    // /vendor/lib* in the app's namespace mid-flight, those checks crash with
    // SIGSEGV in libpairipcore.so. Keep the overlay visible in the app namespace
    // for these paths and rely on Kasumi/sus_mount to handle hiding instead.
    defs::IGNORE_UNMOUNT_PARTITIONS.iter().any(|ignored| {
        let ignored = ignored.trim_end_matches('/');
        path == ignored
            || path
                .strip_prefix(ignored)
                .is_some_and(|rest| rest.starts_with('/'))
    })
}

pub fn send_umountable<P>(target: P) -> Result<()>
where
    P: AsRef<Path>,
{
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        let _ = target;
        Ok(())
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        if !crate::utils::KSU.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(());
        }

        let target = target.as_ref();
        let path = target.as_str()?;

        if is_ignored_partition(path) {
            crate::scoped_log!(
                debug,
                "umount",
                "skip: path={}, reason=ignore_unmount_partition",
                path
            );
            return Ok(());
        }

        let mut history = HISTORY
            .lock()
            .map_err(|_| anyhow::anyhow!("Failed to lock history mutex"))?;

        if !history.insert(path.to_string()) {
            return Ok(());
        }

        LIST.lock()
            .map_err(|_| anyhow::anyhow!("Failed to lock umount list"))?
            .add(target);
        Ok(())
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn commit() -> Result<()> {
    if !crate::utils::KSU.load(std::sync::atomic::Ordering::Relaxed) {
        return Ok(());
    }
    let mut list = LIST
        .lock()
        .map_err(|_| anyhow::anyhow!("Failed to lock umount list"))?;

    list.format_msg(|p| format!("{p:?} umount successful "));
    list.flags(TryUmountFlags::MNT_DETACH);
    if let Err(e2) = list.umount() {
        crate::scoped_log!(warn, "umount", "commit failed: {:#}", e2);
    }

    Ok(())
}

#[cfg(test)]
#[cfg(any(target_os = "linux", target_os = "android"))]
mod tests {
    use super::is_ignored_partition;

    #[test]
    fn skips_exact_ignored_partition() {
        assert!(is_ignored_partition("/system/lib"));
        assert!(is_ignored_partition("/system/lib64"));
        assert!(is_ignored_partition("/vendor/lib"));
        assert!(is_ignored_partition("/vendor/lib64"));
    }

    #[test]
    fn skips_descendants_of_ignored_partition() {
        assert!(is_ignored_partition("/system/lib64/foo"));
        assert!(is_ignored_partition("/vendor/lib/arm/libfoo.so"));
    }

    #[test]
    fn does_not_skip_siblings_with_shared_prefix() {
        // /system/lib should not match /system/lib_extra
        assert!(!is_ignored_partition("/system/lib_extra"));
        assert!(!is_ignored_partition("/system/lib64_other"));
        assert!(!is_ignored_partition("/vendor/library"));
    }

    #[test]
    fn does_not_skip_unrelated_paths() {
        assert!(!is_ignored_partition("/product"));
        assert!(!is_ignored_partition("/system/etc"));
        assert!(!is_ignored_partition(
            "/data/adb/hybrid-mount/run/staging_x"
        ));
    }
}
