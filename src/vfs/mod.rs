// SPDX-License-Identifier: GPL-3.0-only

//! Userspace backend for Hybrid Mount's own VFS kernel subsystem, the `hybridmount` module.

pub mod backend;
pub mod doctor;
pub mod exec;
#[cfg(any(target_os = "linux", target_os = "android"))]
pub mod lkm;
pub mod lkm_target;
pub mod protocol;
pub mod rule;
pub mod sys;

#[cfg(test)]
#[path = "test_support.rs"]
mod test_support;

/// Whether the VFS backend can be offered on this device right now.
///
/// Every surface that advertises VFS reads this, so a kernel without the module shows no VFS
/// option, count or description line anywhere.
pub fn available() -> bool {
    doctor::responds_now()
}
