// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    collections::HashSet,
    path::Path,
    sync::{LazyLock, Mutex},
};

use anyhow::Result;
use ksu::{TryUmount, TryUmountFlags};
use rustix::path::Arg;

pub static LIST: LazyLock<Mutex<TryUmount>> = LazyLock::new(|| Mutex::new(TryUmount::new()));
static HISTORY: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

pub fn send_umountable<P>(target: P) -> Result<()>
where
    P: AsRef<Path>,
{
    if !crate::utils::KSU.load(std::sync::atomic::Ordering::Relaxed) {
        return Ok(());
    }

    let target = target.as_ref();
    let path = target.as_str()?;
    let mut history = HISTORY
        .lock()
        .map_err(|_| anyhow::anyhow!("Failed to lock history mutex"))?;

    history.insert(path.to_string());
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

    list.format_msg(|p| format!("{p:?} umount successful "));
    list.flags(TryUmountFlags::MNT_DETACH);
    if let Err(e2) = list.umount() {
        log::warn!("try_umount failed: {:#}", e2);
    }

    Ok(())
}
