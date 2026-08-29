// SPDX-License-Identifier: GPL-3.0-only

//! Test-only fault injection gates for mount pipeline failure paths.

#![cfg_attr(not(any(target_os = "linux", target_os = "android")), allow(dead_code))]

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

static OVERLAY_MOUNT_FAILURE_ARMED: AtomicBool = AtomicBool::new(false);
static OVERLAY_MOUNT_SUCCESSES_BEFORE_FAILURE: AtomicUsize = AtomicUsize::new(0);
static FAIL_NEXT_MAGIC_MOUNT: AtomicBool = AtomicBool::new(false);
static FAIL_KSU_COMMIT: AtomicBool = AtomicBool::new(false);
static FAIL_STATE_SAVE: AtomicBool = AtomicBool::new(false);
static FAIL_MOUNTINFO_READ: AtomicBool = AtomicBool::new(false);
static FAIL_NEXT_UNMOUNT_EBUSY: AtomicBool = AtomicBool::new(false);
static FAIL_STAGING_REMOVE: AtomicBool = AtomicBool::new(false);

pub fn should_fail_next_overlay_mount() -> bool {
    if !OVERLAY_MOUNT_FAILURE_ARMED.load(Ordering::SeqCst) {
        return false;
    }
    let remaining = OVERLAY_MOUNT_SUCCESSES_BEFORE_FAILURE.load(Ordering::SeqCst);
    if remaining > 0 {
        OVERLAY_MOUNT_SUCCESSES_BEFORE_FAILURE.store(remaining - 1, Ordering::SeqCst);
        return false;
    }
    OVERLAY_MOUNT_FAILURE_ARMED.store(false, Ordering::SeqCst);
    true
}

pub fn should_fail_next_magic_mount() -> bool {
    FAIL_NEXT_MAGIC_MOUNT.swap(false, Ordering::SeqCst)
}

pub fn should_fail_ksu_commit() -> bool {
    FAIL_KSU_COMMIT.swap(false, Ordering::SeqCst)
}

pub fn should_fail_state_save() -> bool {
    FAIL_STATE_SAVE.swap(false, Ordering::SeqCst)
}

pub fn should_fail_mountinfo_read() -> bool {
    FAIL_MOUNTINFO_READ.swap(false, Ordering::SeqCst)
}

pub fn should_fail_next_unmount_ebusy() -> bool {
    FAIL_NEXT_UNMOUNT_EBUSY.swap(false, Ordering::SeqCst)
}

pub fn should_fail_staging_remove() -> bool {
    FAIL_STAGING_REMOVE.swap(false, Ordering::SeqCst)
}

#[cfg(test)]
pub fn enable_next_overlay_mount_failure() {
    enable_overlay_mount_failure_after(0);
}

#[cfg(test)]
pub fn enable_overlay_mount_failure_after(successes: usize) {
    OVERLAY_MOUNT_SUCCESSES_BEFORE_FAILURE.store(successes, Ordering::SeqCst);
    OVERLAY_MOUNT_FAILURE_ARMED.store(true, Ordering::SeqCst);
}

#[cfg(test)]
pub fn enable_next_magic_mount_failure() {
    FAIL_NEXT_MAGIC_MOUNT.store(true, Ordering::SeqCst);
}

#[cfg(test)]
pub fn enable_ksu_commit_failure() {
    FAIL_KSU_COMMIT.store(true, Ordering::SeqCst);
}

#[cfg(test)]
pub fn enable_state_save_failure() {
    FAIL_STATE_SAVE.store(true, Ordering::SeqCst);
}

#[cfg(test)]
pub fn enable_mountinfo_read_failure() {
    FAIL_MOUNTINFO_READ.store(true, Ordering::SeqCst);
}

#[cfg(test)]
pub fn enable_next_unmount_ebusy_failure() {
    FAIL_NEXT_UNMOUNT_EBUSY.store(true, Ordering::SeqCst);
}

#[cfg(test)]
pub fn enable_staging_remove_failure() {
    FAIL_STAGING_REMOVE.store(true, Ordering::SeqCst);
}

#[cfg(test)]
pub fn reset() {
    OVERLAY_MOUNT_FAILURE_ARMED.store(false, Ordering::SeqCst);
    OVERLAY_MOUNT_SUCCESSES_BEFORE_FAILURE.store(0, Ordering::SeqCst);
    for gate in [
        &FAIL_NEXT_MAGIC_MOUNT,
        &FAIL_KSU_COMMIT,
        &FAIL_STATE_SAVE,
        &FAIL_MOUNTINFO_READ,
        &FAIL_NEXT_UNMOUNT_EBUSY,
        &FAIL_STAGING_REMOVE,
    ] {
        gate.store(false, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gates_are_consumed_exactly_once() {
        reset();
        enable_next_overlay_mount_failure();

        assert!(should_fail_next_overlay_mount());
        assert!(!should_fail_next_overlay_mount());
        reset();
    }

    #[test]
    fn every_gate_is_consumed_exactly_once() {
        reset();
        enable_next_overlay_mount_failure();
        enable_next_magic_mount_failure();
        enable_ksu_commit_failure();
        enable_state_save_failure();
        enable_mountinfo_read_failure();
        enable_next_unmount_ebusy_failure();
        enable_staging_remove_failure();

        assert!(should_fail_next_overlay_mount());
        assert!(should_fail_next_magic_mount());
        assert!(should_fail_ksu_commit());
        assert!(should_fail_state_save());
        assert!(should_fail_mountinfo_read());
        assert!(should_fail_next_unmount_ebusy());
        assert!(should_fail_staging_remove());

        assert!(!should_fail_next_overlay_mount());
        assert!(!should_fail_next_magic_mount());
        assert!(!should_fail_ksu_commit());
        assert!(!should_fail_state_save());
        assert!(!should_fail_mountinfo_read());
        assert!(!should_fail_next_unmount_ebusy());
        assert!(!should_fail_staging_remove());
        reset();
    }

    #[test]
    fn overlay_failure_can_skip_successful_mounts() {
        reset();
        enable_overlay_mount_failure_after(2);

        assert!(!should_fail_next_overlay_mount());
        assert!(!should_fail_next_overlay_mount());
        assert!(should_fail_next_overlay_mount());
        assert!(!should_fail_next_overlay_mount());
        reset();
    }

    #[test]
    fn reset_clears_all_gates() {
        enable_next_overlay_mount_failure();
        enable_state_save_failure();
        enable_next_unmount_ebusy_failure();

        reset();

        assert!(!should_fail_next_overlay_mount());
        assert!(!should_fail_state_save());
        assert!(!should_fail_next_unmount_ebusy());
    }
}
