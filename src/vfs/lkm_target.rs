// SPDX-License-Identifier: GPL-3.0-only

//! 内核 release 与 Android 版本到打包模块文件名的映射。
//!
//! 刻意不含任何平台相关代码，便于在任意主机上单测；加载逻辑见 lkm.rs。

/// 从 getprop 风格的值里取 Android 主版本号。
pub fn parse_android_major(value: &str) -> Option<u32> {
    value
        .trim()
        .split(|ch: char| !ch.is_ascii_digit())
        .find(|part| !part.is_empty())?
        .parse()
        .ok()
}

/// 从 GKI release 字符串里取 Android 主版本号，例如 5.10.198-android13-8-gki。
pub fn android_major_from_kernel_release(release: &str) -> Option<u32> {
    let lower = release.to_ascii_lowercase();
    let suffix = lower.split_once("android")?.1;
    parse_android_major(suffix)
}

/// 内核主次版本号，例如 5.10.198-android13-8-gki -> (5, 10)。
pub fn kernel_major_minor(release: &str) -> Option<(u32, u32)> {
    let mut parts = release
        .split(|ch: char| !ch.is_ascii_digit())
        .filter(|part| !part.is_empty());
    Some((parts.next()?.parse().ok()?, parts.next()?.parse().ok()?))
}

/// 与 .github/workflows/kernel-module.yml 的 DDK 目标一一对应。
/// android_major 缺省时回退到 release 字符串里编码的版本。
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
    fn refuses_unknown_or_unsafe_matrix_entries() {
        assert_eq!(select_lkm_filename("5.10.198", Some(14)), None);
        assert_eq!(select_lkm_filename("4.14.336-android12-9", None), None);
        assert_eq!(select_lkm_filename("6.18.0-android17-0", None), None);
        assert_eq!(select_lkm_filename("not-a-release", None), None);
    }
}
