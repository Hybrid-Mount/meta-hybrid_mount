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

/// Rollback one mount target: deepest descendants first, empty-flags unmount
/// with a lazy fallback. Final equivalence against the pre-execution snapshot
/// is verified by the pipeline after all actions have run.
pub fn rollback_mount_target(path: &Path) -> Result<()> {
    let snapshot = MountSnapshot::read()?;
    let mut targets = snapshot.descendants(path);
    targets.push(path);

    for target in targets {
        if crate::sys::faults::should_fail_next_unmount_ebusy() {
            return Err(Error::msg(format!(
                "unmount {}: injected EBUSY",
                target.display()
            )));
        }
        match unmount(target, UnmountFlags::empty()) {
            Ok(()) => log::info!("mount rollback complete: target={}", target.display()),
            Err(err) if matches!(err, rustix::io::Errno::NOENT | rustix::io::Errno::INVAL) => {
                log::debug!(
                    "mount already gone: target={}, errno={err}",
                    target.display()
                );
            }
            Err(err) => {
                log::warn!(
                    "mount rollback busy, falling back to lazy unmount: target={}, errno={err}",
                    target.display()
                );
                unmount(target, UnmountFlags::DETACH).map_err(|detach_err| {
                    Error::msg(format!("lazy unmount {}: {detach_err}", target.display()))
                })?;
            }
        }
    }

    Ok(())
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

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use crate::sys::faults;
    use std::fs;
    use std::path::{Path, PathBuf};

    fn unshare_mount_namespace() -> bool {
        if unsafe { libc::unshare(libc::CLONE_NEWNS) } == 0 {
            true
        } else {
            eprintln!(
                "skipping mount namespace test: unshare failed: {}",
                std::io::Error::last_os_error()
            );
            false
        }
    }

    fn test_root(tag: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("hybrid-mount-mount-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root)
            .unwrap_or_else(|err| panic!("create test root {}: {err}", root.display()));
        root
    }

    fn cleanup_root(root: &Path) {
        let _ = unmount(root, UnmountFlags::DETACH);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rollback_mount_target_removes_nested_mounts_and_confirms() {
        if !unshare_mount_namespace() {
            return;
        }
        let root = test_root("rollback");
        let parent = root.join("parent");
        let child = parent.join("child");
        fs::create_dir_all(&child).unwrap();

        if mount("hybrid-test", &parent, c"tmpfs", MountFlags::empty(), None).is_err()
            || mount("hybrid-test", &child, c"tmpfs", MountFlags::empty(), None).is_err()
        {
            eprintln!("skipping rollback test: nested tmpfs mounts are unavailable");
            cleanup_root(&root);
            return;
        }

        assert!(rollback_mount_target(&parent).is_ok());
        let snapshot = MountSnapshot::read().unwrap();
        assert!(!snapshot.contains(&parent));
        assert!(!snapshot.contains(&child));

        cleanup_root(&root);
    }

    #[test]
    fn injected_ebusy_fails_rollback_with_target() {
        let _fault_guard = faults::test_lock();
        if !unshare_mount_namespace() {
            return;
        }
        let root = test_root("ebusy");
        let parent = root.join("parent");
        fs::create_dir_all(&parent).unwrap();
        if mount("hybrid-test", &parent, c"tmpfs", MountFlags::empty(), None).is_err() {
            eprintln!("skipping EBUSY injection test: tmpfs mount unavailable");
            cleanup_root(&root);
            return;
        }

        faults::enable_next_unmount_ebusy_failure();
        let err = rollback_mount_target(&parent).unwrap_err();
        let message = err.to_string();
        assert!(message.contains("injected EBUSY"), "{message}");
        assert!(message.contains(&parent.display().to_string()), "{message}");

        faults::reset();
        cleanup_root(&root);
    }

    #[test]
    fn injected_mountinfo_failure_propagates_from_rollback() {
        let _fault_guard = faults::test_lock();
        if !unshare_mount_namespace() {
            return;
        }
        faults::enable_mountinfo_read_failure();
        let err = rollback_mount_target(Path::new("/unused")).unwrap_err();
        assert!(err.to_string().contains("injected mountinfo"), "{err}");
        faults::reset();
    }
}
