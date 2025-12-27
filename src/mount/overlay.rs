use anyhow::{Context, Result, bail};
use log::{info, warn};
use std::{
    ffi::CString,
    os::fd::AsRawFd,
    path::{Path, PathBuf},
    thread,
};

use procfs::process::Process;
use rustix::{fd::AsFd, fs::CWD, mount::*};

use crate::defs::KSU_OVERLAY_SOURCE;

#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::ksu::try_umount::send_unmountable;

pub fn mount_overlayfs(
    lower_dirs: &[String],
    lowest: &str,
    upperdir: Option<PathBuf>,
    workdir: Option<PathBuf>,
    dest: impl AsRef<Path>,
    #[cfg(any(target_os = "linux", target_os = "android"))] disable_umount: bool,
) -> Result<()> {
    let lowerdir_config = lower_dirs
        .iter()
        .map(|s| s.as_ref())
        .chain(std::iter::once(lowest))
        .collect::<Vec<_>>()
        .join(":");
    info!(
        "mount overlayfs on {:?}, lowerdir={}, upperdir={:?}, workdir={:?}",
        dest.as_ref(),
        lowerdir_config,
        upperdir,
        workdir
    );

    let upperdir_s = upperdir
        .filter(|up| up.exists())
        .map(|e| e.display().to_string());
    let workdir_s = workdir
        .filter(|wd| wd.exists())
        .map(|e| e.display().to_string());

    let result = (|| {
        let fs = fsopen("overlay", FsOpenFlags::FSOPEN_CLOEXEC)?;
        let fs = fs.as_fd();
        fsconfig_set_string(fs, "lowerdir", &lowerdir_config)?;
        if let (Some(upper), Some(work)) = (&upperdir_s, &workdir_s) {
            fsconfig_set_string(fs, "upperdir", upper)?;
            fsconfig_set_string(fs, "workdir", work)?;
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

    if let Err(e) = result {
        warn!("fsopen mount failed: {e:#}, fallback to mount");
        let mut data = format!("lowerdir={lowerdir_config}");
        if let (Some(upper), Some(work)) = (upperdir_s, workdir_s) {
            data = format!("{data},upperdir={upper},workdir={work}");
        }
        let data_c = CString::new(data)?;
        mount(
            KSU_OVERLAY_SOURCE,
            dest.as_ref(),
            "overlay",
            MountFlags::empty(),
            data_c.as_c_str(),
        )?;
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    if !disable_umount {
        let _ = send_unmountable(dest.as_ref());
    }

    Ok(())
}

pub fn bind_mount(
    from: impl AsRef<Path>,
    to: impl AsRef<Path>,
    #[cfg(any(target_os = "linux", target_os = "android"))] disable_umount: bool,
) -> Result<()> {
    info!(
        "bind mount {} -> {}",
        from.as_ref().display(),
        to.as_ref().display()
    );
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

    #[cfg(any(target_os = "linux", target_os = "android"))]
    if !disable_umount {
        let _ = send_unmountable(to.as_ref());
    }

    Ok(())
}

fn mount_overlay_child(
    mount_point: &str,
    relative: &str,
    module_roots: &[String],
    stock_root: &str,
    #[cfg(any(target_os = "linux", target_os = "android"))] disable_umount: bool,
) -> Result<()> {
    if !module_roots
        .iter()
        .any(|lower| Path::new(lower).join(relative).exists())
    {
        return bind_mount(
            stock_root,
            mount_point,
            #[cfg(any(target_os = "linux", target_os = "android"))]
            disable_umount,
        );
    }

    if !Path::new(stock_root).is_dir() {
        return Ok(());
    }

    let mut lower_dirs: Vec<String> = vec![];
    for lower in module_roots {
        let lower_path = Path::new(lower).join(relative);
        if lower_path.is_dir() {
            lower_dirs.push(lower_path.display().to_string());
        } else if lower_path.exists() {
            return Ok(());
        }
    }

    if lower_dirs.is_empty() {
        return Ok(());
    }

    if let Err(e) = mount_overlayfs(
        &lower_dirs,
        stock_root,
        None,
        None,
        mount_point,
        #[cfg(any(target_os = "linux", target_os = "android"))]
        disable_umount,
    ) {
        warn!("failed to overlay child {mount_point}: {e:#}, fallback to bind mount");
        bind_mount(
            stock_root,
            mount_point,
            #[cfg(any(target_os = "linux", target_os = "android"))]
            disable_umount,
        )?;
    }
    Ok(())
}

pub fn mount_overlay(
    root: &str,
    module_roots: &[String],
    workdir: Option<PathBuf>,
    upperdir: Option<PathBuf>,
    #[cfg(any(target_os = "linux", target_os = "android"))] disable_umount: bool,
) -> Result<()> {
    let root_path = root.to_string();
    let module_roots = module_roots.to_vec();
    let workdir = workdir.clone();
    let upperdir = upperdir.clone();

    let handle = thread::spawn(move || -> Result<()> {
        info!("mount overlay for {root_path} (in isolated thread)");

        unsafe {
            if libc::unshare(libc::CLONE_FS) != 0 {
                bail!(
                    "Failed to unshare CLONE_FS: {}",
                    std::io::Error::last_os_error()
                );
            }
        }

        std::env::set_current_dir(&root_path)
            .with_context(|| format!("failed to chdir to {root_path}"))?;

        let stock_root_base = ".";

        let root_file = std::fs::File::open(stock_root_base)
            .with_context(|| format!("failed to open target root {}", root_path))?;
        let stock_root_fd_path = format!("/proc/self/fd/{}", root_file.as_raw_fd());

        let mounts = Process::myself()?
            .mountinfo()
            .with_context(|| "get mountinfo")?;

        let mut mount_seq = mounts
            .0
            .iter()
            .filter(|m| {
                let mp = m.mount_point.to_string_lossy();
                mp.starts_with(&root_path) && mp != root_path
            })
            .map(|m| m.mount_point.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        mount_seq.sort();
        mount_seq.dedup();

        mount_overlayfs(
            &module_roots,
            &stock_root_fd_path,
            upperdir,
            workdir,
            &root_path,
            #[cfg(any(target_os = "linux", target_os = "android"))]
            disable_umount,
        )
        .with_context(|| "mount overlayfs for root failed")?;

        for mount_point in mount_seq {
            let relative = mount_point.replacen(&root_path, "", 1);
            let relative_clean = relative.trim_start_matches('/');
            let stock_root = format!("{}/{}", stock_root_fd_path, relative_clean);

            if !Path::new(&stock_root).exists() {
                continue;
            }

            if let Err(e) = mount_overlay_child(
                &mount_point,
                relative_clean,
                &module_roots,
                &stock_root,
                #[cfg(any(target_os = "linux", target_os = "android"))]
                disable_umount,
            ) {
                warn!("failed to mount overlay for child {mount_point}: {e:#}, revert");
                umount_dir(&root_path).with_context(|| format!("failed to revert {root_path}"))?;
                bail!(e);
            }
        }
        Ok(())
    });

    handle
        .join()
        .map_err(|e| anyhow::anyhow!("Worker thread panicked: {:?}", e))?
}

pub fn umount_dir(src: impl AsRef<Path>) -> Result<()> {
    unmount(src.as_ref(), UnmountFlags::DETACH)
        .with_context(|| format!("Failed to umount {}", src.as_ref().display()))?;
    Ok(())
}
