use anyhow::{Context, Result};
use log::{info, warn};
use std::{
    ffi::CString,
    fs,
    os::fd::AsRawFd,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use rustix::{fd::AsFd, fs::CWD, mount::*};

use crate::defs::{KSU_OVERLAY_SOURCE, RUN_DIR};
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::utils::send_unmountable;

const PAGE_LIMIT: usize = 4000;

struct StagedMountGuard {
    mounts: Vec<PathBuf>,
    committed: bool,
}

impl Drop for StagedMountGuard {
    fn drop(&mut self) {
        if !self.committed {
            for path in self.mounts.iter().rev() {
                let _ = unmount(path, UnmountFlags::DETACH);
                let _ = fs::remove_dir(path);
            }
        }
    }
}

fn get_overlay_features() -> String {
    let mut features = String::new();

    if Path::new("/sys/module/overlay/parameters/redirect_dir").exists() {
        features.push_str(",redirect_dir=on");
    }

    if Path::new("/sys/module/overlay/parameters/metacopy").exists() {
        if !features.contains("redirect_dir") {
            features.push_str(",redirect_dir=on");
        }
        features.push_str(",metacopy=on");
    }

    features
}

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

    match do_mount_overlay(
        &lowerdir_config,
        upperdir.clone(),
        workdir.clone(),
        dest.as_ref(),
        #[cfg(any(target_os = "linux", target_os = "android"))]
        disable_umount,
    ) {
        Ok(_) => Ok(()),
        Err(e) => {
            if lowerdir_config.len() >= PAGE_LIMIT {
                if upperdir.is_some() || workdir.is_some() {
                    return Err(e);
                }
                info!("Direct overlay mount failed (possibly due to length limits), switching to staged mount. Error: {}", e);
                return mount_overlayfs_staged(
                    lower_dirs,
                    lowest,
                    dest,
                    #[cfg(any(target_os = "linux", target_os = "android"))]
                    disable_umount,
                );
            }
            Err(e)
        }
    }
}

fn mount_overlayfs_staged(
    lower_dirs: &[String],
    lowest: &str,
    dest: impl AsRef<Path>,
    #[cfg(any(target_os = "linux", target_os = "android"))] disable_umount: bool,
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
    let mut guard = StagedMountGuard {
        mounts: Vec::new(),
        committed: false,
    };

    for (i, batch) in batches.iter().rev().enumerate() {
        let is_last_layer = i == batches.len() - 1;

        let target_path = if is_last_layer {
            dest.as_ref().to_path_buf()
        } else {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .context("System time is before UNIX EPOCH")?
                .as_nanos();

            let stage_dir = staging_root.join(format!("stage_{}_{}", timestamp, i));
            fs::create_dir_all(&stage_dir)
                .with_context(|| format!("Failed to create stage dir {:?}", stage_dir))?;
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
            #[cfg(any(target_os = "linux", target_os = "android"))]
            disable_umount,
        )?;

        if !is_last_layer {
            guard.mounts.push(target_path.clone());
            current_base = target_path.to_string_lossy().to_string();
        }
    }

    guard.committed = true;
    Ok(())
}

fn do_mount_overlay(
    lowerdir_config: &str,
    upperdir: Option<PathBuf>,
    workdir: Option<PathBuf>,
    dest: impl AsRef<Path>,
    #[cfg(any(target_os = "linux", target_os = "android"))] disable_umount: bool,
) -> Result<()> {
    let upperdir_s = upperdir
        .filter(|up| up.exists())
        .map(|e| e.display().to_string());
    let workdir_s = workdir
        .filter(|wd| wd.exists())
        .map(|e| e.display().to_string());

    let extra_features = get_overlay_features();

    let result = (|| {
        let fs = fsopen("overlay", FsOpenFlags::FSOPEN_CLOEXEC)?;
        let fs = fs.as_fd();

        fsconfig_set_string(fs, "lowerdir", lowerdir_config)?;

        if let (Some(upperdir), Some(workdir)) = (&upperdir_s, &workdir_s) {
            fsconfig_set_string(fs, "upperdir", upperdir)?;
            fsconfig_set_string(fs, "workdir", workdir)?;
        }

        if extra_features.contains("redirect_dir") {
            let _ = fsconfig_set_string(fs, "redirect_dir", "on");
        }
        if extra_features.contains("metacopy") {
            let _ = fsconfig_set_string(fs, "metacopy", "on");
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

        data.push_str(&extra_features);

        let data_c = CString::new(data).context("Invalid string for mount data")?;

        mount(
            KSU_OVERLAY_SOURCE,
            dest.as_ref(),
            "overlay",
            MountFlags::empty(),
            Some(data_c.as_c_str()),
        )
        .with_context(|| format!("Legacy mount failed (fsopen also failed: {})", fsopen_err))?;
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
    let tree = open_tree(
        CWD,
        from.as_ref(),
        OpenTreeFlags::OPEN_TREE_CLOEXEC
            | OpenTreeFlags::OPEN_TREE_CLONE
            | OpenTreeFlags::AT_RECURSIVE,
    )
    .with_context(|| format!("open_tree failed for {}", from.as_ref().display()))?;

    move_mount(
        tree.as_fd(),
        "",
        CWD,
        to.as_ref(),
        MoveMountFlags::MOVE_MOUNT_F_EMPTY_PATH,
    )
    .with_context(|| format!("move_mount failed to {}", to.as_ref().display()))?;

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
    let has_modification = module_roots.iter().any(|lower| {
        let path = Path::new(lower).join(relative.trim_start_matches('/'));
        path.exists()
    });

    if !has_modification {
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

    if let Err(e) = mount_overlayfs(
        &lower_dirs,
        stock_root,
        None,
        None,
        mount_point,
        #[cfg(any(target_os = "linux", target_os = "android"))]
        disable_umount,
    ) {
        warn!(
            "failed to overlay child {mount_point}: {:#}, fallback to bind mount",
            e
        );
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
    target_root: &str,
    module_roots: &[String],
    workdir: Option<PathBuf>,
    upperdir: Option<PathBuf>,
    child_mounts: &[String],
    #[cfg(any(target_os = "linux", target_os = "android"))] disable_umount: bool,
) -> Result<()> {
    let root_file = fs::File::open(target_root)
        .with_context(|| format!("failed to open target root {}", target_root))?;

    let stock_root = format!("/proc/self/fd/{}", root_file.as_raw_fd());

    mount_overlayfs(
        module_roots,
        &stock_root,
        upperdir,
        workdir,
        target_root,
        #[cfg(any(target_os = "linux", target_os = "android"))]
        disable_umount,
    )
    .with_context(|| format!("mount overlayfs for root {target_root} failed"))?;

    for mount_point in child_mounts {
        let relative = mount_point.replacen(target_root, "", 1);
        let stock_root_relative = format!("{}{}", stock_root, relative);

        if !Path::new(&stock_root_relative).exists() {
            continue;
        }

        if let Err(e) = mount_overlay_child(
            mount_point,
            &relative,
            module_roots,
            &stock_root_relative,
            #[cfg(any(target_os = "linux", target_os = "android"))]
            disable_umount,
        ) {
            warn!("failed to restore child mount {mount_point}: {:#}", e);
        }
    }
    Ok(())
}
