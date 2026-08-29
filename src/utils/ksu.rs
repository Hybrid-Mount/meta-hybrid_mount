// SPDX-License-Identifier: GPL-3.0-only

//! KernelSU 尝试卸载列表集成(注册行为对齐参考项目 `8b85c9e`;
//! 去重与忽略分区对齐本仓库 v4.2.0 `umount_mgr`)。
//!
//! 注意语义边界:这里只把挂载点**注册**进内核列表,不做立即卸载;
//! 立即卸载必须走 rustix 的 `unmount` 系统调用。

use std::collections::HashSet;
use std::path::Path;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use ::ksu::{TryUmount, TryUmountFlags};

use crate::errors::{Error, Result};
use crate::utils::is_ignored_unmount_partition;

static KSU_ACTIVE: AtomicBool = AtomicBool::new(false);
static UMOUNT_BROKEN: AtomicBool = AtomicBool::new(false);
static TRY_UMOUNT_LIST: OnceLock<Mutex<TryUmount>> = OnceLock::new();
static REGISTERED_PATHS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

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

/// 把挂载点加入 KernelSU 尝试卸载列表(不可用/禁用/命中忽略分区/重复时为空操作)。
pub fn send_unmountable(target: impl AsRef<Path>) {
    if !is_active() || UMOUNT_BROKEN.load(Ordering::Relaxed) {
        return;
    }

    let path = target.as_ref();
    let Some(path_str) = path.to_str() else {
        return;
    };

    if is_ignored_unmount_partition(path_str) {
        log::debug!("skip try-umount registration: path={path_str}, reason=ignore_partition");
        return;
    }

    let mut history = REGISTERED_PATHS
        .get_or_init(|| Mutex::new(HashSet::new()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    if !history.insert(path_str.to_owned()) {
        return;
    }
    drop(history);

    TRY_UMOUNT_LIST
        .get_or_init(|| Mutex::new(TryUmount::new()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .add(path);
}

/// 提交尝试卸载列表(`MNT_DETACH`),流水线结束后调用。
pub fn commit_unmount_list() -> Result<()> {
    if crate::sys::faults::should_fail_ksu_commit() {
        return Err(Error::msg("injected KernelSU try-umount commit failure"));
    }
    if !is_active() {
        return Ok(());
    }

    let mut control = TRY_UMOUNT_LIST
        .get_or_init(|| Mutex::new(TryUmount::new()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    control.flags(TryUmountFlags::MNT_DETACH);
    control.format_msg(|paths| format!("umount {paths:?} successful"));
    control
        .umount()
        .map_err(|err| Error::msg(format!("commit KernelSU try-umount list: {err}")))?;
    Ok(())
}

/// 清空内核 try-umount 列表与本进程注册历史,仅用于失败回滚。
pub fn clear_unmount_list() -> Result<()> {
    if !is_active() {
        return Ok(());
    }

    TRY_UMOUNT_LIST
        .get_or_init(|| Mutex::new(TryUmount::new()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .wipe()
        .map_err(|err| Error::msg(format!("wipe KernelSU try-umount list: {err}")))?;

    if let Some(history) = REGISTERED_PATHS.get() {
        history
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
    }
    Ok(())
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn ksu_commit_failure_injection_is_one_shot() {
        crate::sys::faults::enable_ksu_commit_failure();
        let err = commit_unmount_list().unwrap_err();
        assert!(err.to_string().contains("injected KernelSU"), "{err}");
        crate::sys::faults::reset();

        assert!(commit_unmount_list().is_ok());
        assert!(clear_unmount_list().is_ok());
    }
}
