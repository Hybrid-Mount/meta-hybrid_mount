// SPDX-License-Identifier: GPL-3.0-only

//! OverlayFS backend, behaviour-compatible with v4.2.0 `e20f9c19`.
//!
//! The pure algorithms (escaping, layer splitting, sub-mount relative paths) test across platforms;
//! the fsopen / mount / bind / mountinfo execution is Linux/Android only.

#[allow(clippy::module_inception)]
pub mod overlayfs;

#[cfg(any(target_os = "linux", target_os = "android"))]
pub mod utils;
