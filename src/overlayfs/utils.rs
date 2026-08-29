// SPDX-License-Identifier: GPL-3.0-only

//! OverlayFS 与 ext4 的底层挂载原语(仅 Linux/Android)。

use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::thread;
use std::time::Duration;

use loopdev::LoopControl;
use rustix::fd::AsFd;
use rustix::fs::CWD;
use rustix::mount::{
    FsMountFlags, FsOpenFlags, MountAttrFlags, MountFlags, MoveMountFlags, fsconfig_create,
    fsconfig_set_string, fsmount, fsopen, mount, move_mount,
};

use crate::errors::{Error, Result};
use crate::sys::fs::check_kernel_config;

/// EBUSY 重试上限与指数退避(loop attach/mount/detach)。
const EBUSY_MAX_RETRIES: usize = 3;
const EBUSY_BASE_BACKOFF: Duration = Duration::from_millis(50);

fn io_error_is_busy(err: &io::Error) -> bool {
    err.raw_os_error() == Some(rustix::io::Errno::BUSY.raw_os_error())
}

fn errno_is_busy(err: &rustix::io::Errno) -> bool {
    err.raw_os_error() == rustix::io::Errno::BUSY.raw_os_error()
}

fn retry_ebusy_io<T>(operation: &str, mut action: impl FnMut() -> io::Result<T>) -> io::Result<T> {
    let mut last_error = None;
    for retry in 0..=EBUSY_MAX_RETRIES {
        match action() {
            Ok(value) => return Ok(value),
            Err(err) if io_error_is_busy(&err) && retry < EBUSY_MAX_RETRIES => {
                let backoff = EBUSY_BASE_BACKOFF.saturating_mul(1_u32 << retry);
                log::warn!(
                    "{operation} busy, retry={}/{} backoff_ms={}",
                    retry + 1,
                    EBUSY_MAX_RETRIES,
                    backoff.as_millis()
                );
                thread::sleep(backoff);
                last_error = Some(err);
            }
            Err(err) => return Err(err),
        }
    }
    Err(last_error.unwrap_or_else(|| io::Error::other("busy retries exhausted")))
}

fn retry_ebusy_errno<T>(
    operation: &str,
    mut action: impl FnMut() -> rustix::io::Result<T>,
) -> rustix::io::Result<T> {
    let mut last_error = None;
    for retry in 0..=EBUSY_MAX_RETRIES {
        match action() {
            Ok(value) => return Ok(value),
            Err(err) if errno_is_busy(&err) && retry < EBUSY_MAX_RETRIES => {
                let backoff = EBUSY_BASE_BACKOFF.saturating_mul(1_u32 << retry);
                log::warn!(
                    "{operation} busy, retry={}/{} backoff_ms={}",
                    retry + 1,
                    EBUSY_MAX_RETRIES,
                    backoff.as_millis()
                );
                thread::sleep(backoff);
                last_error = Some(err);
            }
            Err(err) => return Err(err),
        }
    }
    Err(last_error.unwrap_or(rustix::io::Errno::BUSY))
}

/// 检查内核是否编译了 overlayfs(`CONFIG_OVERLAY_FS=y`)。
pub fn is_overlay_supported() -> Result<bool> {
    check_kernel_config("CONFIG_OVERLAY_FS")
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

/// 已 attach 的 loop 设备句柄。成功挂载后由 `StorageHandle` 持有，
/// teardown 时先 unmount 再显式 detach；mount 失败路径在此函数内清理。
#[derive(Debug)]
pub struct Ext4LoopMount {
    device: loopdev::LoopDevice,
}

impl Ext4LoopMount {
    pub fn detach(&self) -> io::Result<()> {
        self.device.detach()
    }
}

/// 挂载 ext4 镜像(loop 设备 + autoclear,v4.2.0 行为)。
pub fn mount_ext4(source: &Path, target: &Path) -> Result<Ext4LoopMount> {
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

fn mount_ext4_loop(source: &Path, target: &Path) -> Result<Ext4LoopMount> {
    let loop_control =
        LoopControl::open().map_err(|err| Error::msg(format!("open loop control: {err}")))?;
    let loop_device = loop_control
        .next_free()
        .map_err(|err| Error::msg(format!("find free loop device: {err}")))?;

    retry_ebusy_io("attach loop device", || {
        loop_device
            .with()
            .read_only(false)
            .autoclear(true)
            .attach(source)
    })
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

    match retry_ebusy_errno("mount ext4 staging", || {
        mount(&device_path, target, "ext4", MountFlags::NOATIME, Some(c""))
    }) {
        Ok(()) => Ok(Ext4LoopMount {
            device: loop_device,
        }),
        Err(err) => {
            log::warn!(
                "ext4 mount failed, detaching loop device: device={}, error={err}",
                device_path.display()
            );
            if let Err(detach_err) = retry_ebusy_io("detach loop device", || loop_device.detach()) {
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
