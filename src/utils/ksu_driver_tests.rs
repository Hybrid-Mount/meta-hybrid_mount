// SPDX-License-Identifier: GPL-3.0-only

use super::*;
use std::os::unix::fs::symlink;

#[test]
fn inherited_su_session_descriptor_takes_priority() {
    let root = crate::test_support::Fixture::new("ksu-fds");
    symlink("anon_inode:[ksu_driver]", root.join("4")).unwrap();
    symlink("anon_inode:[ksu_driver_su]", root.join("8")).unwrap();
    symlink("anon_inode:[ksu_driver_su]", root.join("not-a-fd")).unwrap();
    assert_eq!(scan_driver_fd(&root).unwrap(), Some(8));
    fs::remove_file(root.join("8")).unwrap();
    assert_eq!(scan_driver_fd(&root).unwrap(), Some(4));
}

#[test]
fn scanning_ignores_unrelated_broken_and_negative_descriptors() {
    let root = crate::test_support::Fixture::new("ksu-no-fds");
    symlink("/dev/null", root.join("4")).unwrap();
    symlink("anon_inode:[ksu_driver]", root.join("-1")).unwrap();
    fs::write(root.join("8"), "not a symlink").unwrap();
    assert_eq!(scan_driver_fd(&root).unwrap(), None);
    assert!(scan_driver_fd(&root.join("missing")).is_err());
}

#[test]
fn duplicate_is_owned_without_closing_or_modifying_inherited_fd() {
    let original = fs::File::open("/dev/null").unwrap();
    let duplicate = duplicate_driver(original.as_raw_fd()).unwrap();
    assert_ne!(duplicate.as_raw_fd(), original.as_raw_fd());
    // SAFETY: both descriptors are live during these fcntl calls.
    unsafe {
        assert_ne!(
            libc::fcntl(duplicate.as_raw_fd(), libc::F_GETFD) & libc::FD_CLOEXEC,
            0
        );
    }
    drop(duplicate);
    assert!(original.metadata().is_ok());
    assert!(duplicate_driver(-1).is_err());
}

#[test]
fn failed_discovery_is_retryable_and_success_is_cached() {
    let cache = Mutex::new(None);
    assert!(
        with_driver::<()>(
            &cache,
            || Err(io::ErrorKind::NotFound.into()),
            |_| panic!("no fd")
        )
        .is_err()
    );
    assert!(cache.lock().unwrap().is_none());
    let first = with_driver(
        &cache,
        || Ok(fs::File::open("/dev/null")?.into()),
        |fd| Ok(fd.as_raw_fd()),
    )
    .unwrap();
    let second = with_driver(
        &cache,
        || panic!("must reuse cached fd"),
        |fd| Ok(fd.as_raw_fd()),
    )
    .unwrap();
    assert_eq!(first, second);
    let err = with_driver(
        &cache,
        || panic!("must reuse cached fd"),
        |_| Err::<(), _>(io::ErrorKind::PermissionDenied.into()),
    )
    .unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
}

#[test]
fn request_constants_match_platform_ioctl_encoding() {
    use super::ksu_protocol::*;
    let k = u32::from(b'K');
    assert_eq!(GET_INFO, libc::_IOR::<GetInfo>(k, 2) as u32);
    assert_eq!(GET_INFO_LEGACY, libc::_IOR::<()>(k, 2) as u32);
    assert_eq!(NUKE_EXT4_SYSFS, libc::_IOW::<()>(k, 17) as u32);
    assert_eq!(MANAGE_TRY_UMOUNT, libc::_IOW::<()>(k, 18) as u32);
    assert_eq!(ENABLE_KPM, libc::_IOR::<()>(k, 102) as u32);
}
