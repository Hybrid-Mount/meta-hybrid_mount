// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::sync::{Mutex, MutexGuard};

/// 获取 Mutex 锁，如果被 poisoned 则恢复内部数据
///
/// Poisoned mutex 通常在持有锁的线程 panic 后发生，但其内部数据可能仍然有效。
/// 此函数允许恢复数据而不是传播 panic。
///
/// # 示例
///
/// ```no_run
/// use std::sync::Mutex;
/// use hybrid_mount::utils::lock_or_recover;
///
/// let mutex = Mutex::new(42);
/// let guard = lock_or_recover(&mutex);
/// assert_eq!(*guard, 42);
/// ```
pub fn lock_or_recover<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}
