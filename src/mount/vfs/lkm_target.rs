// SPDX-License-Identifier: GPL-3.0-only

use crate::platform::kernel::{android_major_from_kernel_release, kernel_major_minor};

#[path = "lkm_packages.rs"]
mod packages;
use packages::TARGETS;

/// Prefer the kernel's GKI label; an optional hint never rules out same-line candidates.
pub fn select_lkm_filename(release: &str, android_hint: Option<u32>) -> Option<&'static str> {
    let kernel = kernel_major_minor(release)?;
    let android = android_major_from_kernel_release(release).or(android_hint);
    let matches_kernel = |entry: &&(u32, u32, u32, &str)| (entry.0, entry.1) == kernel;
    TARGETS
        .iter()
        .filter(matches_kernel)
        .find(|entry| Some(entry.2) == android)
        .or_else(|| TARGETS.iter().find(matches_kernel))
        .map(|entry| entry.3)
}

/// NoMount's exact-first, same-kernel-line fallback. Android userspace is not a GKI label.
pub fn lkm_candidates(release: &str) -> Vec<&'static str> {
    let Some(kernel) = kernel_major_minor(release) else {
        return Vec::new();
    };
    let preferred = select_lkm_filename(release, None);
    let mut candidates: Vec<_> = TARGETS
        .iter()
        .filter(|entry| (entry.0, entry.1) == kernel)
        .map(|entry| entry.3)
        .collect();
    candidates.sort_by_key(|name| Some(*name) != preferred);
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_kernel_is_not_rejected_by_newer_android_userspace() {
        assert_eq!(
            select_lkm_filename("5.15.197-@Coolpak@Kugouzei_NB_LKM", Some(16)),
            Some("hybridmount-android13-5.15.ko")
        );
    }

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
    fn explicit_branch_hint_can_prioritize_a_same_line_candidate() {
        assert_eq!(
            select_lkm_filename("5.10.198", Some(13)),
            Some("hybridmount-android13-5.10.ko")
        );
    }

    #[test]
    fn candidates_prefer_exact_branch_then_stay_on_the_same_kernel_line() {
        assert_eq!(
            lkm_candidates("5.15.197-android14-custom"),
            vec![
                "hybridmount-android14-5.15.ko",
                "hybridmount-android13-5.15.ko"
            ]
        );
        assert_eq!(
            lkm_candidates("5.15.197-@Coolpak@Kugouzei_NB_LKM"),
            vec![
                "hybridmount-android13-5.15.ko",
                "hybridmount-android14-5.15.ko"
            ]
        );
        assert_eq!(
            lkm_candidates("6.1.99-custom"),
            vec!["hybridmount-android14-6.1.ko"]
        );
        assert!(lkm_candidates("4.19.999-custom").is_empty());
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
        assert_eq!(
            select_lkm_filename("5.10.198", Some(14)),
            Some("hybridmount-android12-5.10.ko")
        );
        assert_eq!(select_lkm_filename("4.14.336-android12-9", None), None);
        assert_eq!(
            select_lkm_filename("6.18.0-android17-0", None),
            Some("hybridmount-android17-6.18.ko")
        );
        assert_eq!(select_lkm_filename("not-a-release", None), None);
    }

    #[test]
    fn every_ci_ddk_target_is_selectable_and_packaged() {
        let workflow = include_str!("../../../.github/workflows/kernel-module.yml");
        let ci_targets: Vec<&str> = workflow
            .lines()
            .filter_map(|line| line.trim().strip_prefix("- "))
            .filter(|target| target.starts_with("android"))
            .collect();
        assert_eq!(
            ci_targets.len(),
            TARGETS.len(),
            "DDK targets must cover the supported kernel matrix: {ci_targets:?}"
        );

        for target in ci_targets {
            let (kernel, android) = target
                .split_once('-')
                .and_then(|(android, kernel)| {
                    android.strip_prefix("android").map(|major| (kernel, major))
                })
                .unwrap_or_else(|| panic!("unparsable DDK target {target}"));

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
