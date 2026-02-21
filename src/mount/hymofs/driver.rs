use std::{
    collections::HashSet,
    ffi::CString,
    fs::File,
    os::fd::{AsFd, AsRawFd},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use log::{error, info, warn};
use regex_lite::Regex;
use rustix::system::finit_module;
use walkdir::WalkDir;

use crate::{
    conf::config,
    core::state::HymofsState,
    mount::hymofs::ioctl::{
        HymoSpoofUname, HymoSyscallArg, get_hymofs_fd, hymo_ioc_add_merge_rule, hymo_ioc_add_rule,
        hymo_ioc_get_features, hymo_ioc_get_version, hymo_ioc_set_debug, hymo_ioc_set_enabled,
        hymo_ioc_set_mirror_path, hymo_ioc_set_stealth, hymo_ioc_set_uname,
    },
};

fn parse_kmi(version: &str) -> Result<String> {
    let re = Regex::new(r"(.* )?(\d+\.\d+)(\S+)?(android\d+)(.*)")?;
    let cap = re
        .captures(version)
        .ok_or_else(|| anyhow::anyhow!("Failed to get KMI from boot/modules"))?;
    let android_version = cap.get(4).map_or("", |m| m.as_str());
    let kernel_version = cap.get(2).map_or("", |m| m.as_str());
    Ok(format!("{android_version}-{kernel_version}"))
}

pub fn load_kernel_module() -> Result<()> {
    let kmi = {
        let uname = rustix::system::uname();
        let version = uname.release().to_string_lossy();
        parse_kmi(&version)
    }?;

    let ko_path =
        Path::new("/data/adb/modules/hybrid_mount/lkm/").join(format!("{}_hymofs_lkm.ko", kmi));

    if !ko_path.exists() {
        warn!("HymoFS LKM not found at {:?}", ko_path);
        return Ok(());
    }

    info!("Loading HymoFS LKM from {:?}", ko_path);
    let file = File::open(&ko_path)?;
    let args = CString::new("hymo_syscall_nr=142")?;

    match finit_module(file.as_fd(), &args, 0) {
        Ok(_) => info!("HymoFS LKM loaded successfully"),
        Err(e) => error!("Failed to load HymoFS LKM: {}", e),
    }

    Ok(())
}

pub fn check_hymofs_status() -> HymofsState {
    let mut state = HymofsState::default();

    let fd = match get_hymofs_fd(142) {
        Ok(fd) => fd,
        Err(e) => {
            state.loaded = false;
            state.error_msg = Some(format!("Failed to get fd: {}", e));
            return state;
        }
    };

    let raw_fd = fd.as_raw_fd();
    let mut version: i32 = 0;

    unsafe {
        match hymo_ioc_get_version(raw_fd, &mut version) {
            Ok(_) => {
                state.loaded = true;
                state.version = version;
            }
            Err(e) => {
                state.loaded = false;
                state.error_msg = Some(format!("Failed to get version: {}", e));
            }
        }

        let mut features: i32 = 0;
        if hymo_ioc_get_features(raw_fd, &mut features).is_ok() {
            let mut active = Vec::new();
            if features & 1 != 0 {
                active.push("kstat_spoof".to_string());
            }
            if features & 2 != 0 {
                active.push("uname_spoof".to_string());
            }
            if features & 4 != 0 {
                active.push("cmdline_spoof".to_string());
            }
            if features & 16 != 0 {
                active.push("selinux_bypass".to_string());
            }
            if features & 32 != 0 {
                active.push("merge_dir".to_string());
            }
            state.active_features = active;
        }
    }

    state
}

pub fn apply_hymofs_rules(
    ids: &HashSet<String>,
    config: &config::Config,
    storage_root: &Path,
) -> Result<Vec<String>> {
    let fd = get_hymofs_fd(142).context("Failed to get hymofs fd")?;
    let raw_fd = fd.as_raw_fd();

    info!("Applying HymoFS configuration");

    unsafe {
        let debug_val = if config.hymofs.debug { 1 } else { 0 };
        if let Err(e) = hymo_ioc_set_debug(raw_fd, &debug_val) {
            error!("Failed to set HymoFS debug mode: {}", e);
        }

        let stealth_val = if config.hymofs.stealth { 1 } else { 0 };
        if let Err(e) = hymo_ioc_set_stealth(raw_fd, &stealth_val) {
            error!("Failed to set HymoFS stealth mode: {}", e);
        }

        let mut uname_arg = HymoSpoofUname {
            sysname: [0; 65],
            nodename: [0; 65],
            release: [0; 65],
            version: [0; 65],
            machine: [0; 65],
            domainname: [0; 65],
            err: 0,
        };

        if config.hymofs.spoof_uname.enable {
            let copy_to_c_array = |src: &str, dst: &mut [libc::c_char; 65]| {
                let bytes = src.as_bytes();
                let len = bytes.len().min(64);
                for i in 0..len {
                    dst[i] = bytes[i] as libc::c_char;
                }
                dst[len] = 0;
            };

            copy_to_c_array(&config.hymofs.spoof_uname.sysname, &mut uname_arg.sysname);
            copy_to_c_array(&config.hymofs.spoof_uname.nodename, &mut uname_arg.nodename);
            copy_to_c_array(&config.hymofs.spoof_uname.release, &mut uname_arg.release);
            copy_to_c_array(&config.hymofs.spoof_uname.version, &mut uname_arg.version);
            copy_to_c_array(&config.hymofs.spoof_uname.machine, &mut uname_arg.machine);
            copy_to_c_array(
                &config.hymofs.spoof_uname.domainname,
                &mut uname_arg.domainname,
            );
        }

        if let Err(e) = hymo_ioc_set_uname(raw_fd, &uname_arg) {
            error!("Failed to set HymoFS uname spoofing: {}", e);
        }

        let mirror_dir = storage_root.join("hymofs");
        if let Err(e) = std::fs::create_dir_all(&mirror_dir) {
            error!("Failed to create hymofs mirror dir: {}", e);
        }

        if let Ok(c_path) = CString::new(mirror_dir.to_string_lossy().as_bytes()) {
            let arg = HymoSyscallArg {
                src: c_path.as_ptr(),
                target: std::ptr::null(),
                type_: 0,
            };
            if let Err(e) = hymo_ioc_set_mirror_path(raw_fd, &arg) {
                error!("Failed to set HymoFS mirror path: {}", e);
            }
        }
    }

    let mut applied_ids = Vec::new();

    for id in ids {
        let module_dir = storage_root.join(id);
        if !module_dir.exists() {
            warn!("Module directory not found: {:?}", module_dir);
            continue;
        }

        info!("Processing rules for module: {}", id);
        let mut success = true;

        for entry in WalkDir::new(&module_dir).min_depth(1).into_iter().flatten() {
            let path = entry.path();
            let rel_path = match path.strip_prefix(&module_dir) {
                Ok(p) => p,
                Err(_) => continue,
            };
            let target_path = PathBuf::from("/").join(rel_path);

            let src_c = match CString::new(path.to_string_lossy().as_bytes()) {
                Ok(c) => c,
                Err(_) => {
                    error!("Invalid source path encoding: {:?}", path);
                    success = false;
                    continue;
                }
            };
            let target_c = match CString::new(target_path.to_string_lossy().as_bytes()) {
                Ok(c) => c,
                Err(_) => {
                    error!("Invalid target path encoding: {:?}", target_path);
                    success = false;
                    continue;
                }
            };

            let arg = HymoSyscallArg {
                src: src_c.as_ptr(),
                target: target_c.as_ptr(),
                type_: if path.is_dir() { 1 } else { 0 },
            };

            unsafe {
                if path.is_dir() {
                    if let Err(e) = hymo_ioc_add_merge_rule(raw_fd, &arg) {
                        error!("Failed to add merge rule for {:?}: {}", path, e);
                        success = false;
                    } else {
                        info!("Added merge rule: {:?}", path);
                    }
                } else if let Err(e) = hymo_ioc_add_rule(raw_fd, &arg) {
                    error!("Failed to add file rule for {:?}: {}", path, e);
                    success = false;
                } else {
                    info!("Added file rule: {:?}", path);
                }
            }
        }

        if success {
            info!("Successfully applied all rules for module: {}", id);
            applied_ids.push(id.clone());
        } else {
            warn!("Partial or failed rule application for module: {}", id);
        }
    }

    unsafe {
        let enabled_val = if config.hymofs.enable { 1 } else { 0 };
        if let Err(e) = hymo_ioc_set_enabled(raw_fd, &enabled_val) {
            error!("Failed to enable HymoFS: {}", e);
        } else {
            info!("HymoFS enabled state set to: {}", config.hymofs.enable);
        }
    }

    Ok(applied_ids)
}
