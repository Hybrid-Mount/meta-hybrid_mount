// SPDX-License-Identifier: GPL-3.0-only

//! Shared mount sources and temporary directories for tests.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::config::Mode;
use crate::module_id::ModuleId;
use crate::mount_tree::{MountSource, NodeFileType};

pub fn mount_source(
    module: &str,
    relative: &str,
    file_type: NodeFileType,
    backend: Mode,
) -> MountSource {
    mount_source_at(
        module,
        relative,
        file_type,
        backend,
        PathBuf::from(format!("/data/adb/modules/{module}/{relative}")),
    )
}

pub fn mount_source_at(
    module: &str,
    relative: &str,
    file_type: NodeFileType,
    backend: Mode,
    source_path: PathBuf,
) -> MountSource {
    MountSource {
        module_id: ModuleId::try_from(module).unwrap(),
        relative: relative.to_owned(),
        source_path,
        file_type,
        replace: false,
        backend,
    }
}

/// Owns a unique temporary directory and removes it even when a test panics.
pub struct Fixture(PathBuf);

impl Fixture {
    pub fn new(tag: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        loop {
            let id = NEXT.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir()
                .join(format!("hybrid-mount-{tag}-{}-{id}", std::process::id()));
            match std::fs::create_dir(&root) {
                Ok(()) => return Self(root),
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(err) => panic!("create fixture {}: {err}", root.display()),
            }
        }
    }
}

impl std::ops::Deref for Fixture {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<Path> for Fixture {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Returns false only for an explicitly requested unprivileged test run.
#[cfg(target_os = "linux")]
pub fn require_mount_namespace() -> bool {
    use rustix::mount::{MountPropagationFlags, mount_change};

    // SAFETY: only the calling test thread changes namespaces; no shared Rust data is affected.
    let result = unsafe { rustix::thread::unshare_unsafe(rustix::thread::UnshareFlags::NEWNS) }
        .and_then(|()| {
            mount_change(
                "/",
                MountPropagationFlags::PRIVATE | MountPropagationFlags::REC,
            )
        });
    match result {
        Ok(()) => true,
        Err(err) if std::env::var_os("HYBRID_MOUNT_UNPRIVILEGED_TESTS").is_some() => {
            eprintln!(
                "SKIPPED mount test: {err}; HYBRID_MOUNT_UNPRIVILEGED_TESTS is set (use --nocapture to show skips)"
            );
            false
        }
        Err(err) => panic!("private mount namespace requires CAP_SYS_ADMIN: {err}"),
    }
}
