use std::{
    collections::HashSet,
    ffi::CString,
    fmt as std_fmt,
    fs::{self, create_dir_all, remove_dir_all, remove_file, write, File},
    io::Write,
    os::fd::RawFd,
    os::unix::fs::symlink,
    path::{Path, PathBuf},
    process::Command,
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{bail, Context, Result};
use procfs::process::Process;
use regex_lite::Regex;
use rustix::{
    fs::ioctl_ficlone,
    mount::{mount, MountFlags},
};
use tracing::{Event, Subscriber};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{
    fmt::{self, FmtContext, FormatEvent, FormatFields},
    layer::SubscriberExt,
    registry::LookupSpan,
    util::SubscriberInitExt,
    EnvFilter,
};

use crate::defs::{self, TMPFS_CANDIDATES};

#[cfg(any(target_os = "linux", target_os = "android"))]
use nix::ioctl_write_ptr_bad;

#[cfg(any(target_os = "linux", target_os = "android"))]
use extattr::{lsetxattr, Flags as XattrFlags};

const SELINUX_XATTR: &str = "security.selinux";

#[allow(dead_code)]
const XATTR_TEST_FILE: &str = ".xattr_test";
const DEFAULT_CONTEXT: &str = "u:object_r:system_file:s0";

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

/// Validate `module_id` format and security
/// Module ID must match: ^[a-zA-Z][a-zA-Z0-9._-]+$
/// - Must start with a letter (a-zA-Z)
/// - Followed by one or more alphanumeric, dot, underscore, or hyphen characters
/// - Minimum length: 2 characters
pub fn validate_module_id(module_id: &str) -> Result<()> {
    let re = MODULE_ID_REGEX
        .get_or_init(|| Regex::new(r"^[a-zA-Z][a-zA-Z0-9._-]+$").expect("Invalid Regex pattern"));

    if re.is_match(module_id) {
        Ok(())
    } else {
        bail!("Invalid module ID: '{module_id}'. Must match /^[a-zA-Z][a-zA-Z0-9._-]+$/")
    }
}

pub fn lsetfilecon<P: AsRef<Path>>(path: P, con: &str) -> Result<()> {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        if let Err(e) = lsetxattr(&path, SELINUX_XATTR, con, XattrFlags::empty()) {
            let io_err = std::io::Error::from(e);
            log::debug!(
                "lsetfilecon: {} -> {} failed: {}",
                path.as_ref().display(),
                con,
                io_err
            );
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        let _ = path;
        let _ = con;
    }
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn lgetfilecon<P: AsRef<Path>>(path: P) -> Result<String> {
    let con = extattr::lgetxattr(&path, SELINUX_XATTR).with_context(|| {
        format!(
            "Failed to get SELinux context for {}",
            path.as_ref().display()
        )
    })?;
    Ok(String::from_utf8_lossy(&con).to_string())
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn lgetfilecon<P: AsRef<Path>>(_path: P) -> Result<String> {
    Ok(DEFAULT_CONTEXT.to_string())
}

pub fn copy_path_context<S: AsRef<Path>, D: AsRef<Path>>(src: S, dst: D) -> Result<()> {
    let context = if src.as_ref().exists() {
        lgetfilecon(&src).unwrap_or_else(|_| DEFAULT_CONTEXT.to_string())
    } else {
        DEFAULT_CONTEXT.to_string()
    };
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
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
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

pub fn is_mounted<P: AsRef<Path>>(path: P) -> bool {
    let path_str = path.as_ref().to_string_lossy();
    let search = path_str.trim_end_matches('/');

    if let Ok(process) = Process::myself() {
        if let Ok(mountinfo) = process.mountinfo() {
            return mountinfo
                .into_iter()
                .any(|m| m.mount_point.to_string_lossy() == search);
        }
    }

    // Fallback if procfs fails for some reason
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
    if let Some(code) = status.code() {
        if code > 2 {
            bail!("e2fsck failed with exit code: {}", code);
        }
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
        tracing::trace!("Reflink success: {:?} -> {:?}", src, dest);
        return Ok(len);
    }

    drop(dest_file);
    drop(src_file);

    tracing::trace!("Reflink failed (fallback to copy): {:?} -> {:?}", src, dest);
    fs::copy(src, dest).map_err(|e| e.into())
}

fn native_cp_r(src: &Path, dst: &Path) -> Result<()> {
    if !dst.exists() {
        create_dir_all(dst)?;
        let src_meta = src.metadata()?;
        fs::set_permissions(dst, src_meta.permissions())?;
        lsetfilecon(dst, DEFAULT_CONTEXT)?;
    }
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ft = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if ft.is_dir() {
            native_cp_r(&src_path, &dst_path)?;
        } else if ft.is_symlink() {
            let link_target = fs::read_link(&src_path)?;
            if dst_path.exists() {
                remove_file(&dst_path)?;
            }
            symlink(&link_target, &dst_path)?;
            let _ = lsetfilecon(&dst_path, DEFAULT_CONTEXT);
        } else {
            reflink_or_copy(&src_path, &dst_path)?;
            lsetfilecon(&dst_path, DEFAULT_CONTEXT)?;
        }
    }
    Ok(())
}

pub fn sync_dir(src: &Path, dst: &Path) -> Result<()> {
    if !src.exists() {
        return Ok(());
    }
    ensure_dir_exists(dst)?;
    native_cp_r(src, dst).with_context(|| {
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

const KSU_INSTALL_MAGIC1: u32 = 0xDEADBEEF;
const KSU_INSTALL_MAGIC2: u32 = 0xCAFEBABE;

const KSU_IOCTL_NUKE_EXT4_SYSFS: u32 = 0x40004b11;
const KSU_IOCTL_ADD_TRY_UMOUNT: u32 = 0x40004b12;

static DRIVER_FD: OnceLock<RawFd> = OnceLock::new();

#[cfg(any(target_os = "linux", target_os = "android"))]
static SENT_UNMOUNTS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

#[repr(C)]
struct KsuAddTryUmount {
    arg: u64,
    flags: u32,
    mode: u8,
}
#[repr(C)]
struct NukeExt4SysfsCmd {
    arg: u64,
}

#[cfg(any(target_os = "linux", target_os = "android"))]
ioctl_write_ptr_bad!(
    ksu_add_try_umount,
    KSU_IOCTL_ADD_TRY_UMOUNT,
    KsuAddTryUmount
);

#[cfg(any(target_os = "linux", target_os = "android"))]
ioctl_write_ptr_bad!(
    ksu_nuke_ext4_sysfs,
    KSU_IOCTL_NUKE_EXT4_SYSFS,
    NukeExt4SysfsCmd
);

fn grab_fd() -> i32 {
    let mut fd = -1;
    unsafe {
        libc::syscall(
            libc::SYS_reboot,
            KSU_INSTALL_MAGIC1,
            KSU_INSTALL_MAGIC2,
            0,
            &mut fd,
        );
    };
    fd
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn send_unmountable<P>(target: P) -> Result<()>
where
    P: AsRef<Path>,
{
    let path_ref = target.as_ref();
    let path_str = path_ref.to_string_lossy().to_string();
    if path_str.is_empty() {
        return Ok(());
    }
    let cache = SENT_UNMOUNTS.get_or_init(|| Mutex::new(HashSet::new()));
    let mut set = cache.lock().unwrap();
    if set.contains(&path_str) {
        log::debug!("Unmount skipped (dedup): {}", path_str);
        return Ok(());
    }
    set.insert(path_str.clone());
    let path = CString::new(path_str)?;
    let cmd = KsuAddTryUmount {
        arg: path.as_ptr() as u64,
        flags: 2,
        mode: 1,
    };
    let fd = *DRIVER_FD.get_or_init(grab_fd);
    if fd < 0 {
        return Ok(());
    }

    unsafe {
        ksu_add_try_umount(fd, &cmd)?;
    }
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn ksu_nuke_sysfs(target: &str) -> Result<()> {
    let c_path = CString::new(target)?;
    let cmd = NukeExt4SysfsCmd {
        arg: c_path.as_ptr() as u64,
    };
    let fd = *DRIVER_FD.get_or_init(grab_fd);
    if fd < 0 {
        bail!("KSU driver not available");
    }

    unsafe {
        ksu_nuke_ext4_sysfs(fd, &cmd).context("KSU Nuke Sysfs ioctl failed")?;
    }
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn ksu_nuke_sysfs(_target: &str) -> Result<()> {
    bail!("Not supported on this OS")
}
