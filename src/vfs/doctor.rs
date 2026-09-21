// SPDX-License-Identifier: GPL-3.0-only

//! `vfs doctor`: how the hybridmount provider is bound, and what it currently holds.
//!
//! Answers what the boot log cannot: whether hybridmount is compiled into the kernel,
//! whether a separately installed module is loaded instead, and whether the wire magic was accepted.
//! Read-only: it never loads or unloads anything.

use serde::Serialize;

use crate::errors::Result;
use crate::vfs::backend::{SUPPORTED_VERSIONS, VfsProvider};
use crate::vfs::protocol::ListedRule;

/// How hybridmount is present on the device.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModulePresence {
    /// Listed in `/proc/modules`: a loadable module, currently loaded.
    Loadable,
    /// A `/sys/module` entry with no `/proc/modules` entry: compiled into the kernel image.
    BuiltIn,
    /// Neither table entry.
    NotPresent,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct VfsDoctorReport {
    pub key_type: &'static str,
    pub provider: &'static str,
    pub presence: ModulePresence,
    /// The version the key type reported, when it answered.
    pub version: Option<String>,
    pub supported_versions: Vec<String>,
    /// Whether the key type answered with a supported version.
    pub responds: bool,
    /// Whether a compatible `hybridmount` must be supplied by the kernel or installed separately.
    pub provider_install_required: bool,
    /// Plain-language verdict, so a bug report does not require the reader to infer one.
    pub diagnosis: &'static str,
    pub rules: Vec<ListedRule>,
    pub uids: Vec<u32>,
    /// Set when the rule or uid table could not be read.
    pub list_error: Option<String>,
    /// The original GET_VERSION error, distinct from rule/UID listing failures.
    pub probe_error: Option<String>,
}

/// `/proc/modules` lists loadable modules only, so an answering key type with a
/// `/sys/module` entry but no `/proc/modules` entry is compiled into the kernel image.
pub fn classify_presence(in_proc_modules: bool, sys_module_exists: bool) -> ModulePresence {
    match (in_proc_modules, sys_module_exists) {
        (true, _) => ModulePresence::Loadable,
        (false, true) => ModulePresence::BuiltIn,
        (false, false) => ModulePresence::NotPresent,
    }
}

/// Whether the device still needs a compatible hybridmount provider supplied independently.
pub fn provider_install_required(presence: ModulePresence, responds: bool) -> bool {
    !responds && presence == ModulePresence::NotPresent
}

/// A one-line verdict for this state.
pub fn diagnose(presence: ModulePresence, version: Option<&str>, responds: bool) -> &'static str {
    if responds {
        return match presence {
            ModulePresence::BuiltIn => "hybridmount is built into this kernel; no module is loaded",
            ModulePresence::Loadable | ModulePresence::NotPresent => {
                "the hybridmount key type is registered by a loaded module"
            }
        };
    }

    match (presence, version) {
        (_, Some(_)) => {
            "the key type answered with a version this build does not support; vfs is skipped"
        }
        (ModulePresence::BuiltIn, None) => {
            "hybridmount is built in but its key type did not answer; the wire magic may not match"
        }
        (ModulePresence::Loadable, None) => {
            "a hybridmount module is loaded but its key type did not answer; the wire magic may not match"
        }
        (ModulePresence::NotPresent, None) => {
            "no hybridmount key type is registered; vfs needs it built into the kernel or separately installed"
        }
    }
}

/// Both listed tables as the provider reports them.
pub type ListedTables = (Vec<ListedRule>, Vec<u32>);

/// Builds the report from an observed probe plus the two optional tables.
pub fn summarize(
    version: Option<String>,
    presence: ModulePresence,
    listed: Result<ListedTables>,
) -> VfsDoctorReport {
    let responds = version
        .as_deref()
        .is_some_and(|found| SUPPORTED_VERSIONS.contains(&found));
    let (rules, uids, list_error) = match listed {
        Ok((rules, uids)) => (rules, uids, None),
        Err(err) => (Vec::new(), Vec::new(), Some(err.to_string())),
    };

    VfsDoctorReport {
        key_type: "hybridmount",
        provider: VfsProvider::Hm.as_str(),
        presence,
        responds,
        provider_install_required: provider_install_required(presence, responds),
        diagnosis: diagnose(presence, version.as_deref(), responds),
        version,
        supported_versions: SUPPORTED_VERSIONS.iter().map(|v| (*v).to_owned()).collect(),
        rules,
        uids,
        list_error,
        probe_error: None,
    }
}

const PROC_MODULES: &str = "/proc/modules";
const SYS_MODULE_DIR: &str = "/sys/module/hybridmount";

/// Whether `hybridmount` is the first field of any `/proc/modules` line.
fn listed_in_proc_modules(text: &str) -> bool {
    text.lines()
        .filter_map(|line| line.split_whitespace().next())
        .any(|name| name == "hybridmount")
}

pub(crate) fn presence_on_device() -> ModulePresence {
    let in_proc = std::fs::read_to_string(PROC_MODULES)
        .map(|text| listed_in_proc_modules(&text))
        .unwrap_or(false);
    classify_presence(in_proc, std::path::Path::new(SYS_MODULE_DIR).exists())
}

/// Probes the key type and, when it answers, reads both tables. Never loads anything.
#[cfg(any(target_os = "linux", target_os = "android"))]
fn observe() -> (Option<String>, Result<ListedTables>, Option<String>) {
    use crate::vfs::backend::{KeyringKernel, VfsKernel};
    use crate::vfs::sys::KeyringChannel;

    let mut kernel = match KeyringKernel::new(KeyringChannel::Hybridmount) {
        Ok(kernel) => kernel,
        Err(err) => return (None, Ok((Vec::new(), Vec::new())), Some(err.to_string())),
    };
    let version = match kernel.version() {
        Ok(version) => version,
        Err(err) => return (None, Ok((Vec::new(), Vec::new())), Some(err.to_string())),
    };

    let rules = kernel.list_rules();
    let uids = kernel.list_uids();
    (
        Some(version),
        rules.and_then(|rules| uids.map(|uids| (rules, uids))),
        None,
    )
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn observe() -> (Option<String>, Result<ListedTables>, Option<String>) {
    (None, Ok((Vec::new(), Vec::new())), None)
}

/// Whether the kernel answers as a supported hybridmount provider this boot.
///
/// The single authority for advertising VFS anywhere: pages, counts and the dynamic module
/// description all read this. A device whose kernel does not carry the module and whose
/// bundled module is absent or cannot load reports `false`, and the front ends hide the
/// backend entirely.
///
/// Read-only with respect to module loading: it probes the key type only, so a status or
/// doctor query never turns into an `insmod`.
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn responds_now() -> bool {
    use crate::vfs::backend::KeyringKernel;
    use crate::vfs::sys::KeyringChannel;

    let Ok(mut kernel) = KeyringKernel::new(KeyringChannel::Hybridmount) else {
        return false;
    };
    kernel
        .probe_version()
        .is_some_and(|found| SUPPORTED_VERSIONS.contains(&found.as_str()))
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn responds_now() -> bool {
    false
}

/// Collects the live report without loading or unloading a provider.
pub fn report() -> VfsDoctorReport {
    let (version, listed, probe_error) = observe();
    let mut report = summarize(version, presence_on_device(), listed);
    report.probe_error = probe_error;
    report
}

/// Prints the diagnostic report as JSON on stdout.
pub fn handle() -> Result<()> {
    let report = report();
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

#[cfg(test)]
#[path = "doctor_tests.rs"]
mod tests;

/// Never insert or unload a second module over an existing provider, even when
/// its protocol is incompatible. A built-in provider can only change on reboot.
pub(crate) fn ensure_provider_absent(presence: ModulePresence) -> std::result::Result<(), String> {
    match presence {
        ModulePresence::NotPresent => Ok(()),
        ModulePresence::BuiltIn | ModulePresence::Loadable => Err(format!(
            "hybridmount is already present ({presence:?}); refusing duplicate insmod/unload; \
             run vfs-doctor and update the existing provider to match the userspace protocol"
        )),
    }
}
