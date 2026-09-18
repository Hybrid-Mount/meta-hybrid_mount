// SPDX-License-Identifier: GPL-3.0-only

//! Android and kernel release parsing shared by platform-specific helpers.
//!
//! Deliberately free of platform code so it is unit-testable on any host.

use std::time::Duration;

use crate::sys::process::{CaptureMode, CommandSpec, run_command};

const GETPROP_TIMEOUT: Duration = Duration::from_secs(10);

/// Reads the Android release from getprop. Best-effort diagnostic path.
pub fn device_android_major() -> Option<u32> {
    for program in ["/system/bin/getprop", "getprop"] {
        let spec = CommandSpec::new(program)
            .operation("read Android version")
            .arg("ro.build.version.release")
            .capture(CaptureMode::Stdout)
            .timeout(GETPROP_TIMEOUT);

        let Ok(outcome) = run_command(&spec) else {
            continue;
        };
        if let Some(major) = outcome
            .stdout_text()
            .as_deref()
            .and_then(parse_android_major)
        {
            return Some(major);
        }
    }
    None
}

/// Android major version from a getprop-style value.
pub fn parse_android_major(value: &str) -> Option<u32> {
    value
        .trim()
        .split(|ch: char| !ch.is_ascii_digit())
        .find(|part| !part.is_empty())?
        .parse()
        .ok()
}

/// Android major version encoded in a GKI release, e.g. 5.10.198-android13-8-gki.
pub fn android_major_from_kernel_release(release: &str) -> Option<u32> {
    let lower = release.to_ascii_lowercase();
    let suffix = lower.split_once("android")?.1;
    parse_android_major(suffix)
}

/// Kernel major/minor, e.g. 5.10.198-android13-8-gki -> (5, 10).
pub fn kernel_major_minor(release: &str) -> Option<(u32, u32)> {
    let mut parts = release
        .split(|ch: char| !ch.is_ascii_digit())
        .filter(|part| !part.is_empty());
    Some((parts.next()?.parse().ok()?, parts.next()?.parse().ok()?))
}

/// Mirrors the DDK targets in .github/workflows/kernel-module.yml.
/// Falls back to the version encoded in the release string when android_major is absent.
pub fn select_lkm_filename(release: &str, android_major: Option<u32>) -> Option<&'static str> {
    let android = android_major.or_else(|| android_major_from_kernel_release(release));
    match (kernel_major_minor(release)?, android) {
        ((5, 10), Some(12)) => Some("hybridmount-android12-5.10.ko"),
        ((5, 10), Some(13)) => Some("hybridmount-android13-5.10.ko"),
        ((5, 15), Some(13)) => Some("hybridmount-android13-5.15.ko"),
        ((5, 15), Some(14)) => Some("hybridmount-android14-5.15.ko"),
        ((6, 1), Some(14)) => Some("hybridmount-android14-6.1.ko"),
        ((6, 6), Some(15)) => Some("hybridmount-android15-6.6.ko"),
        ((6, 12), Some(16)) => Some("hybridmount-android16-6.12.ko"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_the_ddk_matrix() {
        assert_eq!(
            select_lkm_filename("5.10.198-android12-9-gki", None),
            Some("hybridmount-android12-5.10.ko")
        );
        assert_eq!(
            select_lkm_filename("5.10.198-android13-8-gki", None),
            Some("hybridmount-android13-5.10.ko")
        );
        assert_eq!(
            select_lkm_filename("5.15.137-android13-11-gki", None),
            Some("hybridmount-android13-5.15.ko")
        );
        assert_eq!(
            select_lkm_filename("5.15.149-android14-12-gki", None),
            Some("hybridmount-android14-5.15.ko")
        );
        assert_eq!(
            select_lkm_filename("6.1.75-android14-13-gki", None),
            Some("hybridmount-android14-6.1.ko")
        );
        assert_eq!(
            select_lkm_filename("6.6.30-android15-8-gki", None),
            Some("hybridmount-android15-6.6.ko")
        );
        assert_eq!(
            select_lkm_filename("6.12.30-android16-6-gki", None),
            Some("hybridmount-android16-6.12.ko")
        );
    }

    #[test]
    fn uses_the_device_android_version_when_the_release_lacks_one() {
        assert_eq!(
            select_lkm_filename("5.10.198", Some(13)),
            Some("hybridmount-android13-5.10.ko")
        );
    }

    #[test]
    fn parses_android_and_kernel_versions() {
        assert_eq!(
            android_major_from_kernel_release("5.10.198-android13-8-gki"),
            Some(13)
        );
        assert_eq!(
            kernel_major_minor("5.10.198-android13-8-gki"),
            Some((5, 10))
        );
    }

    #[test]
    fn refuses_unknown_or_unsafe_matrix_entries() {
        assert_eq!(select_lkm_filename("5.10.198", Some(14)), None);
        assert_eq!(select_lkm_filename("4.14.336-android12-9", None), None);
        assert_eq!(select_lkm_filename("6.18.0-android17-0", None), None);
        assert_eq!(select_lkm_filename("not-a-release", None), None);
    }

    /// The selector's matrix must match the DDK targets the CI workflow builds, and every
    /// target must resolve to a module the packaging step actually ships.
    #[test]
    fn every_ci_ddk_target_is_selectable_and_packaged() {
        let workflow = include_str!("../../.github/workflows/kernel-module.yml");
        let ci_targets: Vec<&str> = workflow
            .lines()
            .filter_map(|line| line.trim().strip_prefix("- "))
            .filter(|target| target.starts_with("android"))
            .collect();
        assert_eq!(
            ci_targets.len(),
            7,
            "expected the seven supported DDK targets, found {ci_targets:?}"
        );

        for target in ci_targets {
            let (kernel, android) = target
                .split_once('-')
                .and_then(|(android, kernel)| {
                    android.strip_prefix("android").map(|major| (kernel, major))
                })
                .unwrap_or_else(|| panic!("unparsable DDK target {target}"));

            // GKI releases read like "5.10.198-android12-8-gki".
            let release = format!("{kernel}.198-android{android}-8-gki");
            let file_name = select_lkm_filename(&release, None)
                .unwrap_or_else(|| panic!("{target} is built by CI but not selectable"));
            assert_eq!(
                file_name,
                format!("hybridmount-{target}.ko"),
                "{target} maps to a name that does not match the packaging convention"
            );
        }

        assert!(
            select_lkm_filename("5.10.198-android12-9-gki", None).is_some(),
            "the selector must still cover android12-5.10"
        );
    }
}
