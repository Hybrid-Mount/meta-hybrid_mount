// SPDX-License-Identifier: GPL-3.0-only

//! Shared foreign-provider guard for boot and explicit VFS loading.

use std::fs;
use std::path::Path;

use super::backend::{KeyringKernel, VfsKernel};
use super::sys::KeyringChannel;

#[derive(Debug, Default)]
pub(crate) struct ForeignNomountProbe {
    pub(crate) present: bool,
    /// A probe infrastructure failure blocks an automatic load just like a positive detection.
    /// Loading a second inode-hooking provider while the guard is uncertain is unsafe.
    pub(crate) error: Option<String>,
}

impl ForeignNomountProbe {
    pub(crate) fn blocked(&self) -> bool {
        self.present || self.error.is_some()
    }
}

/// One-way guard: detects whether a foreign NoMount implementation already exists on the device.
///
/// Module tables are checked first because an absent key type is the normal result on devices
/// without NoMount. If those tables cannot be read, the decision fails closed and no bundled
/// `hybridmount` module is loaded. The keyring probe catches built-in or renamed providers that
/// are not visible in the usual module tables.
pub(crate) fn detect_foreign_nomount() -> ForeignNomountProbe {
    const PROC_MODULES: &str = "/proc/modules";
    const SYS_MODULE_DIR: &str = "/sys/module/nomount";

    let proc_modules = match fs::read_to_string(PROC_MODULES) {
        Ok(text) => text,
        Err(err) => {
            return ForeignNomountProbe {
                present: false,
                error: Some(format!("read {PROC_MODULES}: {err}")),
            };
        }
    };
    let listed_in_proc = proc_modules
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .any(|name| name == "nomount");
    let listed_in_sys = Path::new(SYS_MODULE_DIR).exists();
    if listed_in_proc || listed_in_sys {
        log::warn!(
            "foreign NoMount VFS implementation detected: proc_modules={}, sys_module={}",
            listed_in_proc,
            listed_in_sys
        );
        return ForeignNomountProbe {
            present: true,
            error: None,
        };
    }

    match KeyringKernel::new(KeyringChannel::Nomount) {
        Ok(mut probe) => match probe.version() {
            Ok(version) => {
                log::warn!("foreign NoMount VFS implementation detected (version {version})");
                ForeignNomountProbe {
                    present: true,
                    error: None,
                }
            }
            Err(err) => {
                // No module-table entry plus an unanswered key type is the expected absent case.
                log::debug!("NoMount key type did not answer: {err}");
                ForeignNomountProbe::default()
            }
        },
        Err(err) => ForeignNomountProbe {
            present: false,
            error: Some(format!("create NoMount probe: {err}")),
        },
    }
}
