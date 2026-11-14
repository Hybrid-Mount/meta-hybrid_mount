// SPDX-License-Identifier: GPL-3.0-only

//! Magic Mount backend, behaviour-compatible with meta-magic_mount-rs `8b85c9e`.
//!
//! Node types, module contributions and backend selection all live in [`crate::mount_tree`];
//! this module keeps only the Magic Mount executor and never rescans module directories or builds a private tree.

#[cfg(any(target_os = "linux", target_os = "android"))]
pub mod exec;
