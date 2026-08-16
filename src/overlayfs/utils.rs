// ReHybrid-Mount
//
// SPDX-License-Identifier: GPL-3.0-only

//! OverlayFS 与 ext4 的底层挂载原语(仅 Linux/Android)。
//!
//! `umount_dir` 是**立即卸载**(rustix `unmount` 系统调用),
//! 与 KernelSU try-umount 列表注册严格区分。
//!
//! Stage 3 脚手架:部分原语在 Stage 5 CLI 接入前暂未被调用;
//! 接入完成后移除本豁免,恢复 dead_code 检查。
#![allow(dead_code)]

use std::os::unix::fs::MetadataExt;
use std::path::Path;

use loopdev::LoopControl;
use rustix::fd::AsFd;
use rustix::fs::CWD;
use rustix::mount::{
    FsMountFlags, FsOpenFlags, MountAttrFlags, MountFlags, MoveMountFlags, UnmountFlags,
    fsconfig_create, fsconfig_set_string, fsmount, fsopen, mount, move_mount, unmount,
};

use crate::errors::{Error, Result};
use crate::sys::fs::check_kernel_config;

/// 检查内核是否编译了 overlayfs(`CONFIG_OVERLAY_FS=y`)。
pub fn is_overlay_supported() -> Result<bool> {
    check_kernel_config("CONFIG_OVERLAY_FS")
}

/// 立即卸载路径(空 flags)。用于回滚与清理,不是注册列表。
pub fn umount_dir(path: &Path) -> Result<()> {
    unmount(path, UnmountFlags::empty())
        .map_err(|err| Error::msg(format!("unmount {}: {err}", path.display())))
}

/// fsopen("overlay") 主路径:fsconfig → fsmount → move_mount。
pub fn fsopen_mount(
    upperdir: Option<String>,
    workdir: Option<String>,
    lowerdir_config: String,
    source: &str,
    dest: &Path,
) -> Result<()> {
    let fs = fsopen("overlay", FsOpenFlags::FSOPEN_CLOEXEC)
        .map_err(|err| Error::msg(format!("fsopen overlay: {err}")))?;

    fsconfig_set_string(&fs, "lowerdir", &lowerdir_config)
        .map_err(|err| Error::msg(format!("fsconfig lowerdir {lowerdir_config}: {err}")))?;

    if let (Some(upperdir), Some(workdir)) = (&upperdir, &workdir) {
        fsconfig_set_string(&fs, "upperdir", upperdir)
            .map_err(|err| Error::msg(format!("fsconfig upperdir {upperdir}: {err}")))?;
        fsconfig_set_string(&fs, "workdir", workdir)
            .map_err(|err| Error::msg(format!("fsconfig workdir {workdir}: {err}")))?;
    }

    fsconfig_set_string(&fs, "source", source)
        .map_err(|err| Error::msg(format!("fsconfig source {source}: {err}")))?;
    fsconfig_create(&fs).map_err(|err| Error::msg(format!("fsconfig create: {err}")))?;

    let mount_handle = fsmount(&fs, FsMountFlags::FSMOUNT_CLOEXEC, MountAttrFlags::empty())
        .map_err(|err| Error::msg(format!("fsmount overlay: {err}")))?;

    move_mount(
        mount_handle.as_fd(),
        "",
        CWD,
        dest,
        MoveMountFlags::MOVE_MOUNT_F_EMPTY_PATH,
    )
    .map_err(|err| Error::msg(format!("move overlay mount to {}: {err}", dest.display())))
}

/// 挂载 ext4 镜像(loop 设备 + autoclear,v4.2.0 行为)。
pub fn mount_ext4(source: &Path, target: &Path) -> Result<()> {
    if !source.exists() {
        log::warn!("ext4 source does not exist: {}", source.display());
    } else {
        let metadata = std::fs::metadata(source)?;
        let permissions = metadata.permissions();
        if permissions.readonly() {
            log::debug!(
                "ext4 image permissions(octal): {:o}",
                metadata.mode() & 0o777
            );
        }
    }

    mount_ext4_loop(source, target)
}

fn mount_ext4_loop(source: &Path, target: &Path) -> Result<()> {
    let loop_control =
        LoopControl::open().map_err(|err| Error::msg(format!("open loop control: {err}")))?;
    let loop_device = loop_control
        .next_free()
        .map_err(|err| Error::msg(format!("find free loop device: {err}")))?;

    loop_device
        .with()
        .read_only(false)
        .autoclear(true)
        .attach(source)
        .map_err(|err| {
            Error::msg(format!(
                "attach loop device for {}: {err}",
                source.display()
            ))
        })?;

    let device_path = loop_device
        .path()
        .ok_or_else(|| Error::msg("get loop device path: no path available"))?;
    log::debug!("loop device: path={}", device_path.display());

    match mount(&device_path, target, "ext4", MountFlags::NOATIME, Some(c"")) {
        Ok(()) => Ok(()),
        Err(err) => {
            log::warn!(
                "ext4 mount failed, detaching loop device: device={}, error={err}",
                device_path.display()
            );
            if let Err(detach_err) = loop_device.detach() {
                log::error!(
                    "detach loop device failed: device={}, error={detach_err}",
                    device_path.display()
                );
            }
            Err(Error::msg(format!(
                "mount ext4 {} at {}: {err}",
                device_path.display(),
                target.display()
            )))
        }
    }
}
