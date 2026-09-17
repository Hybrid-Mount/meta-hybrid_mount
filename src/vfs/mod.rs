// SPDX-License-Identifier: GPL-3.0-only

//! Userspace backend for Hybrid Mount's own VFS kernel subsystem (K2).

pub mod backend;
pub mod exec;
#[cfg(any(target_os = "linux", target_os = "android"))]
pub mod lkm;
pub mod lkm_target;
pub mod protocol;
pub mod rule;
pub mod sys;
