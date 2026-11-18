// SPDX-License-Identifier: GPL-3.0-only

/// Supported packages in deterministic fallback order, matching the DDK build matrix.
pub(crate) const TARGETS: &[(u32, u32, u32, &str)] = &[
    (5, 10, 12, "hybridmount-android12-5.10.ko"),
    (5, 10, 13, "hybridmount-android13-5.10.ko"),
    (5, 15, 13, "hybridmount-android13-5.15.ko"),
    (5, 15, 14, "hybridmount-android14-5.15.ko"),
    (6, 1, 14, "hybridmount-android14-6.1.ko"),
    (6, 6, 15, "hybridmount-android15-6.6.ko"),
    (6, 12, 16, "hybridmount-android16-6.12.ko"),
    (6, 18, 17, "hybridmount-android17-6.18.ko"),
];
