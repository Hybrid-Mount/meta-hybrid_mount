// SPDX-License-Identifier: GPL-3.0-only
//! Device adapter for the testable ownership/transaction model.

use super::ledger::{self, Ledger};
use super::rules::{RuleKernel, SavedRule};
use crate::errors::{Error, Result};
use crate::vfs::backend::{KeyringKernel, SUPPORTED_VERSIONS, VfsKernel};
use crate::vfs::protocol::{self, EncodedRule, NmCommand};
use crate::vfs::sys::KeyringChannel;
use std::fs;
use std::os::fd::AsRawFd;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

pub fn namespace_at(path: &str) -> Result<String> {
    let meta = fs::metadata(path)?;
    Ok(format!("{}:{}", meta.dev(), meta.ino()))
}

pub fn enter_init_namespace() -> Result<()> {
    if namespace_at("/proc/self/ns/mnt")? != namespace_at("/proc/1/ns/mnt")? {
        let ns = fs::File::open("/proc/1/ns/mnt")?;
        // SAFETY: ns is a live mount namespace descriptor; CLI mutators are single-threaded.
        if unsafe { libc::setns(ns.as_raw_fd(), libc::CLONE_NEWNS) } != 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        std::env::set_current_dir("/")?;
    }
    Ok(())
}

pub fn load() -> Result<Ledger> {
    let boot_id = fs::read_to_string("/proc/sys/kernel/random/boot_id")?;
    let ns = namespace_at("/proc/1/ns/mnt")?;
    ledger::ledger_for_boot(
        ledger::read_at(Path::new(ledger::LEDGER_PATH))?,
        boot_id.trim(),
        &ns,
    )
}

pub struct Kernel(pub KeyringKernel);
impl Kernel {
    pub fn open() -> Result<Self> {
        if crate::vfs::guard::detect_foreign_nomount().blocked() {
            return Err(Error::msg("foreign NoMount provider detected"));
        }
        Self::inspect()
    }

    /// Read-only inspection must not block an ordinary Magic/Overlay boot just
    /// because an external NoMount installation disabled HM VFS planning.
    pub fn inspect() -> Result<Self> {
        let mut kernel = KeyringKernel::new(KeyringChannel::Hybridmount)?;
        let version = kernel.version()?;
        if !SUPPORTED_VERSIONS.contains(&version.as_str()) {
            return Err(Error::msg("unsupported VFS provider"));
        }
        Ok(Self(kernel))
    }

    pub fn uids(&mut self) -> Result<Vec<u32>> {
        protocol::paginate(
            |cursor| {
                self.0
                    .exchange(&protocol::build_list_payload(NmCommand::GetUids, cursor)?)
            },
            protocol::parse_uids,
        )
    }

    pub fn remove_uids(&mut self, uids: &[u32]) -> Result<()> {
        for uid in uids {
            protocol::ensure_status_allow_enoent(&self.0.exchange(&protocol::build_payload(
                NmCommand::DelUid,
                *uid,
                &[],
            )?)?)?;
        }
        if self.uids()?.iter().any(|uid| uids.contains(uid)) {
            return Err(Error::msg(
                "VFS isolation UID cleanup could not be verified",
            ));
        }
        Ok(())
    }
}

impl RuleKernel for Kernel {
    fn list(&mut self) -> Result<Vec<SavedRule>> {
        Ok(self
            .0
            .list_rules()?
            .into_iter()
            .map(|r| SavedRule {
                module_id: String::new(),
                virtual_path: r.virtual_path,
                real_path: r.real_path,
                flags: r.flags,
                uid: r.uid,
            })
            .collect())
    }
    fn put(&mut self, rules: &[SavedRule]) -> Result<()> {
        // Per-record UID is respected; encoding all inputs before mutation remains the caller's responsibility.
        let pages = rules
            .iter()
            .map(|r| {
                protocol::build_add_rule_payloads(
                    &[EncodedRule {
                        flags: r.flags,
                        virtual_path: r.virtual_path.as_bytes().to_vec(),
                        real_path: r.real_path.as_bytes().to_vec(),
                    }],
                    r.uid,
                )
            })
            .collect::<Result<Vec<_>>>()?;
        for group in pages {
            for page in group {
                protocol::ensure_consumed(&self.0.exchange(&page)?)?;
            }
        }
        Ok(())
    }
    fn delete(&mut self, rules: &[SavedRule]) -> Result<()> {
        let pages = rules
            .iter()
            .map(|r| {
                protocol::build_del_rule_payloads(&[r.virtual_path.as_bytes().to_vec()], r.uid)
            })
            .collect::<Result<Vec<_>>>()?;
        for group in pages {
            for page in group {
                protocol::ensure_status_allow_enoent(&self.0.exchange(&page)?)?;
            }
        }
        Ok(())
    }
}
