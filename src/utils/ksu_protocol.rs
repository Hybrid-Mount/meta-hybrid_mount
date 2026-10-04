// Copyright (C) 2026 meta-magic_mount-rs developers
// SPDX-License-Identifier: GPL-3.0-only

//! KernelSU UAPI buffers and fallbacks, independent of driver discovery.
//! Adapted from meta-magic_mount-rs 9d401ef41570a191ad3732328679c88570f42467.

use std::ffi::CString;
use std::io;

// Linux/Android asm-generic ioctl encoding, shared by every supported target.
pub(super) const GET_INFO: u32 = 0x8010_4b02; // _IOR('K', 2, GetInfo)
pub(super) const GET_INFO_LEGACY: u32 = 0x8000_4b02; // _IOR('K', 2, ())
pub(super) const NUKE_EXT4_SYSFS: u32 = 0x4000_4b11; // _IOW('K', 17, ())
pub(super) const MANAGE_TRY_UMOUNT: u32 = 0x4000_4b12; // _IOW('K', 18, ())
pub(super) const ENABLE_KPM: u32 = 0x8000_4b66; // _IOR('K', 102, ())

#[repr(C)]
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct GetInfo {
    pub version: u32,
    pub flags: u32,
    pub features: u32,
    pub uapi_version: u32,
}

impl GetInfo {
    pub fn late_load(&self) -> bool {
        self.flags & (1 << 2) != 0
    }
}

#[repr(C)]
#[derive(Default)]
pub(super) struct EnableKpm {
    pub enabled: u8,
}

// UAPI uses __aligned_u64, including on armv7.
#[repr(C, align(8))]
pub(super) struct NukeExt4 {
    pub arg: u64,
}

pub(super) fn get_info_with(
    mut call: impl FnMut(u32, &mut GetInfo) -> io::Result<()>,
) -> io::Result<GetInfo> {
    let mut info = GetInfo::default();
    if call(GET_INFO, &mut info).is_err() {
        // Older drivers write fewer fields; discard partial output from a failed probe.
        info = GetInfo::default();
        call(GET_INFO_LEGACY, &mut info)?;
    }
    Ok(info)
}

/// None includes unsupported and permission denied, distinct from disabled KPM.
pub(super) fn kpm_with(call: impl FnOnce(u32, &mut EnableKpm) -> io::Result<()>) -> Option<bool> {
    let mut cmd = EnableKpm::default();
    match call(ENABLE_KPM, &mut cmd) {
        Ok(()) => Some(cmd.enabled != 0),
        Err(err) => {
            log::debug!("KernelSU KPM probe unavailable: {err}");
            None
        }
    }
}

pub(super) fn mount_path(bytes: &[u8]) -> io::Result<CString> {
    if bytes.is_empty() || bytes.len() > 255 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "mount path must be 1–255 bytes",
        ));
    }
    Ok(CString::new(bytes)?)
}

pub(super) fn nuke_with(
    path: &[u8],
    call: impl FnOnce(u32, &mut NukeExt4) -> io::Result<()>,
) -> io::Result<()> {
    let path = mount_path(path)?;
    let mut cmd = NukeExt4 {
        arg: path.as_ptr() as u64,
    };
    call(NUKE_EXT4_SYSFS, &mut cmd)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;
    use std::mem::{align_of, offset_of, size_of};

    #[test]
    fn modern_info_is_used_without_a_legacy_call() {
        let mut calls = 0;
        let info = get_info_with(|request, cmd| {
            calls += 1;
            assert_eq!(request, GET_INFO);
            cmd.version = 50000;
            cmd.flags = 1 << 2;
            cmd.uapi_version = 1;
            Ok(())
        })
        .unwrap();
        assert_eq!(calls, 1);
        assert_eq!(info.version, 50000);
        assert!(info.late_load());
    }

    #[test]
    fn legacy_fallback_resets_a_partially_written_buffer() {
        let mut calls = 0;
        let info = get_info_with(|request, cmd| {
            calls += 1;
            if calls == 1 {
                assert_eq!(request, GET_INFO);
                cmd.flags = 1 << 2;
                cmd.uapi_version = 99;
                return Err(io::ErrorKind::Unsupported.into());
            }
            assert_eq!(request, GET_INFO_LEGACY);
            assert_eq!(*cmd, GetInfo::default());
            cmd.version = 40000;
            Ok(())
        })
        .unwrap();
        assert_eq!(calls, 2);
        assert_eq!(info.version, 40000);
        assert!(!info.late_load());
        assert_eq!(info.uapi_version, 0);
    }

    #[test]
    fn detection_preserves_failure_when_both_requests_fail() {
        let err = get_info_with(|_, _| Err(io::ErrorKind::PermissionDenied.into())).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
    }

    #[test]
    fn late_load_uses_only_the_documented_flag() {
        for flags in [0, 1, 2, 8, 16] {
            assert!(
                !GetInfo {
                    flags,
                    ..GetInfo::default()
                }
                .late_load()
            );
        }
        assert!(
            GetInfo {
                flags: 4,
                ..GetInfo::default()
            }
            .late_load()
        );
    }

    #[test]
    fn kpm_disabled_is_distinct_from_an_unavailable_probe() {
        assert_eq!(
            kpm_with(|request, cmd| {
                assert_eq!(request, ENABLE_KPM);
                assert_eq!(cmd.enabled, 0);
                Ok(())
            }),
            Some(false)
        );
        assert_eq!(
            kpm_with(|_, cmd| {
                cmd.enabled = 1;
                Ok(())
            }),
            Some(true)
        );
        assert_eq!(
            kpm_with(|_, _| Err(io::ErrorKind::PermissionDenied.into())),
            None
        );
        assert_eq!(
            kpm_with(|_, _| Err(io::ErrorKind::Unsupported.into())),
            None
        );
    }

    #[test]
    fn nuke_keeps_the_exact_c_path_alive_during_ioctl() {
        nuke_with(b"/mnt/staging", |request, cmd| {
            assert_eq!(request, NUKE_EXT4_SYSFS);
            // SAFETY: nuke_with keeps its CString alive until the callback returns.
            let path = unsafe { CStr::from_ptr(cmd.arg as *const std::ffi::c_char) };
            assert_eq!(path.to_bytes(), b"/mnt/staging");
            Ok(())
        })
        .unwrap();
        let err = nuke_with(b"/mnt/staging", |_, _| {
            Err(io::ErrorKind::PermissionDenied.into())
        })
        .unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
    }

    #[test]
    fn invalid_nuke_paths_are_rejected_before_ioctl() {
        for path in [Vec::new(), vec![b'x'; 256], b"/a\0b".to_vec()] {
            assert!(nuke_with(&path, |_, _| panic!("invalid path reached ioctl")).is_err());
        }
    }

    #[test]
    fn uapi_layout_matches_aligned_u64_even_on_32_bit_targets() {
        use crate::utils::ksu_umount::UmountCommand;
        assert_eq!(size_of::<GetInfo>(), 16);
        assert_eq!(size_of::<EnableKpm>(), 1);
        assert_eq!(size_of::<NukeExt4>(), 8);
        assert_eq!(align_of::<NukeExt4>(), 8);
        assert_eq!(size_of::<UmountCommand>(), 16);
        assert_eq!(align_of::<UmountCommand>(), 8);
        assert_eq!(offset_of!(UmountCommand, flags), 8);
        assert_eq!(offset_of!(UmountCommand, mode), 12);
    }
}
