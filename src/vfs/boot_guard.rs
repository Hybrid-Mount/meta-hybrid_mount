// SPDX-License-Identifier: GPL-3.0-only

//! The VFS boot guards: the persistent crash markers that gate every VFS mutation.
//!
//! A mutation arms the rule marker before it touches the kernel and clears it on any handled
//! return, so a kernel panic or a killed process leaves the marker behind and the next boot
//! skips the backend instead of repeating the crash.
//!
//! A marker therefore has to say who left it. Only the build that armed it can be blamed for
//! the interruption: an upgrade, and a rebuild of the same version, both replace the binary
//! that performed the injection, so a marker the running build cannot attribute to itself is
//! retired and the backend is tried again. The legacy one-byte marker carries no identity at
//! all and is retired the same way. A marker this build did write still stands, and only
//! `hybrid-mount vfs guard clear --yes` overrides it deliberately.
//!
//! The module-load marker (`vfs_lkm_boot_guard`) carries the same identity plus the content
//! identity of the kernel object the interrupted attempt was about to insert. A marker for a
//! candidate the packaged module no longer contains — a refreshed prebuilt `.ko`, or a marker
//! an older build left — is retired so the updated object is tried, while a marker for a
//! candidate this build still ships keeps refusing the automatic retry.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use crate::defs;
use crate::errors::{CausalError, ContextError, Error, Result};

/// The VFS mutation that armed a marker, recorded for the diagnostic the next boot reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum VfsMutationSource {
    /// The boot pipeline's rule injection.
    Boot,
    /// Cleanup of owned rules and isolated UIDs at boot or before a soft reboot.
    Cleanup,
    /// A runtime load/unload/reload.
    Runtime,
    /// A VFS kernel module load attempt.
    Module,
}

impl VfsMutationSource {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Boot => "boot",
            Self::Cleanup => "cleanup",
            Self::Runtime => "runtime",
            Self::Module => "module",
        }
    }
}

/// Identity of the build that armed a marker.
#[derive(Clone, Debug, PartialEq, Eq)]
struct GuardIdentity {
    version: String,
    binary: Option<String>,
    /// `<size>:<mtime>` of the kernel object one load attempt targets; only the module-load
    /// marker records one.
    object: Option<String>,
}

impl GuardIdentity {
    /// The running build: its packaged version plus the size and mtime of its executable.
    ///
    /// The version alone would treat a rebuilt release of the same version as the same
    /// implementation, which is exactly the case of a developer retrying a kernel fix.
    fn current() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            binary: executable_identity(),
            object: None,
        }
    }

    /// The running build, further narrowed to the kernel object of one load attempt.
    fn for_object(object: Option<String>) -> Self {
        Self {
            object,
            ..Self::current()
        }
    }
}

/// `<size>:<mtime>` of the running executable; `None` when it cannot be read.
fn executable_identity() -> Option<String> {
    let path = std::env::current_exe().ok()?;
    stat_identity(&fs::metadata(path).ok()?)
}

/// `<size>:<mtime>` of a kernel object a load attempt targets; `None` when it cannot be read.
fn file_identity(path: &Path) -> Option<String> {
    stat_identity(&fs::metadata(path).ok()?)
}

/// The content identity of one file: as much as its size and modification time tell.
fn stat_identity(metadata: &fs::Metadata) -> Option<String> {
    let seconds = metadata
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    Some(format!("{}:{seconds}", metadata.len()))
}

/// Kernel boot identity when the platform exposes one; only the diagnostic uses it.
#[cfg(any(target_os = "linux", target_os = "android"))]
fn current_boot_id() -> Option<String> {
    crate::runtime::device::boot_id().ok()
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn current_boot_id() -> Option<String> {
    None
}

/// What a marker says about the mutation that armed it, as far as it parsed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GuardRecord {
    /// Packaged version of the build that armed the marker.
    version: Option<String>,
    /// `<size>:<mtime>` of that build's executable.
    binary: Option<String>,
    /// `<size>:<mtime>` of the kernel object a load attempt targeted.
    object: Option<String>,
    /// Kernel boot identity the interrupted mutation ran under.
    boot_id: Option<String>,
    /// Which VFS mutation armed the marker.
    source: Option<String>,
}

impl GuardRecord {
    /// Parses `key=value` lines; unknown keys are ignored, an unparsable marker yields `None`.
    fn parse(text: &str) -> Option<Self> {
        let mut record = Self {
            version: None,
            binary: None,
            object: None,
            boot_id: None,
            source: None,
        };
        for line in text.lines() {
            let Some((key, value)) = line.trim().split_once('=') else {
                continue;
            };
            let value = value.trim();
            if value.is_empty() {
                continue;
            }
            match key.trim() {
                "version" => record.version = Some(value.to_owned()),
                "binary" => record.binary = Some(value.to_owned()),
                "object" => record.object = Some(value.to_owned()),
                "boot" => record.boot_id = Some(value.to_owned()),
                "source" => record.source = Some(value.to_owned()),
                _ => {}
            }
        }
        if record.version.is_none()
            && record.binary.is_none()
            && record.object.is_none()
            && record.boot_id.is_none()
            && record.source.is_none()
        {
            return None;
        }
        Some(record)
    }

    /// Whether this marker was written by `identity`'s build.
    fn matches(&self, identity: &GuardIdentity) -> bool {
        if self.version.as_deref() != Some(identity.version.as_str()) {
            return false;
        }
        let same_binary = match (&self.binary, &identity.binary) {
            // A build that cannot identify its own executable still owns its version.
            (_, None) => true,
            (Some(recorded), Some(current)) => recorded == current,
            (None, Some(_)) => false,
        };
        // Only a marker that records an object for another target is a different attempt: the
        // rule marker never records one, and neither does a load attempt that could not read
        // the kernel object.
        let same_object = match (&self.object, &identity.object) {
            (_, None) => true,
            (Some(recorded), Some(current)) => recorded == current,
            (None, Some(_)) => false,
        };
        same_binary && same_object
    }

    /// One line for a log entry or a status card.
    pub(crate) fn describe(&self) -> String {
        let mut parts = vec![format!(
            "version={}",
            self.version.as_deref().unwrap_or("unknown")
        )];
        for (key, value) in [
            ("binary", &self.binary),
            ("object", &self.object),
            ("boot", &self.boot_id),
            ("source", &self.source),
        ] {
            if let Some(value) = value {
                parts.push(format!("{key}={value}"));
            }
        }
        parts.join(", ")
    }
}

/// What the rule marker on disk means for a VFS mutation about to run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum VfsGuardVerdict {
    /// No marker: the mutation may arm one and run.
    Absent,
    /// This build armed the marker. It is the persistent crash guard and it stands.
    Own(GuardRecord),
    /// The marker cannot be attributed to this build's injection: the legacy one-byte
    /// marker, or one written by a different build. `record` is what could be read back.
    Foreign { record: Option<GuardRecord> },
}

/// One guard file as reported by `hybrid-mount vfs guard`.
#[derive(Clone, Debug)]
pub(crate) struct VfsGuardState {
    /// Stable name of the marker, used as an output key.
    pub(crate) name: &'static str,
    /// Absolute path of the marker file.
    pub(crate) path: &'static str,
    /// `absent`, `own`, `foreign`, `stale`, or `unattributed` for the marker without identity.
    pub(crate) verdict: &'static str,
    /// The marker body verbatim, when it is present.
    pub(crate) contents: Option<String>,
}

/// RAII guard for the VFS boot guard file: once armed, any handled return (Ok or Err)
/// clears it on Drop. Only a hard crash leaves it behind and trips the next boot.
#[derive(Debug)]
pub(crate) struct VfsBootGuard {
    path: PathBuf,
}

impl VfsBootGuard {
    /// Arms the marker for a mutation of `source` by the running build.
    pub(crate) fn arm(source: VfsMutationSource) -> Result<Self> {
        Self::arm_at(
            Path::new(defs::VFS_BOOT_GUARD_PATH),
            source,
            &GuardIdentity::current(),
        )
    }

    /// Arms the marker at `path` with the payload of `identity`.
    fn arm_at(path: &Path, source: VfsMutationSource, identity: &GuardIdentity) -> Result<Self> {
        let payload = marker_payload(source, identity);
        (|| -> Result<()> {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)?;
            file.write_all(payload.as_bytes())?;
            file.sync_all()?;
            crate::sys::fs::sync_parent_directory(path)?;
            Ok(())
        })()
        .map_err(|err| {
            let cause = match err {
                Error::Io(source) => CausalError::Io(source),
                other => CausalError::Message(other.to_string()),
            };
            contextual("write vfs boot guard", path, cause)
        })?;
        Ok(Self {
            path: path.to_path_buf(),
        })
    }
}

impl Drop for VfsBootGuard {
    fn drop(&mut self) {
        if let Err(err) = fs::remove_file(&self.path) {
            log::warn!("clear vfs boot guard failed: {err}");
        } else if let Err(err) = crate::sys::fs::sync_parent_directory(&self.path) {
            log::warn!("persist vfs boot guard removal failed: {err}");
        }
    }
}

/// The marker body: one `key=value` line per fact.
fn marker_payload(source: VfsMutationSource, identity: &GuardIdentity) -> String {
    let mut payload = format!("version={}\n", identity.version);
    if let Some(binary) = &identity.binary {
        payload.push_str(&format!("binary={binary}\n"));
    }
    if let Some(object) = &identity.object {
        payload.push_str(&format!("object={object}\n"));
    }
    if let Some(boot_id) = current_boot_id() {
        payload.push_str(&format!("boot={boot_id}\n"));
    }
    payload.push_str(&format!("source={}\n", source.as_str()));
    payload
}

/// The module-load marker body for one candidate: the build identity plus the kernel object the
/// attempt is about to insert, so the next boot can tell a retry from a replaced module.
pub(crate) fn lkm_attempt_payload(candidate: &Path) -> String {
    let identity = GuardIdentity::for_object(file_identity(candidate));
    let mut payload = marker_payload(VfsMutationSource::Module, &identity);
    payload.push_str(&format!("lkm={}\n", candidate.display()));
    payload
}

/// Reads the rule marker at the default path for the running build.
pub(crate) fn inspect() -> Result<VfsGuardVerdict> {
    inspect_at(
        Path::new(defs::VFS_BOOT_GUARD_PATH),
        &GuardIdentity::current(),
    )
}

/// Reads `path` and decides whether the marker there speaks for `identity`.
fn inspect_at(path: &Path, identity: &GuardIdentity) -> Result<VfsGuardVerdict> {
    let Some(text) = read_marker(path)? else {
        return Ok(VfsGuardVerdict::Absent);
    };
    match GuardRecord::parse(&text) {
        Some(record) if record.matches(identity) => Ok(VfsGuardVerdict::Own(record)),
        other => Ok(VfsGuardVerdict::Foreign { record: other }),
    }
}

/// Arms a marker for a mutation about to run, retiring a marker this build cannot attribute
/// to its own injection first.
///
/// A marker this build wrote is still standing, so the mutation is refused rather than
/// retried: clearing it is the deliberate way past it.
pub(crate) fn arm_for_mutation(source: VfsMutationSource) -> Result<VfsBootGuard> {
    match inspect()? {
        VfsGuardVerdict::Absent => {}
        VfsGuardVerdict::Foreign { record } => {
            log::warn!(
                "retiring a VFS boot guard this build cannot attribute to itself: {}",
                describe(record.as_ref())
            );
            clear_rule_guard()?;
        }
        VfsGuardVerdict::Own(record) => {
            return Err(Error::msg(format!(
                "VFS boot guard is present and this build wrote it ({}); clear it with 'hybrid-mount vfs guard clear --yes' to retry",
                record.describe()
            )));
        }
    }
    VfsBootGuard::arm(source)
}

/// Reports the rule-injection guard for the running build.
pub(crate) fn rule_guard() -> Result<VfsGuardState> {
    let path = Path::new(defs::VFS_BOOT_GUARD_PATH);
    let contents = read_marker(path)?;
    let verdict = match &contents {
        None => "absent",
        Some(text) => match GuardRecord::parse(text) {
            Some(record) if record.matches(&GuardIdentity::current()) => "own",
            _ => "foreign",
        },
    };
    Ok(VfsGuardState {
        name: "rules",
        path: defs::VFS_BOOT_GUARD_PATH,
        verdict,
        contents,
    })
}

/// Reports the module-load guard for the running build.
pub(crate) fn lkm_guard() -> Result<VfsGuardState> {
    lkm_guard_at(Path::new(defs::VFS_LKM_BOOT_GUARD_PATH))
}

/// Reports the module-load guard at `path` for the running build.
fn lkm_guard_at(path: &Path) -> Result<VfsGuardState> {
    let contents = read_marker(path)?;
    let verdict = match &contents {
        None => "absent",
        Some(text) => match GuardRecord::parse(text) {
            Some(record) if record.matches(&GuardIdentity::current()) => "own",
            Some(_) => "stale",
            None => "unattributed",
        },
    };
    Ok(VfsGuardState {
        name: "lkm",
        path: defs::VFS_LKM_BOOT_GUARD_PATH,
        verdict,
        contents,
    })
}

/// Whether `record` blames a kernel object the candidate set still ships.
///
/// A marker that records no object still blames this build: it was armed by this executable
/// before the object identity was recorded.
fn blames_packaged_object(record: &GuardRecord, candidates: &[PathBuf]) -> bool {
    match &record.object {
        None => true,
        Some(object) => candidates
            .iter()
            .any(|candidate| file_identity(candidate).as_deref() == Some(object.as_str())),
    }
}

/// Prepares the module-load marker for a search over `candidates`.
///
/// The search is refused when this build already left the marker for a candidate it still
/// ships: the previous attempt ended the kernel, and retrying it unattended would repeat the
/// crash. A marker from another build, or one for a kernel object the packaged module no longer
/// contains, is retired so the updated module is tried — refreshing the prebuilt `.ko` files
/// must not need a manual `rm` before VFS can come back.
pub(crate) fn prepare_lkm_guard(candidates: &[PathBuf]) -> Result<()> {
    prepare_lkm_guard_at(Path::new(defs::VFS_LKM_BOOT_GUARD_PATH), candidates)
}

/// Prepares the module-load marker at `path` for a search over `candidates`.
fn prepare_lkm_guard_at(path: &Path, candidates: &[PathBuf]) -> Result<()> {
    match inspect_at(path, &GuardIdentity::current())? {
        VfsGuardVerdict::Absent => {}
        VfsGuardVerdict::Own(record) if blames_packaged_object(&record, candidates) => {
            return Err(Error::msg(format!(
                "incomplete VFS module load attempt by this build ({}); remove {} (or run 'hybrid-mount vfs guard clear --yes') to retry",
                record.describe(),
                defs::VFS_LKM_BOOT_GUARD_PATH
            )));
        }
        VfsGuardVerdict::Own(record) => {
            log::warn!(
                "retiring a VFS module load guard for a kernel object this package replaced: {}",
                record.describe()
            );
            clear_at(path)?;
        }
        VfsGuardVerdict::Foreign { record } => {
            log::warn!(
                "retiring a VFS module load guard this build cannot attribute to itself: {}",
                describe(record.as_ref())
            );
            clear_at(path)?;
        }
    }
    Ok(())
}

/// Removes the rule marker so the next VFS mutation can arm a fresh one.
///
/// Returns whether a marker was actually removed.
pub(crate) fn clear_rule_guard() -> Result<bool> {
    clear_at(Path::new(defs::VFS_BOOT_GUARD_PATH))
}

/// Removes both VFS boot guards, returning the paths that were present.
///
/// The caller is deliberately overriding the crash guard: the next VFS mutation injects the
/// rules again even though an earlier one interrupted the kernel.
pub(crate) fn clear_guards() -> Result<Vec<&'static str>> {
    let mut cleared = Vec::new();
    for path in [defs::VFS_BOOT_GUARD_PATH, defs::VFS_LKM_BOOT_GUARD_PATH] {
        if clear_at(Path::new(path))? {
            cleared.push(path);
        }
    }
    Ok(cleared)
}

/// Removes the marker at `path`; a missing file is already clear.
///
/// Returns whether a marker was actually removed.
fn clear_at(path: &Path) -> Result<bool> {
    match fs::remove_file(path) {
        Ok(()) => {
            if let Err(err) = crate::sys::fs::sync_parent_directory(path) {
                log::warn!("persist vfs boot guard removal failed: {err}");
            }
            Ok(true)
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(contextual(
            "remove vfs boot guard",
            path,
            CausalError::Io(err),
        )),
    }
}

/// One line describing a parsed marker, or the legacy marker that has no identity.
pub(crate) fn describe(record: Option<&GuardRecord>) -> String {
    match record {
        Some(record) => record.describe(),
        None => "no identity (legacy marker)".to_owned(),
    }
}

/// Reads a marker file verbatim; `None` when it does not exist.
fn read_marker(path: &Path) -> Result<Option<String>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(String::from_utf8_lossy(&bytes).into_owned())),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(contextual(
            "read vfs boot guard",
            path,
            CausalError::Io(err),
        )),
    }
}

fn contextual(operation: &'static str, path: &Path, cause: CausalError) -> Error {
    Error::Vfs(Box::new(ContextError::new(
        operation,
        Some(path.to_path_buf()),
        cause,
    )))
}

#[cfg(test)]
#[path = "boot_guard_tests.rs"]
mod tests;
