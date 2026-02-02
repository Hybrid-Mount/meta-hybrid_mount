// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    collections::HashSet,
    path::Path,
    sync::{LazyLock, Mutex, OnceLock},
};

use anyhow::Result;
use ksu::TryUmount;

pub static TMPFS: OnceLock<String> = OnceLock::new();
pub static LIST: LazyLock<Mutex<TryUmount>> = LazyLock::new(|| Mutex::new(TryUmount::new()));
static HISTORY: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

pub fn send_umountable<P>(target: P) -> Result<()>
where
    P: AsRef<Path>,
{
    if !crate::utils::KSU.load(std::sync::atomic::Ordering::Relaxed) {
        return Ok(());
    }

    let path_str = target.as_ref().to_string_lossy().to_string();
    let mut history = HISTORY
        .lock()
        .map_err(|_| anyhow::anyhow!("Failed to lock history mutex"))?;

    if history.contains(&path_str) {
        log::debug!("Ignored duplicate umount request: {}", path_str);
        return Ok(());
    }

    history.insert(path_str);
    LIST.lock()
        .map_err(|_| anyhow::anyhow!("Failed to lock umount list"))?
        .add(target);
    Ok(())
}

pub fn commit() -> Result<()> {
    if !crate::utils::KSU.load(std::sync::atomic::Ordering::Relaxed) {
        return Ok(());
    }
    let mut list = LIST
        .lock()
        .map_err(|_| anyhow::anyhow!("Failed to lock umount list"))?;

    list.flags(0);
    if let Err(e0) = list.umount() {
        log::debug!("try_umount(0) failed: {:#}, retrying with flags(2)", e0);

        list.flags(2);
        if let Err(e2) = list.umount() {
            log::warn!("try_umount(2) failed: {:#}", e2);
        }
    }

    if let Ok(mut history) = HISTORY.lock() {
        history.clear();
    }

    Ok(())
}
