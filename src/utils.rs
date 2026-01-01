// Copyright 2025 Meta-Hybrid Mount Authors
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    ffi::CString,
    fmt as std_fmt,
    fs::{self, File, create_dir_all, remove_dir_all, remove_file, write},
    io::Write,
    os::unix::{
        ffi::OsStrExt,
        fs::{FileTypeExt, MetadataExt, PermissionsExt, symlink},
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::OnceLock,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};
#[cfg(any(target_os = "linux", target_os = "android"))]
use extattr::{Flags as XattrFlags, lgetxattr, llistxattr, lsetxattr};
use procfs::process::Process;
use regex_lite::Regex;
use rustix::{
    fs::ioctl_ficlone,
    mount::{MountFlags, mount},
};
use tracing::{Event, Subscriber};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{
    EnvFilter,
    fmt::{self, FmtContext, FormatEvent, FormatFields},
    layer::SubscriberExt,
    registry::LookupSpan,
    util::SubscriberInitExt,
};

use crate::defs::{self, TMPFS_CANDIDATES};

const SELINUX_XATTR: &str = "security.selinux";
const OVERLAY_OPAQUE_XATTR: &str = "trusted.overlay.opaque";
const CONTEXT_SYSTEM: &str = "u:object_r:system_file:s0";
const CONTEXT_VENDOR: &str = "u:object_r:vendor_file:s0";
const OVERLAY_TEST_XATTR: &str = "trusted.overlay.test";

#[allow(dead_code)]
const XATTR_TEST_FILE: &str = ".xattr_test";

static MODULE_ID_REGEX: OnceLock<Regex> = OnceLock::new();

struct SimpleFormatter;

impl<S, N> FormatEvent<S, N> for SimpleFormatter
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: fmt::format::Writer<'_>,
        event: &Event<'_>,
    ) -> std_fmt::Result {
        let level = *event.metadata().level();
        write!(writer, "[{}] ", level)?;
        ctx.field_format().format_fields(writer.by_ref(), event)?;
        writeln!(writer)
    }
}

pub fn init_logging(verbose: bool, log_path: &Path) -> Result<WorkerGuard> {
    if let Some(parent) = log_path.parent() {
        create_dir_all(parent)?;
    }
    let file_appender =
        tracing_appender::rolling::never(log_path.parent().unwrap(), log_path.file_name().unwrap());
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    let filter = if verbose {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("info")
    };

    let file_layer = fmt::layer()
        .with_ansi(false)
        .with_writer(non_blocking)
        .event_format(SimpleFormatter);

    tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .init();
    tracing_log::LogTracer::init().ok();

    let log_path_buf = log_path.to_path_buf();

    std::panic::set_hook(Box::new(move |info| {
        let msg = match info.payload().downcast_ref::<&str>() {
            Some(s) => *s,
            None => match info.payload().downcast_ref::<String>() {
                Some(s) => &s[..],
                None => "Box<Any>",
            },
        };

        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_default();

        let error_msg = format!("\n[ERROR] PANIC: Thread crashed at {}: {}\n", location, msg);

        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path_buf)
        {
            let _ = writeln!(file, "{}", error_msg);
        }

        eprintln!("{}", error_msg);
    }));

    Ok(guard)
}

pub fn validate_module_id(module_id: &str) -> Result<()> {
    let re = MODULE_ID_REGEX
        .get_or_init(|| Regex::new(r"^[a-zA-Z][a-zA-Z0-9._-]+$").expect("Invalid Regex pattern"));
    if re.is_match(module_id) {
        Ok(())
    } else {
        bail!("Invalid module ID: '{module_id}'. Must match /^[a-zA-Z][a-zA-Z0-9._-]+$/")
    }
}

pub fn check_zygisksu_enforce_status() -> bool {
    std::fs::read_to_string("/data/adb/zygisksu/denylist_enforce")
        .map(|s| s.trim() != "0")
        .unwrap_or(false)
}

fn copy_extended_attributes(src: &Path, dst: &Path) -> Result<()> {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        if let Ok(mut ctx) = lgetfilecon(src) {
            if ctx.contains("u:object_r:rootfs:s0") {
                ctx = CONTEXT_SYSTEM.to_string();
            }
            let _ = lsetfilecon(dst, &ctx);
        } else {
            let _ = lsetfilecon(dst, CONTEXT_SYSTEM);
        }
        if let Ok(opaque) = lgetxattr(src, OVERLAY_OPAQUE_XATTR) {
            let _ = lsetxattr(dst, OVERLAY_OPAQUE_XATTR, &opaque, XattrFlags::empty());
        }
        if let Ok(xattrs) = llistxattr(src) {
            for xattr_name in xattrs {
                let name_bytes = xattr_name.as_bytes();
                let name_str = String::from_utf8_lossy(name_bytes);

                #[allow(clippy::collapsible_if)]
                if name_str.starts_with("trusted.overlay.") && name_str != OVERLAY_OPAQUE_XATTR {
                    if let Ok(val) = lgetxattr(src, &xattr_name) {
                        let _ = lsetxattr(dst, &xattr_name, &val, XattrFlags::empty());
                    }
                }
            }
        }
    }
    Ok(())
}

pub fn lsetfilecon<P: AsRef<Path>>(path: P, con: &str) -> Result<()> {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        if let Err(e) = lsetxattr(
            path.as_ref(),
            SELINUX_XATTR,
            con.as_bytes(),
            XattrFlags::empty(),
        ) {
            let io_err = std::io::Error::from(e);
            log::debug!(
                "lsetfilecon: {} -> {} failed: {}",
                path.as_ref().display(),
                con,
                io_err
            );
        }
    }
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn lgetfilecon<P: AsRef<Path>>(path: P) -> Result<String> {
    let con = extattr::lgetxattr(path.as_ref(), SELINUX_XATTR).with_context(|| {
        format!(
            "Failed to get SELinux context for {}",
            path.as_ref().display()
        )
    })?;
    let con_str = String::from_utf8_lossy(&con).trim_matches('\0').to_string();

    Ok(con_str)
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn lgetfilecon<P: AsRef<Path>>(_path: P) -> Result<String> {
    Ok(CONTEXT_SYSTEM.to_string())
}

pub fn copy_path_context<S: AsRef<Path>, D: AsRef<Path>>(src: S, dst: D) -> Result<()> {
    let mut context = if src.as_ref().exists() {
        lgetfilecon(&src).unwrap_or_else(|_| CONTEXT_SYSTEM.to_string())
    } else {
        CONTEXT_SYSTEM.to_string()
    };

    if context.contains("u:object_r:rootfs:s0") {
        context = CONTEXT_SYSTEM.to_string();
    }

    lsetfilecon(dst, &context)
}

pub fn ensure_dir_exists<T: AsRef<Path>>(dir: T) -> Result<()> {
    if !dir.as_ref().exists() {
        create_dir_all(&dir)?;
    }
    Ok(())
}

pub fn camouflage_process(name: &str) -> Result<()> {
    let c_name = CString::new(name)?;
    unsafe {
        libc::prctl(libc::PR_SET_NAME, c_name.as_ptr() as u64, 0, 0, 0);
    }
    Ok(())
}

pub fn random_kworker_name() -> String {
    use std::{
        collections::hash_map::DefaultHasher,
        hash::{Hash, Hasher},
    };

    let mut hasher = DefaultHasher::new();
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .hash(&mut hasher);
    let hash = hasher.finish();
    let x = hash % 16;
    let y = (hash >> 4) % 10;
    format!("kworker/u{}:{}", x, y)
}

#[allow(dead_code)]
pub fn is_xattr_supported(path: &Path) -> bool {
    let test_file = path.join(XATTR_TEST_FILE);
    if let Err(e) = write(&test_file, b"test") {
        log::debug!("XATTR Check: Failed to create test file: {}", e);
        return false;
    }
    let result = lsetfilecon(&test_file, "u:object_r:system_file:s0");
    let supported = result.is_ok();
    let _ = remove_file(test_file);
    supported
}

pub fn is_overlay_xattr_supported(path: &Path) -> bool {
    let test_file = path.join(".overlay_xattr_test");
    if let Err(e) = write(&test_file, b"test") {
        log::debug!("XATTR Check: Failed to create test file: {}", e);
        return false;
    }

    let c_path = match CString::new(test_file.as_os_str().as_encoded_bytes()) {
        Ok(c) => c,
        Err(_) => {
            let _ = remove_file(&test_file);
            return false;
        }
    };
    let c_key = CString::new(OVERLAY_TEST_XATTR).unwrap();
    let c_val = CString::new("y").unwrap();

    let supported = unsafe {
        let ret = libc::lsetxattr(
            c_path.as_ptr(),
            c_key.as_ptr(),
            c_val.as_ptr() as *const libc::c_void,
            c_val.as_bytes().len(),
            0,
        );
        ret == 0
    };
    let _ = remove_file(test_file);
    supported
}

pub fn is_mounted<P: AsRef<Path>>(path: P) -> bool {
    let path_str = path.as_ref().to_string_lossy();
    let search = path_str.trim_end_matches('/');

    if let Ok(process) = Process::myself()
        && let Ok(mountinfo) = process.mountinfo()
    {
        return mountinfo
            .into_iter()
            .any(|m| m.mount_point.to_string_lossy() == search);
    }

    if let Ok(content) = fs::read_to_string("/proc/mounts") {
        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() > 1 && parts[1] == search {
                return true;
            }
        }
    }
    false
}

pub fn mount_tmpfs(target: &Path, source: &str) -> Result<()> {
    ensure_dir_exists(target)?;
    let data = CString::new("mode=0755")?;
    mount(
        source,
        target,
        "tmpfs",
        MountFlags::empty(),
        data.as_c_str(),
    )
    .context("Failed to mount tmpfs")?;
    Ok(())
}

pub fn mount_image(image_path: &Path, target: &Path) -> Result<()> {
    ensure_dir_exists(target)?;
    lsetfilecon(image_path, "u:object_r:ksu_file:s0").ok();
    let status = Command::new("mount")
        .args(["-t", "ext4", "-o", "loop,rw,noatime"])
        .arg(image_path)
        .arg(target)
        .status()
        .context("Failed to execute mount command")?;

    if !status.success() {
        bail!("Mount command failed");
    }
    Ok(())
}

pub fn repair_image(image_path: &Path) -> Result<()> {
    log::info!("Running e2fsck on {}", image_path.display());
    let status = Command::new("e2fsck")
        .args(["-y", "-f"])
        .arg(image_path)
        .status()
        .context("Failed to execute e2fsck")?;

    if let Some(code) = status.code()
        && code > 2
    {
        bail!("e2fsck failed with exit code: {}", code);
    }
    Ok(())
}

pub fn reflink_or_copy(src: &Path, dest: &Path) -> Result<u64> {
    let src_file = File::open(src)?;
    let dest_file = File::create(dest)?;

    if ioctl_ficlone(&dest_file, &src_file).is_ok() {
        let metadata = src_file.metadata()?;
        let len = metadata.len();
        dest_file.set_permissions(metadata.permissions())?;
        return Ok(len);
    }
    drop(dest_file);
    drop(src_file);
    fs::copy(src, dest).map_err(|e| e.into())
}

fn make_device_node(path: &Path, mode: u32, rdev: u64) -> Result<()> {
    let c_path = CString::new(path.as_os_str().as_encoded_bytes())?;
    let dev = rdev as libc::dev_t;
    unsafe {
        if libc::mknod(c_path.as_ptr(), mode, dev) != 0 {
            let err = std::io::Error::last_os_error();
            bail!("mknod failed for {}: {}", path.display(), err);
        }
    }
    Ok(())
}

fn get_context_for_path(path: &Path) -> &'static str {
    let path_str = path.to_string_lossy();
    if path_str.starts_with("/vendor") || path_str.starts_with("/odm") {
        return CONTEXT_VENDOR;
    }
    CONTEXT_SYSTEM
}

fn apply_system_context(current: &Path, relative: &Path) -> Result<()> {
    if let Some(name) = current.file_name().and_then(|n| n.to_str())
        && (name == "upperdir" || name == "workdir")
        && let Some(parent) = current.parent()
        && let Ok(ctx) = lgetfilecon(parent)
    {
        return lsetfilecon(current, &ctx);
    }

    let system_path = Path::new("/").join(relative);

    if system_path.exists() {
        copy_path_context(&system_path, current)?;
    } else if let Some(parent) = system_path.parent()
        && parent.exists()
    {
        copy_path_context(parent, current)?;
    } else {
        let target_context = get_context_for_path(&system_path);
        lsetfilecon(current, target_context)?;
    }
    Ok(())
}

fn native_cp_r(src: &Path, dst: &Path, relative: &Path, repair: bool) -> Result<()> {
    if !dst.exists() {
        if src.is_dir() {
            create_dir_all(dst)?;
        }
        if let Ok(src_meta) = src.metadata() {
            let _ = fs::set_permissions(dst, src_meta.permissions());
        }

        if repair && relative.as_os_str().is_empty() {
            let _ = apply_system_context(dst, relative);
        } else if !repair {
            let _ = copy_extended_attributes(src, dst);
        }
    }

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let file_name = entry.file_name();
        let dst_path = dst.join(&file_name);
        let next_relative = relative.join(&file_name);

        let metadata = entry.metadata()?;
        let ft = metadata.file_type();

        if ft.is_dir() {
            native_cp_r(&src_path, &dst_path, &next_relative, repair)?;
        } else if ft.is_symlink() {
            if dst_path.exists() {
                remove_file(&dst_path)?;
            }
            let link_target = fs::read_link(&src_path)?;
            symlink(&link_target, &dst_path)?;
        } else if ft.is_char_device() || ft.is_block_device() || ft.is_fifo() {
            if dst_path.exists() {
                remove_file(&dst_path)?;
            }
            let mode = metadata.permissions().mode();
            let rdev = metadata.rdev();
            make_device_node(&dst_path, mode, rdev)?;
        } else {
            reflink_or_copy(&src_path, &dst_path)?;
        }

        if repair {
            let _ = apply_system_context(&dst_path, &next_relative);
        } else {
            let _ = copy_extended_attributes(&src_path, &dst_path);
        }
    }
    Ok(())
}

pub fn sync_dir(src: &Path, dst: &Path, repair_context: bool) -> Result<()> {
    if !src.exists() {
        return Ok(());
    }
    ensure_dir_exists(dst)?;
    native_cp_r(src, dst, Path::new(""), repair_context).with_context(|| {
        format!(
            "Failed to natively sync {} to {}",
            src.display(),
            dst.display()
        )
    })
}

fn is_ok_empty<P: AsRef<Path>>(dir: P) -> bool {
    if !dir.as_ref().exists() {
        return false;
    }
    dir.as_ref()
        .read_dir()
        .is_ok_and(|mut entries| entries.next().is_none())
}

pub fn select_temp_dir() -> Result<PathBuf> {
    for path_str in TMPFS_CANDIDATES {
        let path = Path::new(path_str);
        if is_ok_empty(path) {
            log::info!("Selected dynamic temp root: {}", path.display());
            return Ok(path.to_path_buf());
        }
    }
    let run_dir = Path::new(defs::RUN_DIR);
    ensure_dir_exists(run_dir)?;
    let work_dir = run_dir.join("workdir");
    Ok(work_dir)
}

#[allow(dead_code)]
pub fn cleanup_temp_dir(temp_dir: &Path) {
    if let Err(e) = remove_dir_all(temp_dir) {
        log::warn!(
            "Failed to clean up temp dir {}: {:#}",
            temp_dir.display(),
            e
        );
    }
}

#[allow(dead_code)]
pub fn ensure_temp_dir(temp_dir: &Path) -> Result<()> {
    if temp_dir.exists() {
        remove_dir_all(temp_dir).ok();
    }
    create_dir_all(temp_dir)?;
    Ok(())
}

pub fn is_erofs_supported() -> bool {
    fs::read_to_string("/proc/filesystems")
        .map(|content| content.contains("erofs"))
        .unwrap_or(false)
}

pub fn create_erofs_image(src_dir: &Path, image_path: &Path) -> Result<()> {
    let mkfs_bin = Path::new("/data/adb/metamodule/tools/mkfs.erofs");
    let cmd_name = if mkfs_bin.exists() {
        mkfs_bin.as_os_str()
    } else {
        std::ffi::OsStr::new("mkfs.erofs")
    };

    log::info!("Packing EROFS image: {}", image_path.display());

    let output = Command::new(cmd_name)
        .arg("-z")
        .arg("lz4hc")
        .arg(image_path)
        .arg(src_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .context("Failed to execute mkfs.erofs")?;

    let log_lines = |bytes: &[u8]| {
        let s = String::from_utf8_lossy(bytes);
        for line in s.lines() {
            if !line.trim().is_empty() {
                log::debug!("{}", line);
            }
        }
    };

    log_lines(&output.stdout);
    log_lines(&output.stderr);

    if !output.status.success() {
        bail!("Failed to create EROFS image");
    }

    log::info!("Build Completed.");
    let _ = fs::set_permissions(image_path, fs::Permissions::from_mode(0o644));
    lsetfilecon(image_path, "u:object_r:ksu_file:s0")?;
    Ok(())
}

pub fn mount_erofs_image(image_path: &Path, target: &Path) -> Result<()> {
    ensure_dir_exists(target)?;
    lsetfilecon(image_path, "u:object_r:ksu_file:s0").ok();
    let status = Command::new("mount")
        .args(["-t", "erofs", "-o", "loop,ro,nodev,noatime"])
        .arg(image_path)
        .arg(target)
        .status()
        .context("Failed to execute mount command for EROFS")?;

    if !status.success() {
        bail!("EROFS Mount command failed");
    }
    Ok(())
}

pub fn extract_module_id(path: &Path) -> Option<String> {
    path.parent()
        .and_then(|p| p.file_name())
        .map(|s| s.to_string_lossy().to_string())
}
