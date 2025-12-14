use anyhow::{Context, Result, bail};
use log::{info, warn};
use std::path::{Path, PathBuf};
use std::fs;
use std::os::fd::AsRawFd;
use std::time::{SystemTime, UNIX_EPOCH};
use std::ffi::CString;
use procfs::process::Process;
use rustix::{fd::AsFd, fs::CWD, mount::*};
use crate::defs::{KSU_OVERLAY_SOURCE, RUN_DIR};
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::try_umount::send_unmountable;

const PAGE_LIMIT: usize = 4000;
pub fn mount_overlayfs(
    lower_dirs: &[String],
    lowest: &str,
    upperdir: Option<PathBuf>,
    workdir: Option<PathBuf>,
    dest: impl AsRef<Path>,
    disable_umount: bool,
) -> Result<()> {
    let lowerdir_config = lower_dirs
        .iter()
        .map(|s| s.as_ref())
        .chain(std::iter::once(lowest))
        .collect::<Vec<_>>()
        .join(":");
    if lowerdir_config.len() < PAGE_LIMIT {
        return do_mount_overlay(
            &lowerdir_config,
            upperdir,
            workdir,
            dest,
            disable_umount
        );
    }
    info!("!! Lowerdir params too long ({} bytes), switching to staged mount.", lowerdir_config.len());
    if upperdir.is_some() || workdir.is_some() {
        bail!("Staged mount not supported for RW overlay (upperdir/workdir present)");
    }
    mount_overlayfs_staged(lower_dirs, lowest, dest, disable_umount)
}
fn mount_overlayfs_staged(
    lower_dirs: &[String],
    lowest: &str,
    dest: impl AsRef<Path>,
    disable_umount: bool,
) -> Result<()> {
    let mut batches: Vec<Vec<String>> = Vec::new();
    let mut current_batch: Vec<String> = Vec::new();
    let mut current_len = 0;
    const SAFE_CHUNK_SIZE: usize = 3500;
    for dir in lower_dirs {
        if current_len + dir.len() + 1 > SAFE_CHUNK_SIZE {
            batches.push(current_batch);
            current_batch = Vec::new();
            current_len = 0;
        }
        current_batch.push(dir.clone());
        current_len += dir.len() + 1;
    }
    if !current_batch.is_empty() {
        batches.push(current_batch);
    }
    let staging_root = Path::new(RUN_DIR).join("staging");
    if !staging_root.exists() {
        fs::create_dir_all(&staging_root).context("failed to create staging dir")?;
    }
    let mut current_base = lowest.to_string();
    for (i, batch) in batches.iter().rev().enumerate() {
        let is_last_layer = i == batches.len() - 1; 
        let target_path = if is_last_layer {
            dest.as_ref().to_path_buf()
        } else {
            let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
            let stage_dir = staging_root.join(format!("stage_{}_{}", timestamp, i));
            fs::create_dir_all(&stage_dir)?;
            stage_dir
        };
        let lowerdir_str = batch
            .iter()
            .map(|s| s.as_str())
            .chain(std::iter::once(current_base.as_str()))
            .collect::<Vec<_>>()
            .join(":");
        do_mount_overlay(
            &lowerdir_str,
            None,
            None,
            &target_path,
            disable_umount
        )?;
        current_base = target_path.to_string_lossy().to_string();
    }
    Ok(())
}
fn do_mount_overlay(
    lowerdir_config: &str,
    upperdir: Option<PathBuf>,
    workdir: Option<PathBuf>,
    dest: impl AsRef<Path>,
    disable_umount: bool,
) -> Result<()> {
    let upperdir_s = upperdir
        .filter(|up| up.exists())
        .map(|e| e.display().to_string());
    let workdir_s = workdir
        .filter(|wd| wd.exists())
        .map(|e| e.display().to_string());
    let result = (|| {
        let fs = fsopen("overlay", FsOpenFlags::FSOPEN_CLOEXEC)?;
        let fs = fs.as_fd();
        fsconfig_set_string(fs, "lowerdir", lowerdir_config)?;
        if let (Some(upperdir), Some(workdir)) = (&upperdir_s, &workdir_s) {
            fsconfig_set_string(fs, "upperdir", upperdir)?;
            fsconfig_set_string(fs, "workdir", workdir)?;
        }
        fsconfig_set_string(fs, "source", KSU_OVERLAY_SOURCE)?;
        fsconfig_create(fs)?;
        let mount = fsmount(fs, FsMountFlags::FSMOUNT_CLOEXEC, MountAttrFlags::empty())?;
        move_mount(
            mount.as_fd(),
            "",
            CWD,
            dest.as_ref(),
            MoveMountFlags::MOVE_MOUNT_F_EMPTY_PATH,
        )
    })();
    if let Err(fsopen_err) = result {
        let mut data = format!("lowerdir={lowerdir_config}");
        if let (Some(upperdir), Some(workdir)) = (upperdir_s, workdir_s) {
            data = format!("{data},upperdir={upperdir},workdir={workdir}");
        }
        let data_c = CString::new(data).map_err(|e| anyhow::anyhow!("Invalid string for mount data: {}", e))?;
        mount(
            KSU_OVERLAY_SOURCE,
            dest.as_ref(),
            "overlay",
            MountFlags::empty(),
            Some(data_c.as_c_str()),
        ).map_err(|mount_err| anyhow::anyhow!("Legacy mount failed: {} (fsopen error: {})", mount_err, fsopen_err))?;
    }
    if !disable_umount {
        #[cfg(any(target_os = "linux", target_os = "android"))]
        let _ = send_unmountable(dest.as_ref());
    }
    Ok(())
}
pub fn bind_mount(from: impl AsRef<Path>, to: impl AsRef<Path>, disable_umount: bool) -> Result<()> {
    let tree = open_tree(
        CWD,
        from.as_ref(),
        OpenTreeFlags::OPEN_TREE_CLOEXEC
            | OpenTreeFlags::OPEN_TREE_CLONE
            | OpenTreeFlags::AT_RECURSIVE,
    )?;
    move_mount(
        tree.as_fd(),
        "",
        CWD,
        to.as_ref(),
        MoveMountFlags::MOVE_MOUNT_F_EMPTY_PATH,
    )?;
    if !disable_umount {
        #[cfg(any(target_os = "linux", target_os = "android"))]
        let _ = send_unmountable(to.as_ref());
    }
    Ok(())
}
fn mount_overlay_child(
    mount_point: &str,
    relative: &str,
    module_roots: &[String],
    stock_root: &str,
    disable_umount: bool,
) -> Result<()> {
    let has_modification = module_roots.iter().any(|lower| {
        let path = Path::new(lower).join(relative.trim_start_matches('/'));
        path.exists()
    });
    if !has_modification {
        return bind_mount(stock_root, mount_point, disable_umount);
    }
    if !Path::new(stock_root).is_dir() {
        return Ok(());
    }
    let mut lower_dirs: Vec<String> = vec![];
    for lower in module_roots {
        let path = Path::new(lower).join(relative.trim_start_matches('/'));
        if path.is_dir() {
            lower_dirs.push(path.display().to_string());
        } else if path.exists() {
            return Ok(());
        }
    }
    if lower_dirs.is_empty() {
        return Ok(());
    }
    if let Err(e) = mount_overlayfs(&lower_dirs, stock_root, None, None, mount_point, disable_umount) {
        warn!("failed to overlay child {mount_point}: {e:#}, fallback to bind mount");
        bind_mount(stock_root, mount_point, disable_umount)?;
    }
    Ok(())
}
pub fn mount_overlay(
    target_root: &str,
    module_roots: &[String],
    workdir: Option<PathBuf>,
    upperdir: Option<PathBuf>,
    disable_umount: bool,
) -> Result<()> {
    let root_file = fs::File::open(target_root)
        .with_context(|| format!("failed to open target root {}", target_root))?;
    let stock_root = format!("/proc/self/fd/{}", root_file.as_raw_fd());
    let mounts = Process::myself()?
        .mountinfo()
        .with_context(|| "get mountinfo")?;
    let mut mount_seq = mounts.0.iter()
        .filter(|m| {
            m.mount_point.starts_with(target_root) && 
            m.mount_point != Path::new(target_root)
        })
        .map(|m| m.mount_point.to_string_lossy().to_string())
        .collect::<Vec<_>>();
    mount_seq.sort();
    mount_seq.dedup();
    mount_overlayfs(module_roots, &stock_root, upperdir, workdir, target_root, disable_umount)
        .with_context(|| format!("mount overlayfs for root {target_root} failed"))?;
    for mount_point in mount_seq {
        let relative = mount_point.replacen(target_root, "", 1);
        let stock_root_relative = format!("{}{}", stock_root, relative);
        if !Path::new(&stock_root_relative).exists() {
            continue;
        }
        if let Err(e) = mount_overlay_child(&mount_point, &relative, module_roots, &stock_root_relative, disable_umount) {
            warn!("failed to restore child mount {mount_point}: {e:#}");
        }
    }
    Ok(())
}
