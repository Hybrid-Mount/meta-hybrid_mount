// SPDX-License-Identifier: GPL-3.0-only

//! `vfs doctor`: how the hybridmount provider is bound, and what it currently holds.
//!
//! Answers what the boot log cannot: whether hybridmount is compiled into the kernel,
//! whether a separately installed module is loaded instead, and whether the wire magic was accepted.
//! Read-only: it never loads or unloads anything.

use serde::Serialize;

use crate::errors::Result;
use crate::vfs::backend::{
    KeyringKernel, NOMOUNT_VERSIONS, SUPPORTED_VERSIONS, VfsProvider, rule_format,
};
use crate::vfs::protocol::ListedRule;
use crate::vfs::sys::KeyringChannel;

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
#[cfg(test)]
pub fn summarize(
    version: Option<String>,
    presence: ModulePresence,
    listed: Result<ListedTables>,
) -> VfsDoctorReport {
    summarize_channel(KeyringChannel::Hybridmount, version, presence, listed)
}

fn summarize_channel(
    channel: KeyringChannel,
    version: Option<String>,
    presence: ModulePresence,
    listed: Result<ListedTables>,
) -> VfsDoctorReport {
    let responds = version
        .as_deref()
        .is_some_and(|found| rule_format(channel, found).is_ok());
    let (rules, uids, list_error) = match listed {
        Ok((rules, uids)) => (rules, uids, None),
        Err(err) => (Vec::new(), Vec::new(), Some(err.to_string())),
    };

    let (key_type, provider, versions, diagnosis) = match channel {
        KeyringChannel::Hybridmount => (
            "hybridmount",
            VfsProvider::Hm,
            SUPPORTED_VERSIONS,
            diagnose(presence, version.as_deref(), responds),
        ),
        KeyringChannel::Nomount => (
            "nomount",
            VfsProvider::Nomount,
            NOMOUNT_VERSIONS,
            if responds {
                "NoMount is available as the VFS provider"
            } else {
                "NoMount is present but its VFS protocol is unavailable or unsupported"
            },
        ),
    };
    VfsDoctorReport {
        key_type,
        provider: provider.as_str(),
        presence,
        responds,
        provider_install_required: channel == KeyringChannel::Hybridmount
            && provider_install_required(presence, responds),
        diagnosis,
        version,
        supported_versions: versions.iter().map(|v| (*v).to_owned()).collect(),
        rules,
        uids,
        list_error,
        probe_error: None,
    }
}

const PROC_MODULES: &str = "/proc/modules";
fn listed_in_proc_modules(text: &str, name: &str) -> bool {
    text.lines()
        .any(|line| line.split_whitespace().next() == Some(name))
}

fn presence_of(name: &str) -> ModulePresence {
    let in_proc =
        std::fs::read_to_string(PROC_MODULES).is_ok_and(|text| listed_in_proc_modules(&text, name));
    let in_sys = std::path::Path::new("/sys/module").join(name).exists();
    classify_presence(in_proc, in_sys)
}

pub(crate) fn presence_on_device() -> ModulePresence {
    presence_of("hybridmount")
}

pub(crate) fn active_channel() -> Result<KeyringChannel> {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        let probe = super::guard::detect_foreign_nomount();
        if let Some(reason) = probe.error {
            return Err(crate::errors::Error::VfsUnavailable { reason });
        }
        if probe.present {
            if presence_on_device() != ModulePresence::NotPresent || hybridmount_responds() {
                return Err(crate::errors::Error::VfsUnavailable {
                    reason: "both NoMount and hybridmount are present".into(),
                });
            }
            return Ok(KeyringChannel::Nomount);
        }
    }
    Ok(KeyringChannel::Hybridmount)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub(crate) fn hybridmount_responds() -> bool {
    KeyringKernel::new(KeyringChannel::Hybridmount)
        .and_then(|mut kernel| kernel.require_supported())
        .is_ok()
}

/// Probes the key type and, when it answers, reads both tables. Never loads anything.
#[cfg(any(target_os = "linux", target_os = "android"))]
fn observe(channel: KeyringChannel) -> (Option<String>, Result<ListedTables>, Option<String>) {
    use crate::vfs::backend::VfsKernel;

    let mut kernel = match KeyringKernel::new(channel) {
        Ok(kernel) => kernel,
        Err(err) => return (None, Ok((Vec::new(), Vec::new())), Some(err.to_string())),
    };
    let version = match kernel.require_supported() {
        Ok(version) => version,
        Err(crate::errors::Error::VfsUnsupportedVersion { found, .. }) => {
            return (Some(found), Ok((Vec::new(), Vec::new())), None);
        }
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
fn observe(_channel: KeyringChannel) -> (Option<String>, Result<ListedTables>, Option<String>) {
    (None, Ok((Vec::new(), Vec::new())), None)
}

/// Read-only availability probe shared by boot, CLI and WebUI.
pub fn responds_now() -> bool {
    super::backend::open_active().is_ok()
}

/// Detects an installed provider whose protocol this build cannot recognize.
pub(crate) fn has_unrecognized_provider() -> bool {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        [
            (KeyringChannel::Hybridmount, "hybridmount"),
            (KeyringChannel::Nomount, "nomount"),
        ]
        .into_iter()
        .any(|(channel, name)| {
            let version =
                KeyringKernel::new(channel).and_then(|mut kernel| kernel.require_supported());
            is_unrecognized_provider(presence_of(name) != ModulePresence::NotPresent, version)
        })
    }
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    false
}

#[cfg(any(test, target_os = "linux", target_os = "android"))]
fn is_unrecognized_provider(present: bool, version: Result<String>) -> bool {
    use crate::errors::Error;

    match version {
        Ok(_) => false,
        Err(Error::VfsProtocol { .. } | Error::VfsUnsupportedVersion { .. }) => true,
        Err(_) => present,
    }
}

/// Collects the live report without loading or unloading a provider.
pub fn report() -> VfsDoctorReport {
    let (channel, presence, version, listed, probe_error) = match active_channel() {
        Ok(channel) => {
            let presence = match channel {
                KeyringChannel::Hybridmount => presence_on_device(),
                KeyringChannel::Nomount => presence_of("nomount"),
            };
            let (version, listed, probe_error) = observe(channel);
            (channel, presence, version, listed, probe_error)
        }
        Err(err) => (
            KeyringChannel::Hybridmount,
            presence_on_device(),
            None,
            Ok((Vec::new(), Vec::new())),
            Some(err.to_string()),
        ),
    };
    let mut report = summarize_channel(channel, version, presence, listed);
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
