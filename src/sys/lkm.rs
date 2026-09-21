// SPDX-License-Identifier: GPL-3.0-only

//! Shared loading machinery for the prebuilt kernel modules Hybrid Mount ships.
//!
//! Both bundled modules go through the same three steps: pick the `.ko` that matches
//! this kernel, write a boot guard so a crash mid-`insmod` is not retried automatically,
//! and try the `insmod` entry points available on the device. Only two things are
//! product-specific — the filename matrix, supplied by each caller, and the acceptance
//! test (`nuke` checks that the ext4 sysfs node disappeared, `hybridmount` checks that
//! the keyring key type answers).

use std::fs::{self, OpenOptions};
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};

use crate::sys::process::{CaptureMode, CommandSpec, run_command};
use crate::vfs::lkm_target::{android_major_from_kernel_release, device_android_major};

const KERNEL_RELEASE_PATH: &str = "/proc/sys/kernel/osrelease";

/// The kernel release this device is running, trimmed.
pub fn kernel_release() -> Result<String, String> {
    fs::read_to_string(KERNEL_RELEASE_PATH)
        .map(|release| release.trim().to_owned())
        .map_err(|err| format!("read kernel release: {err}"))
}

/// Whether a prebuilt module can be loaded on this device at all.
///
/// Every bundled `.ko` is aarch64-only, so no other architecture is worth probing.
pub fn bundled_lkm_arch_supported(arch: &str) -> bool {
    arch == "aarch64"
}

/// Resolves the `.ko` this device should load out of a bundled directory.
///
/// `directory` is where the release package put the modules, `env_var` is the
/// controlled-testing override, `product` names the module in operator-facing errors,
/// and `select` maps a kernel release plus Android label to a packaged filename.
pub fn select_bundled_lkm_path(
    directory: &str,
    env_var: &str,
    product: &str,
    select: impl Fn(&str, Option<u32>) -> Option<&'static str>,
) -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os(env_var).filter(|path| !path.is_empty()) {
        return Ok(PathBuf::from(path));
    }

    if !bundled_lkm_arch_supported(std::env::consts::ARCH) {
        return Err(format!(
            "bundled {product} is aarch64-only, running architecture is {}",
            std::env::consts::ARCH
        ));
    }

    let release = kernel_release()?;
    select_for_release(directory, product, &release, select)
}

/// The kernel-release half of [`select_bundled_lkm_path`], split out so it can be tested
/// against fixture releases instead of the host's `/proc/sys/kernel/osrelease`.
fn select_for_release(
    directory: &str,
    product: &str,
    release: &str,
    select: impl Fn(&str, Option<u32>) -> Option<&'static str>,
) -> Result<PathBuf, String> {
    let android_major = android_major_from_kernel_release(release).or_else(device_android_major);
    let file_name = select(release, android_major).ok_or_else(|| {
        format!(
            "no bundled {product} for kernel={} android={}",
            release,
            android_major
                .map(|value| value.to_string())
                .unwrap_or_else(|| "unknown".to_owned())
        )
    })?;

    Ok(Path::new(directory).join(file_name))
}

/// Shared VFS/nuke loading order: ksud, our built-in loader, then ordinary insmod.
/// The optional subcommand precedes the module path and parameters.
pub const INSMOD_CANDIDATES: &[(&str, Option<&str>)] = &[
    ("/data/adb/ksud", Some("insmod")),
    (
        "/data/adb/modules/hybrid_mount/hybrid-mount",
        Some("lkm-load"),
    ),
    ("/system/bin/insmod", None),
    ("/data/adb/ap/bin/busybox", Some("insmod")),
    ("/data/adb/ksu/bin/busybox", Some("insmod")),
    ("insmod", None),
];

/// `rmmod` entry points, same convention as [`INSMOD_CANDIDATES`].
pub const RMMOD_CANDIDATES: &[(&str, Option<&str>)] = &[
    ("/system/bin/rmmod", None),
    ("/data/adb/ap/bin/busybox", Some("rmmod")),
    ("/data/adb/ksu/bin/busybox", Some("rmmod")),
    ("rmmod", None),
];

/// What one attempted `insmod` reported, for the caller's error message.
pub struct InsmodAttempt {
    pub program: &'static str,
    pub status: String,
    pub stderr: String,
}

/// Builds the argument vector for one candidate: optional applet, then the module path
/// and any module parameters.
pub fn insmod_args(applet: Option<&str>, lkm_path: &Path, params: &[String]) -> Vec<String> {
    let mut args = Vec::with_capacity(params.len() + 2);
    if let Some(applet) = applet {
        args.push(applet.to_owned());
    }
    args.push(lkm_path.display().to_string());
    args.extend_from_slice(params);
    args
}

/// Builds the argument vector for one `rmmod` candidate.
pub fn rmmod_args(applet: Option<&str>, module_name: &str) -> Vec<String> {
    let mut args = Vec::with_capacity(2);
    if let Some(applet) = applet {
        args.push(applet.to_owned());
    }
    args.push(module_name.to_owned());
    args
}

/// Tries the supplied loader candidates until the caller confirms the module's effect.
///
/// The `insmod` exit code is never authoritative: an already-loaded module and a
/// deliberately self-unloading one both exit non-zero on the successful path. `accepted`
/// is therefore the only success criterion. Every failed attempt is collected so the
/// caller can report what was actually tried.
///
/// `params` are appended after the module path on every attempt.
pub fn load_with_candidates(
    candidates: impl IntoIterator<Item = (&'static str, Option<&'static str>)>,
    lkm_path: &Path,
    operation: &'static str,
    params: &[String],
    timeout: std::time::Duration,
    mut accepted: impl FnMut() -> bool,
) -> Result<(), Vec<InsmodAttempt>> {
    let mut attempts = Vec::new();

    for (program, applet) in candidates {
        let spec = CommandSpec::new(program)
            .operation(operation)
            .args(insmod_args(applet, lkm_path, params))
            .capture(CaptureMode::Stderr)
            // A non-zero exit can be the expected successful path, so the acceptance
            // probe decides rather than the status.
            .any_exit_status()
            .timeout(timeout);

        match run_command(&spec) {
            Ok(outcome) => {
                if accepted() {
                    log::info!("kernel module effect confirmed: loader={program}");
                    return Ok(());
                }
                attempts.push(InsmodAttempt {
                    program,
                    status: outcome.status.to_string(),
                    stderr: outcome.stderr_text().unwrap_or_default().trim().to_owned(),
                });
            }
            Err(err) => attempts.push(InsmodAttempt {
                program,
                status: "not runnable".to_owned(),
                stderr: err.to_string(),
            }),
        }
    }

    Err(attempts)
}

/// Renders the failed-attempt list for an error message.
pub fn describe_attempts(attempts: &[InsmodAttempt]) -> String {
    if attempts.is_empty() {
        return "no insmod candidate produced output".to_owned();
    }
    attempts
        .iter()
        .map(|attempt| {
            format!(
                "{}: status={}, stderr={}",
                attempt.program, attempt.status, attempt.stderr
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// Best-effort `rmmod` of a module, used to clear a half-initialised load.
pub fn unload(module_name: &str, operation: &'static str, timeout: std::time::Duration) {
    for (program, applet) in RMMOD_CANDIDATES {
        let spec = CommandSpec::new(*program)
            .operation(operation)
            .args(rmmod_args(*applet, module_name))
            .capture(CaptureMode::Stderr)
            .any_exit_status()
            .timeout(timeout);

        if run_command(&spec)
            .is_ok_and(|outcome| outcome.status == crate::sys::process::ExitStatus::Exited(0))
        {
            return;
        }
    }
}

/// Boot guard written before `insmod` and cleared on `Drop`.
///
/// A handled return clears it in both directions; only a hard crash leaves the marker
/// behind, which makes the next boot refuse an automatic retry.
#[derive(Debug)]
pub struct LoadAttemptGuard {
    marker_path: PathBuf,
}

impl LoadAttemptGuard {
    /// Arms the guard at the default location, syncing the directory entry.
    pub fn arm(marker_path: &Path, description: &str, payload: &str) -> Result<Self, String> {
        Self::arm_with_sync(
            marker_path,
            description,
            payload,
            crate::sys::fs::sync_parent_directory,
        )
    }

    /// Arms the guard with an injected directory sync, for failure-path tests.
    pub fn arm_with_sync(
        marker_path: &Path,
        description: &str,
        payload: &str,
        mut sync_parent: impl FnMut(&Path) -> io::Result<()>,
    ) -> Result<Self, String> {
        if let Some(parent) = marker_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(|err| {
                format!(
                    "create {description} guard directory {}: {err}",
                    parent.display()
                )
            })?;
            sync_parent(parent).map_err(|err| {
                format!(
                    "sync {description} guard directory entry {}: {err}",
                    parent.display()
                )
            })?;
        }

        let mut marker = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(marker_path)
            .map_err(|err| {
                if err.kind() == ErrorKind::AlreadyExists {
                    format!(
                        "previous {description} attempt did not complete; refusing automatic retry. Verify the kernel ABI, then remove {} to retry",
                        marker_path.display()
                    )
                } else {
                    format!("create {description} boot guard {}: {err}", marker_path.display())
                }
            })?;

        let guard = Self {
            marker_path: marker_path.to_path_buf(),
        };
        if let Err(err) = writeln!(marker, "{payload}") {
            drop(guard);
            return Err(format!(
                "write {description} boot guard {}: {err}",
                marker_path.display()
            ));
        }
        if let Err(err) = marker.sync_all() {
            drop(guard);
            return Err(format!(
                "sync {description} boot guard {}: {err}",
                marker_path.display()
            ));
        }
        if let Err(err) = sync_parent(marker_path) {
            drop(guard);
            return Err(format!(
                "sync {description} boot-guard parent for {}: {err}",
                marker_path.display()
            ));
        }

        Ok(guard)
    }
}

impl Drop for LoadAttemptGuard {
    fn drop(&mut self) {
        match fs::remove_file(&self.marker_path) {
            Ok(()) => {
                if let Err(err) = crate::sys::fs::sync_parent_directory(&self.marker_path) {
                    log::warn!(
                        "failed to persist boot guard removal {}: {err}",
                        self.marker_path.display()
                    );
                }
            }
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            Err(err) => log::warn!(
                "failed to clear boot guard {}: {err}",
                self.marker_path.display()
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[cfg(unix)]
    #[test]
    fn loader_fallback_can_succeed_after_insmod_failure_despite_nonzero_exit() {
        let root = crate::test_support::Fixture::new("ko-loader-fallback");
        let script = root.join("loader.sh");
        let ready = root.join("ready");
        fs::write(&script, "touch \"$1\"\nexit 11\n").unwrap();
        let candidates = [("false", None), ("sh", None)];
        let result = load_with_candidates(
            candidates,
            &script,
            "test loader fallback",
            &[ready.display().to_string()],
            std::time::Duration::from_secs(2),
            || ready.is_file(),
        );
        assert!(
            result.is_ok(),
            "{}",
            describe_attempts(&result.err().unwrap_or_default())
        );
    }

    #[cfg(unix)]
    #[test]
    fn zero_exit_without_provider_is_failure_and_keeps_loader_diagnostics() {
        let result = load_with_candidates(
            [("true", None), ("/nonexistent/hm-ko-loader", None)],
            Path::new("/m/hybridmount.ko"),
            "test absent provider",
            &[],
            std::time::Duration::from_secs(2),
            || false,
        );
        let attempts = result.expect_err("no provider must fail");
        assert_eq!(attempts.len(), 2);
        assert!(describe_attempts(&attempts).contains("/nonexistent/hm-ko-loader"));
    }

    #[cfg(unix)]
    #[test]
    fn responding_provider_stops_before_fallback() {
        let probes = Cell::new(0);
        let result = load_with_candidates(
            [("true", None), ("false", None)],
            Path::new("/m/hybridmount.ko"),
            "test ready provider",
            &[],
            std::time::Duration::from_secs(2),
            || {
                probes.set(probes.get() + 1);
                true
            },
        );
        assert!(result.is_ok());
        assert_eq!(probes.get(), 1);
    }

    #[test]
    fn candidate_arguments_place_the_applet_first_and_parameters_last() {
        let params = vec!["mount_point=/mnt".to_owned(), "symaddr=0x1".to_owned()];

        assert_eq!(
            insmod_args(Some("insmod"), Path::new("/m/nuke.ko"), &params),
            vec!["insmod", "/m/nuke.ko", "mount_point=/mnt", "symaddr=0x1"]
        );
        assert_eq!(
            insmod_args(None, Path::new("/m/nuke.ko"), &[]),
            vec!["/m/nuke.ko"]
        );
        assert_eq!(
            rmmod_args(Some("rmmod"), "hybridmount"),
            vec!["rmmod", "hybridmount"]
        );
        assert_eq!(rmmod_args(None, "hybridmount"), vec!["hybridmount"]);
    }

    #[test]
    fn guard_refuses_a_stale_marker_and_clears_on_drop() {
        let root = std::env::temp_dir().join(format!("hm-lkm-guard-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let marker = root.join("guard");

        let guard = LoadAttemptGuard::arm(&marker, "test", "lkm=/m/nuke.ko").expect("arm guard");
        assert!(marker.is_file());

        let err = LoadAttemptGuard::arm(&marker, "test", "lkm=/m/nuke.ko")
            .expect_err("a stale marker must refuse a retry");
        assert!(err.contains("refusing automatic retry"), "{err}");

        drop(guard);
        assert!(!marker.exists());
        fs::remove_dir(root).expect("remove fixture dir");
    }

    #[test]
    fn guard_aborts_and_unlinks_when_the_marker_parent_sync_fails() {
        let root = std::env::temp_dir().join(format!("hm-lkm-sync-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let marker = root.join("guard");
        let sync_calls = Cell::new(0);
        let marker_was_visible = Cell::new(false);

        let err = LoadAttemptGuard::arm_with_sync(&marker, "test", "lkm=/m/nuke.ko", |path| {
            let call = sync_calls.get() + 1;
            sync_calls.set(call);
            if call == 2 {
                marker_was_visible.set(path == marker && marker.is_file());
                return Err(io::Error::other("injected marker parent sync failure"));
            }
            Ok(())
        })
        .expect_err("a failed parent sync must abort the arm");

        assert!(
            sync_calls.get() == 2
                && marker_was_visible.get()
                && !marker.exists()
                && err.contains("guard parent"),
            "{err}"
        );
        fs::remove_dir(root).expect("remove fixture dir");
    }

    #[test]
    fn failed_attempts_are_reported_with_their_program() {
        let attempts = vec![
            InsmodAttempt {
                program: "/system/bin/insmod",
                status: "exit status: 1".to_owned(),
                stderr: "unknown symbol".to_owned(),
            },
            InsmodAttempt {
                program: "insmod",
                status: "not runnable".to_owned(),
                stderr: "not found".to_owned(),
            },
        ];

        let rendered = describe_attempts(&attempts);
        assert!(rendered.contains("/system/bin/insmod"), "{rendered}");
        assert!(rendered.contains("unknown symbol"), "{rendered}");
        assert!(
            rendered.contains("insmod: status=not runnable"),
            "{rendered}"
        );
        assert_eq!(
            describe_attempts(&[]),
            "no insmod candidate produced output".to_owned()
        );
    }
}
