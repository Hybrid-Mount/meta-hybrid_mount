// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

#[cfg(any(target_os = "linux", target_os = "android"))]
use std::{ffi::CString, os::fd::AsRawFd};
use std::{
    fs,
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::{conf::schema::KasumiConfig, defs, sys::kasumi};

#[derive(Debug, Clone, Serialize, Default, PartialEq, Eq)]
pub struct LkmStatus {
    pub loaded: bool,
    pub module_name: Option<String>,
    pub autoload: bool,
    pub kmi_override: String,
    pub current_kmi: String,
    pub search_dir: PathBuf,
    pub module_file: PathBuf,
}

#[cfg(all(
    any(target_os = "linux", target_os = "android"),
    any(target_arch = "aarch64", target_arch = "arm")
))]
const SYS_FINIT_MODULE_NUM: libc::c_long = 379;
#[cfg(all(
    any(target_os = "linux", target_os = "android"),
    any(target_arch = "x86_64", target_arch = "x86")
))]
const SYS_FINIT_MODULE_NUM: libc::c_long = 313;

#[cfg(all(
    any(target_os = "linux", target_os = "android"),
    target_arch = "aarch64"
))]
const SYS_DELETE_MODULE_NUM: libc::c_long = 106;
#[cfg(all(
    any(target_os = "linux", target_os = "android"),
    any(target_arch = "x86_64", target_arch = "x86")
))]
const SYS_DELETE_MODULE_NUM: libc::c_long = 176;
#[cfg(all(any(target_os = "linux", target_os = "android"), target_arch = "arm"))]
const SYS_DELETE_MODULE_NUM: libc::c_long = 129;

fn read_first_line(path: &Path) -> Result<String> {
    let content =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let line = content
        .lines()
        .next()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .with_context(|| format!("{} does not contain a value", path.display()))?;
    Ok(line.to_string())
}

fn arch_suffix() -> &'static str {
    #[cfg(target_arch = "aarch64")]
    {
        "_arm64"
    }
    #[cfg(target_arch = "arm")]
    {
        "_armv7"
    }
    #[cfg(target_arch = "x86_64")]
    {
        "_x86_64"
    }
    #[cfg(target_arch = "x86")]
    {
        "_x86"
    }
}

fn parse_kmi_from_release(release: &str) -> Result<String> {
    let full_version = release.trim();
    if full_version.is_empty() {
        bail!("kernel release is empty");
    }

    let dot1 = full_version
        .find('.')
        .context("kernel release has no major/minor separator")?;
    let dot2 = full_version[dot1 + 1..]
        .find('.')
        .map(|offset| dot1 + 1 + offset)
        .unwrap_or(full_version.len());
    let major_minor = &full_version[..dot2];

    let android_pos = full_version
        .find("-android")
        .context("kernel release has no Android version")?;
    let ver_start = android_pos + "-android".len();
    let ver_end = full_version[ver_start..]
        .find('-')
        .map(|offset| ver_start + offset)
        .unwrap_or(full_version.len());
    let android_ver = &full_version[ver_start..ver_end];

    if android_ver.is_empty() {
        bail!("kernel release has an empty Android version");
    }
    Ok(format!("android{}-{}", android_ver, major_minor))
}

fn real_kernel_release() -> Result<String> {
    read_first_line(Path::new("/proc/sys/kernel/osrelease"))
}

pub fn current_kmi() -> Result<String> {
    parse_kmi_from_release(&real_kernel_release()?)
}

fn effective_kmi(config: &KasumiConfig) -> Result<String> {
    if !config.lkm_kmi_override.trim().is_empty() {
        Ok(config.lkm_kmi_override.trim().to_string())
    } else {
        current_kmi()
    }
}

fn resolve_module_file(config: &KasumiConfig) -> Result<PathBuf> {
    if !config.lkm_dir.is_dir() {
        bail!(
            "Kasumi LKM directory does not exist: {}",
            config.lkm_dir.display()
        );
    }
    let kmi = effective_kmi(config)?;
    let path = config
        .lkm_dir
        .join(format!("{kmi}{}_kasumi_lkm.ko", arch_suffix()));
    if !path.is_file() {
        bail!(
            "canonical Kasumi LKM file does not exist: {}",
            path.display()
        );
    }
    Ok(path)
}

fn loaded_module_name() -> Result<Option<String>> {
    let content = fs::read_to_string("/proc/modules").context("failed to read /proc/modules")?;
    Ok(content.lines().find_map(|line| {
        let name = line.split_whitespace().next()?;
        (name == defs::KASUMI_LKM_MODULE_NAME).then(|| name.to_string())
    }))
}

pub fn is_loaded() -> Result<bool> {
    Ok(loaded_module_name()?.is_some())
}

pub fn status(config: &KasumiConfig) -> Result<LkmStatus> {
    let module_name = loaded_module_name()?;
    Ok(LkmStatus {
        loaded: module_name.is_some(),
        module_name,
        autoload: config.lkm_autoload,
        kmi_override: config.lkm_kmi_override.clone(),
        current_kmi: current_kmi()?,
        search_dir: config.lkm_dir.clone(),
        module_file: resolve_module_file(config)?,
    })
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn load_module_via_finit(ko_path: &Path, params: &str) -> Result<()> {
    let file = fs::File::open(ko_path)
        .with_context(|| format!("failed to open module {}", ko_path.display()))?;
    let params = CString::new(params).context("module params contain interior NUL")?;

    let ret = unsafe { libc::syscall(SYS_FINIT_MODULE_NUM, file.as_raw_fd(), params.as_ptr(), 0) };
    if ret != 0 {
        let err = std::io::Error::last_os_error();
        if err.raw_os_error() == Some(libc::EEXIST) {
            return Ok(());
        }
        return Err(err).with_context(|| format!("finit_module failed for {}", ko_path.display()));
    }

    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn load_module_via_finit(_ko_path: &Path, _params: &str) -> Result<()> {
    bail!("kernel module loading is only supported on linux/android")
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn unload_module_via_syscall(module_name: &str) -> Result<()> {
    let module_name = CString::new(module_name).context("module name contains interior NUL")?;
    let ret = unsafe { libc::syscall(SYS_DELETE_MODULE_NUM, module_name.as_ptr(), 0) };
    if ret != 0 {
        return Err(std::io::Error::last_os_error()).context("delete_module failed");
    }
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn unload_module_via_syscall(_module_name: &str) -> Result<()> {
    bail!("kernel module unloading is only supported on linux/android")
}

pub fn load(config: &KasumiConfig) -> Result<()> {
    if is_loaded()? {
        kasumi::invalidate_status_cache()?;
        return Ok(());
    }

    let ko_path = resolve_module_file(config)?;

    let params = String::new();
    load_module_via_finit(&ko_path, &params)?;

    kasumi::invalidate_status_cache()?;
    crate::scoped_log!(
        info,
        "lkm",
        "load complete: file={}, kmi={}",
        ko_path.display(),
        effective_kmi(config)?
    );
    Ok(())
}

pub fn unload(_config: &KasumiConfig) -> Result<()> {
    let Some(module_name) = loaded_module_name()? else {
        kasumi::release_connection()?;
        return Ok(());
    };

    kasumi::set_enabled(false)?;
    kasumi::clear_rules()?;
    kasumi::release_connection()?;
    thread::sleep(Duration::from_millis(120));

    let mut last_retry_error = None;
    for _ in 0..5 {
        match unload_module_via_syscall(&module_name) {
            Ok(()) => {
                kasumi::invalidate_status_cache()?;
                crate::scoped_log!(info, "lkm", "unload complete: module={}", module_name);
                return Ok(());
            }
            Err(err) => {
                let retryable = err
                    .downcast_ref::<std::io::Error>()
                    .and_then(|io_err| io_err.raw_os_error())
                    .is_some_and(|code| code == libc::EAGAIN || code == libc::EBUSY);
                last_retry_error = Some(err);
                if !retryable {
                    break;
                }
                thread::sleep(Duration::from_millis(120));
            }
        }
    }

    let err = last_retry_error.expect("delete_module retry loop always executes");
    Err(err)
}

pub fn autoload_if_needed(config: &KasumiConfig) -> Result<bool> {
    if !config.enabled
        || !config.lkm_autoload
        || is_loaded()?
        || kasumi::check_status()? == kasumi::KasumiStatus::KernelNotSupported
    {
        return Ok(false);
    }

    load(config)?;
    Ok(true)
}
