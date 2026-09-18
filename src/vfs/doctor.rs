// SPDX-License-Identifier: GPL-3.0-only

//! `vfs doctor`: how the K2 provider is bound, and what it currently holds.
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
    /// Whether a compatible K2 must be supplied by the kernel or installed separately.
    pub provider_install_required: bool,
    /// Plain-language verdict, so a bug report does not require the reader to infer one.
    pub diagnosis: &'static str,
    pub rules: Vec<ListedRule>,
    pub uids: Vec<u32>,
    /// Set when the rule or uid table could not be read.
    pub list_error: Option<String>,
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

/// Whether the device still needs a compatible K2 provider supplied independently.
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
            "no hybridmount key type is registered; vfs requires a compatible built-in or separately installed K2"
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

fn presence_on_device() -> ModulePresence {
    let in_proc = std::fs::read_to_string(PROC_MODULES)
        .map(|text| listed_in_proc_modules(&text))
        .unwrap_or(false);
    classify_presence(in_proc, std::path::Path::new(SYS_MODULE_DIR).exists())
}

/// Probes the key type and, when it answers, reads both tables. Never loads anything.
#[cfg(any(target_os = "linux", target_os = "android"))]
fn observe() -> (Option<String>, Result<ListedTables>) {
    use crate::vfs::backend::KeyringKernel;
    use crate::vfs::sys::KeyringChannel;

    let Ok(mut kernel) = KeyringKernel::new(KeyringChannel::Hybridmount) else {
        return (None, Ok((Vec::new(), Vec::new())));
    };
    let Some(version) = kernel.probe_version() else {
        return (None, Ok((Vec::new(), Vec::new())));
    };

    let rules = kernel.list_rules();
    let uids = kernel.list_uids();
    (
        Some(version),
        rules.and_then(|rules| uids.map(|uids| (rules, uids))),
    )
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn observe() -> (Option<String>, Result<ListedTables>) {
    (None, Ok((Vec::new(), Vec::new())))
}

/// Prints the diagnostic report as JSON on stdout.
pub fn handle() -> Result<()> {
    let (version, listed) = observe();
    let report = summarize(version, presence_on_device(), listed);
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

#[cfg(test)]
#[path = "doctor_tests.rs"]
mod tests;
