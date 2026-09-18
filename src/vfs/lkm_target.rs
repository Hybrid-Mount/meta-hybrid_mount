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

#[cfg(test)]
mod tests {
    use super::*;

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
}
