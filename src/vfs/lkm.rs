// SPDX-License-Identifier: GPL-3.0-only

//! 加载 Hybrid Mount 自有的 VFS 内核模块（K2）。
//!
//! DDK 为每个 Android/GKI 目标构建一个 hybridmount-<android>-<kernel>.ko
//! （见 .github/workflows/kernel-module.yml）。这里按内核 release 与 Android 版本
//! 精确匹配后加载：先写熔断标记，再尝试 insmod，最后以 key type 是否响应为准裁决。
//!
//! 加载失败不是致命错误：调用方会重新探测，失败后按 vfs_strict 降级或报错，与
//! 设备本来就没有 K2 走同一条路径。

use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::defs;
use crate::errors::Result;
use crate::sys::process::{CaptureMode, CommandSpec, run_command};
use crate::vfs::backend::{KeyringKernel, LkmLoader, VfsKernel};
use crate::vfs::lkm_target::{
    android_major_from_kernel_release, parse_android_major, select_lkm_filename,
};
use crate::vfs::sys::KeyringChannel;

const KERNEL_RELEASE_PATH: &str = "/proc/sys/kernel/osrelease";
const MODULE_NAME: &str = "hybridmount";
/// 覆盖选择结果，供真机调试未打包的目标。
const LKM_OVERRIDE_ENV: &str = "HYBRID_MOUNT_VFS_LKM_PATH";
/// getprop 是辅助路径，短超时后降级。
const GETPROP_TIMEOUT: Duration = Duration::from_secs(10);
const INSMOD_TIMEOUT: Duration = Duration::from_secs(30);
const RMMOD_TIMEOUT: Duration = Duration::from_secs(30);

/// 生产用加载器：选择、加载并校验打包的 K2 模块。
pub struct VfsLkmLoader;

impl LkmLoader for VfsLkmLoader {
    fn load_hm_vfs(&self) -> Result<()> {
        if let Err(err) = load() {
            log::warn!("vfs kernel module was not loaded: {err}");
        }
        Ok(())
    }
}

fn load() -> std::result::Result<(), String> {
    let lkm_path = select_lkm_path()?;
    if !lkm_path.is_file() {
        return Err(format!(
            "no bundled VFS module for this kernel (expected {})",
            lkm_path.display()
        ));
    }

    let _attempt = LkmAttemptGuard::arm(&lkm_path)?;
    let mut attempts = Vec::new();
    for (program, applet) in insmod_candidates() {
        let mut args = Vec::new();
        if let Some(applet) = applet {
            args.push(applet.to_owned());
        }
        args.push(lkm_path.display().to_string());

        let spec = CommandSpec::new(program)
            .operation("load the Hybrid Mount VFS kernel module")
            .args(args)
            .capture(CaptureMode::Stderr)
            .timeout(INSMOD_TIMEOUT);

        match run_command(&spec) {
            Ok(outcome) => {
                // insmod 的退出码不是权威依据：模块可能已由上次启动加载，或 init
                // 之后才失败。以 key type 是否响应为准。
                if module_responds() {
                    return Ok(());
                }
                let stderr = outcome.stderr_text().unwrap_or_default();
                attempts.push(format!(
                    "{program}: status={}, stderr={}",
                    outcome.status,
                    stderr.trim()
                ));
            }
            Err(err) => attempts.push(format!("{program}: {err}")),
        }
    }

    // 探测失败时模块可能处于半初始化状态，尽力卸载，避免留下一个坏的实现。
    unload();
    Err(format!(
        "{MODULE_NAME} did not become available; attempts: {}",
        attempts.join("; ")
    ))
}

fn insmod_candidates() -> [(&'static str, Option<&'static str>); 4] {
    [
        ("/system/bin/insmod", None),
        ("/data/adb/ap/bin/busybox", Some("insmod")),
        ("/data/adb/ksu/bin/busybox", Some("insmod")),
        ("insmod", None),
    ]
}

fn rmmod_candidates() -> [(&'static str, Option<&'static str>); 4] {
    [
        ("/system/bin/rmmod", None),
        ("/data/adb/ap/bin/busybox", Some("rmmod")),
        ("/data/adb/ksu/bin/busybox", Some("rmmod")),
        ("rmmod", None),
    ]
}

/// K2 是否已经响应：以 key type 为准，而不是 insmod 的退出码。
fn module_responds() -> bool {
    match KeyringKernel::new(KeyringChannel::Hybridmount) {
        Ok(mut kernel) => kernel.version().is_ok(),
        Err(_) => false,
    }
}

fn unload() {
    for (program, applet) in rmmod_candidates() {
        let mut args = Vec::new();
        if let Some(applet) = applet {
            args.push(applet.to_owned());
        }
        args.push(MODULE_NAME.to_owned());

        let spec = CommandSpec::new(program)
            .operation("unload the Hybrid Mount VFS kernel module")
            .args(args)
            .capture(CaptureMode::Stderr)
            .any_exit_status()
            .timeout(RMMOD_TIMEOUT);

        if run_command(&spec).is_ok() {
            return;
        }
    }
}

fn select_lkm_path() -> std::result::Result<PathBuf, String> {
    if let Some(path) = std::env::var_os(LKM_OVERRIDE_ENV).filter(|path| !path.is_empty()) {
        return Ok(PathBuf::from(path));
    }

    if std::env::consts::ARCH != "aarch64" {
        return Err(format!(
            "the bundled VFS module is aarch64-only, running architecture is {}",
            std::env::consts::ARCH
        ));
    }

    let release = fs::read_to_string(KERNEL_RELEASE_PATH)
        .map_err(|err| format!("read kernel release: {err}"))?;
    let android_major = android_major_from_kernel_release(&release).or_else(device_android_major);
    let file_name = select_lkm_filename(release.trim(), android_major).ok_or_else(|| {
        format!(
            "no bundled VFS module for kernel={} android={}",
            release.trim(),
            android_major
                .map(|value| value.to_string())
                .unwrap_or_else(|| "unknown".to_owned())
        )
    })?;

    Ok(Path::new(defs::VFS_LKM_DIR).join(file_name))
}

fn device_android_major() -> Option<u32> {
    for program in ["/system/bin/getprop", "getprop"] {
        let spec = CommandSpec::new(program)
            .operation("read Android version")
            .arg("ro.build.version.release")
            .capture(CaptureMode::Stdout)
            .timeout(GETPROP_TIMEOUT);

        let Ok(outcome) = run_command(&spec) else {
            continue;
        };
        if let Some(major) = outcome
            .stdout_text()
            .as_deref()
            .and_then(parse_android_major)
        {
            return Some(major);
        }
    }
    None
}

/// 加载前写下的熔断标记：Drop 即清除，硬崩溃则遗留，下次启动据此拒绝自动重试。
#[derive(Debug)]
struct LkmAttemptGuard {
    marker_path: PathBuf,
}

impl LkmAttemptGuard {
    fn arm(lkm_path: &Path) -> std::result::Result<Self, String> {
        let marker_path = Path::new(defs::VFS_LKM_BOOT_GUARD_PATH);
        if let Some(parent) = marker_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)
                .map_err(|err| format!("create VFS LKM guard directory: {err}"))?;
        }

        let mut marker = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(marker_path)
            .map_err(|err| {
                if err.kind() == ErrorKind::AlreadyExists {
                    format!(
                        "previous VFS module load did not complete; refusing automatic retry. Verify the kernel ABI, then remove {}",
                        marker_path.display()
                    )
                } else {
                    format!("create VFS LKM boot guard: {err}")
                }
            })?;

        let guard = Self {
            marker_path: marker_path.to_path_buf(),
        };
        writeln!(marker, "lkm={}", lkm_path.display())
            .map_err(|err| format!("write VFS LKM boot guard: {err}"))?;
        marker
            .sync_all()
            .map_err(|err| format!("sync VFS LKM boot guard: {err}"))?;
        Ok(guard)
    }
}

impl Drop for LkmAttemptGuard {
    fn drop(&mut self) {
        if let Err(err) = fs::remove_file(&self.marker_path) {
            log::warn!("failed to clear the VFS LKM boot guard: {err}");
        }
    }
}
