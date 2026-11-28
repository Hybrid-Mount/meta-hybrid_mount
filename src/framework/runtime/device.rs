// SPDX-License-Identifier: GPL-3.0-only
//! Device adapter for the testable ownership/transaction model.

use super::ledger::{self, Ledger};
use super::rules::{RuleKernel, SavedRule};
use crate::errors::{Error, Result};
use crate::mount::vfs::backend::{KeyringKernel, VfsKernel, open_active};
use crate::mount::vfs::protocol::{self, EncodedRule, NmCommand};
use crate::platform::namespace::{boot_id, namespace_at};
use std::path::Path;

pub fn load() -> Result<Ledger> {
    let boot_id = boot_id()?;
    let ns = namespace_at("/proc/1/ns/mnt")?;
    ledger::ledger_for_boot(
        ledger::read_at(Path::new(ledger::LEDGER_PATH))?,
        &boot_id,
        &ns,
    )
}

pub struct Kernel(pub KeyringKernel);
impl Kernel {
    pub fn open(saved: &Ledger) -> Result<Self> {
        let kernel = open_active()?;
        if let Some(expected) = saved.vfs_provider.as_deref().or_else(|| {
            (!saved.rules.is_empty() || !saved.isolated_uids.is_empty()).then_some("hm")
        }) && kernel.provider().as_str() != expected
        {
            return Err(Error::msg("VFS provider changed during this boot"));
        }
        Ok(Self(kernel))
    }

    pub fn inspect() -> Result<Self> {
        Ok(Self(open_active()?))
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
            // The ownership ledger can only represent global and single-UID rules.
            .filter(|rule| rule.scope.range.is_none())
            .map(|r| SavedRule {
                module_id: String::new(),
                virtual_path: r.virtual_path,
                real_path: r.real_path,
                flags: r.flags,
                uid: r.scope.uid,
            })
            .collect())
    }
    fn put(&mut self, rules: &[SavedRule]) -> Result<()> {
        // Per-record UID is respected; encoding all inputs before mutation remains the caller's responsibility.
        let pages = rules
            .iter()
            .map(|r| {
                protocol::build_add_rule_payloads_for(
                    &[EncodedRule {
                        flags: r.flags,
                        virtual_path: r.virtual_path.as_bytes().to_vec(),
                        real_path: r.real_path.as_bytes().to_vec(),
                    }],
                    r.uid,
                    self.0.format(),
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
                protocol::build_del_rule_payloads_for(
                    &[r.virtual_path.as_bytes().to_vec()],
                    r.uid,
                    self.0.format(),
                )
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

impl super::transaction::UidKernel for Kernel {
    fn uids(&mut self) -> Result<Vec<u32>> {
        Kernel::uids(self)
    }

    fn add_uids(&mut self, uids: &[u32]) -> Result<()> {
        self.0.add_uids(uids)
    }

    fn remove_uids(&mut self, uids: &[u32]) -> Result<()> {
        Kernel::remove_uids(self, uids)
    }
}
