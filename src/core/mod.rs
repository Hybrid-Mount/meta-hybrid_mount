use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::{LazyLock, Mutex, RwLock},
    thread,
};

use anyhow::Result;

use crate::{conf::config::Config, mount::overlay::mount_partition};

mod scanner;

pub static MAGIC_MOUNT_ID: LazyLock<Mutex<HashSet<String>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));
pub static OVERLAYFS_ID: LazyLock<Mutex<HashSet<String>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));
pub static PARTITIONS: LazyLock<RwLock<Vec<String>>> = LazyLock::new(|| RwLock::new(Vec::new()));
pub static CONFIG: LazyLock<RwLock<Config>> = LazyLock::new(|| RwLock::new(Config::default()));

pub fn mount(config: Config) -> Result<()> {
    let files =
        scanner::ModuleScanner::new(&config.moduledir, config.partitions.clone()).scanner()?;

    if files.is_empty() {
        log::info!("no modules need mount!!");
        return Ok(());
    }

    CONFIG.set(config)?;

    for file in files {
        if file.magic_mount {
            log::debug!("module {} will use magic_mount to mount", file.id.clone());
            MAGIC_MOUNT_ID.lock().unwrap().insert(file.id.clone());
        }

        if file.overlayfs {
            log::debug!("module {} will use overlayfs to mount", file.id.clone());
            OVERLAYFS_ID.lock().unwrap().insert(file.id.clone());
        }
    }

    thread::Builder::new()
        .name("Moount-Overlayfs".to_string())
        .spawn(move || {
            let mut system_lowerdir: Vec<String> = Vec::new();
            let mut partition_lowerdir: HashMap<String, Vec<String>> = HashMap::new();
            let mut config = CONFIG.read().unwrap();

            for part in config.partitions.iter() {
                partition_lowerdir.insert((*part).to_string(), Vec::new());
            }

            for i in OVERLAYFS_ID.lock().unwrap().iter() {
                let path = Path::new(&config.moduledir).join(i);

                if path.is_dir()
                    && let Some(v) = partition_lowerdir.get_mut(i)
                {
                    v.push(path.display().to_string());
                    log::info!("  + {}/", i);
                }
            }
            if let Err(e) = mount_partition(
                "system",
                &system_lowerdir,
                #[cfg(any(target_os = "linux", target_os = "android"))]
                config.disable_umount,
            ) {
                log::warn!("mount system failed: {e:#}");
            }

            for (k, v) in partition_lowerdir {
                if let Err(e) = mount_partition(
                    &k,
                    &v,
                    #[cfg(any(target_os = "linux", target_os = "android"))]
                    config.disable_umount,
                ) {
                    log::warn!("mount {k} failed: {e:#}");
                }
            }
        })?;
    Ok(())
}
