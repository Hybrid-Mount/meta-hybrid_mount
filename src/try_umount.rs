use std::{
    path::Path,
    sync::{LazyLock, Mutex, OnceLock},
};

use anyhow::Result;
use ksu::{NukeExt4Sysfs, TryUmount};

pub static TMPFS: OnceLock<String> = OnceLock::new();
pub static LIST: LazyLock<Mutex<TryUmount>> = LazyLock::new(|| Mutex::new(TryUmount::new()));

pub fn send_unmountable<P>(target: P) -> Result<()>
where
    P: AsRef<Path>,
{
    LIST.lock()
        .map_err(|_| anyhow::anyhow!("Failed to lock unmount list"))?
        .add(target);
    Ok(())
}

pub fn commit() -> Result<()> {
    let mut list = LIST
        .lock()
        .map_err(|_| anyhow::anyhow!("Failed to lock unmount list"))?;
    list.flags(2);
    list.umount()?;
    Ok(())
}

pub fn ksu_nuke_sysfs(target: &str) -> Result<()> {
    NukeExt4Sysfs::new().add(target).execute()?;
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn ksu_nuke_sysfs(_target: &str) -> Result<()> {
    bail!("Not supported on this OS")
}
