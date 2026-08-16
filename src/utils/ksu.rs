// ReHybrid-Mount
//
// SPDX-License-Identifier: GPL-3.0-only

//! KernelSU 尝试卸载列表集成(行为对齐参考项目 `8b85c9e`)。
//!
//! 通过 `ksu` crate 的 `TryUmount` 收集挂载点,流水线结束后一次提交;
//! 检测到 KernelSU 大版本为 4 时禁用该通道(参考项目已知问题规避)。

use std::path::Path;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use ksu::{TryUmount, TryUmountFlags};

use crate::errors::{Error, Result};

static KSU_ACTIVE: AtomicBool = AtomicBool::new(false);
static UMOUNT_BROKEN: AtomicBool = AtomicBool::new(false);
static TRY_UMOUNT_LIST: OnceLock<Mutex<TryUmount>> = OnceLock::new();

/// 启动时检测 KernelSU 是否可用;大版本 4 时禁用卸载列表。
pub fn init() {
    let active = ksu::version().is_some_and(|version| {
        log::info!("KernelSU Version: {version}");
        if version.to_string().starts_with('4') {
            log::warn!(
                "the ioctl function of this KernelSU line is broken, umount list is disabled"
            );
            UMOUNT_BROKEN.store(true, Ordering::Relaxed);
        }
        true
    });

    KSU_ACTIVE.store(active, Ordering::Relaxed);
}

pub fn is_active() -> bool {
    KSU_ACTIVE.load(Ordering::Relaxed)
}

/// 把挂载点加入 KernelSU 尝试卸载列表(不可用或已禁用时为空操作)。
pub fn send_unmountable(target: impl AsRef<Path>) {
    if !is_active() || UMOUNT_BROKEN.load(Ordering::Relaxed) {
        return;
    }

    TRY_UMOUNT_LIST
        .get_or_init(|| Mutex::new(TryUmount::new()))
        .lock()
        .expect("try-umount list poisoned")
        .add(target);
}

/// 提交尝试卸载列表(`MNT_DETACH`),参考项目在流水线结束后调用。
pub fn commit_unmount_list() -> Result<()> {
    if !is_active() {
        return Ok(());
    }

    let mut control = TRY_UMOUNT_LIST
        .get_or_init(|| Mutex::new(TryUmount::new()))
        .lock()
        .expect("try-umount list poisoned");

    control.flags(TryUmountFlags::MNT_DETACH);
    control.format_msg(|paths| format!("umount {paths:?} successful"));
    control
        .umount()
        .map_err(|err| Error::msg(format!("commit KernelSU try-umount list: {err}")))?;
    Ok(())
}
