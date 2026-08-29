// SPDX-License-Identifier: GPL-3.0-only

//! 挂载系统辅助(仅 Linux/Android):挂载点探测、tmpfs 挂载、镜像修复。
//!
//! `unmount` 语义:本文件所有“卸载”都是 rustix `unmount` 系统调用,
//! 即立即执行;与 KernelSU try-umount 列表注册不是一回事。

use std::path::Path;
use std::process::Command;

use procfs::process::Process;
use rustix::mount::{MountFlags, UnmountFlags, mount, unmount};

use crate::errors::{Error, Result};
use crate::sys::mountinfo::MountSnapshot;
use crate::utils::ensure_dir_exists;

/// 从 `/proc/self/mountinfo` 判断路径是否为挂载点。
pub fn is_mounted(path: &Path) -> Result<bool> {
    Ok(MountSnapshot::read()?.contains(path))
}

/// Drop 清理路径的 best-effort 探测:查询失败记录原因并按未挂载处理,
/// 正常路径必须使用返回错误的 [`is_mounted`]。
pub fn is_mounted_best_effort(path: &Path) -> bool {
    match is_mounted(path) {
        Ok(mounted) => mounted,
        Err(err) => {
            log::warn!(
                "mount probe failed, assuming unmounted: path={}, error={err}",
                path.display()
            );
            false
        }
    }
}

/// 挂载 tmpfs(`mode=0755`),用于 overlay staging(v4.2.0 行为)。
pub fn mount_tmpfs(target: &Path, source: &str) -> Result<()> {
    ensure_dir_exists(target)?;
    mount(
        source,
        target,
        c"tmpfs",
        MountFlags::empty(),
        Some(c"mode=0755"),
    )
    .map_err(|err| {
        Error::msg(format!(
            "mount tmpfs {} at {}: {err}",
            source,
            target.display()
        ))
    })
}

/// 用 `e2fsck -y -f` 修复镜像;退出码 0..=3 视为成功(v4.2.0 行为)。
pub fn repair_image(image_path: &Path) -> Result<()> {
    let status = Command::new("e2fsck")
        .args(["-y", "-f"])
        .arg(image_path)
        .status()
        .map_err(|err| Error::msg(format!("execute e2fsck {}: {err}", image_path.display())))?;

    match status.code() {
        Some(code) if code > 3 => Err(Error::msg(format!(
            "e2fsck failed for {} with exit code {code}",
            image_path.display()
        ))),
        None => Err(Error::msg(format!(
            "e2fsck terminated by signal for {}",
            image_path.display()
        ))),
        Some(_) => Ok(()),
    }
}

/// `emulated-soft-reboot`:立即卸载 mountinfo 中 source 为指定值的所有挂载点
/// (参考项目行为,用于模拟软重启前的挂载清理)。
pub fn emulated_soft_reboot(source: &str) -> Result<()> {
    let process =
        Process::myself().map_err(|err| Error::msg(format!("get self process: {err}")))?;
    let mountinfo = process
        .mountinfo()
        .map_err(|err| Error::msg(format!("get mountinfo: {err}")))?;

    let mut mount_points = mountinfo
        .into_iter()
        .filter(|entry| entry.mount_source.as_deref() == Some(source))
        .map(|entry| entry.mount_point)
        .collect::<Vec<_>>();
    mount_points.sort_by(|left, right| {
        right
            .components()
            .count()
            .cmp(&left.components().count())
            .then_with(|| right.cmp(left))
    });

    for mount_point in mount_points {
        log::debug!(
            "unmounting {} from {source} in emulated-soft-reboot",
            mount_point.display()
        );
        unmount(&mount_point, UnmountFlags::DETACH)
            .map_err(|err| Error::msg(format!("unmount {}: {err}", mount_point.display())))?;
    }
    Ok(())
}
