// SPDX-License-Identifier: GPL-3.0-only

//! VFS kernel provider binding. v2 supports only K2; a device already running a
//! foreign NoMount implementation is refused. The binding is fixed for the boot.

use crate::errors::{Error, Result};
use crate::vfs::protocol::{self, EncodedRule, NmCommand};
use crate::vfs::sys::{self, KeyringChannel, PageBuffer};

/// Protocol versions K2 accepts. Upstream NoMount's "20" is not one of them.
pub const SUPPORTED_VERSIONS: &[&str] = &["hm1"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VfsProvider {
    Hm,
}

impl VfsProvider {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Hm => "hm",
        }
    }
}

pub trait VfsKernel {
    fn version(&mut self) -> Result<String>;
    fn apply_rules(&mut self, rules: &[EncodedRule]) -> Result<()>;
    fn add_uids(&mut self, uids: &[u32]) -> Result<()>;
    /// Deletes the given rules only, leaving rules from other sources in place.
    fn remove_rules(&mut self, rules: &[EncodedRule]) -> Result<()>;
}

pub trait LkmLoader {
    fn load_hm_vfs(&self) -> Result<()>;
}

pub struct KeyringKernel {
    page: PageBuffer,
    channel: KeyringChannel,
}

impl KeyringKernel {
    pub fn new(channel: KeyringChannel) -> Result<Self> {
        let page = PageBuffer::new().map_err(Error::Io)?;
        Ok(Self { page, channel })
    }

    /// Probes the key type, returning its version when it answers.
    ///
    /// `None` covers both "no such key type" and "the page was rejected"; the caller
    /// distinguishes those from the device's module tables rather than from the errno.
    pub fn probe_version(&mut self) -> Option<String> {
        let request = protocol::build_payload(NmCommand::GetVersion, 0, &[]).ok()?;
        let response = self.exchange(&request).ok()?;
        protocol::parse_version(&response).ok()
    }

    /// Every rule currently installed in the provider.
    pub fn list_rules(&mut self) -> Result<Vec<protocol::ListedRule>> {
        protocol::paginate(
            |cursor| {
                let page = protocol::build_list_payload(NmCommand::GetList, cursor)?;
                self.exchange(&page)
            },
            protocol::parse_list,
        )
    }

    /// Every isolated uid currently installed in the provider.
    pub fn list_uids(&mut self) -> Result<Vec<u32>> {
        protocol::paginate(
            |cursor| {
                let page = protocol::build_list_payload(NmCommand::GetUids, cursor)?;
                self.exchange(&page)
            },
            protocol::parse_uids,
        )
    }

    /// Sends one payload page and returns the page the kernel wrote back.
    pub(crate) fn exchange(&mut self, request: &[u8]) -> Result<Vec<u8>> {
        let page = self.page.as_mut_slice();
        if page.len() != request.len() {
            return Err(Error::VfsProtocol {
                detail: format!(
                    "request length {} does not match page length {}",
                    request.len(),
                    page.len()
                ),
            });
        }
        page.copy_from_slice(request);
        sys::add_key(&mut self.page, self.channel).map_err(Error::Io)?;
        Ok(self.page.as_mut_slice().to_vec())
    }
}

impl VfsKernel for KeyringKernel {
    fn version(&mut self) -> Result<String> {
        let request = protocol::build_payload(NmCommand::GetVersion, 0, &[])?;
        let response = self.exchange(&request)?;
        protocol::parse_version(&response)
    }

    fn apply_rules(&mut self, rules: &[EncodedRule]) -> Result<()> {
        for page in protocol::build_add_rule_payloads(rules, 0)? {
            let response = self.exchange(&page)?;
            // ADD_RULE must consume the whole batch; GET_VERSION leaves arg1 unset.
            protocol::ensure_consumed(&response)?;
        }
        Ok(())
    }

    fn add_uids(&mut self, uids: &[u32]) -> Result<()> {
        for uid in uids {
            let request = protocol::build_payload(NmCommand::AddUid, *uid, &[])?;
            let response = self.exchange(&request)?;
            // The uid table survives until reboot, so a second pipeline run in the
            // same boot sees -EEXIST even though the uid is already isolated.
            protocol::ensure_status_allow_eexist(&response)?;
        }
        Ok(())
    }

    fn remove_rules(&mut self, rules: &[EncodedRule]) -> Result<()> {
        // DEL_RULE indexes by virtual path, so only vpath is needed.
        let paths = rules
            .iter()
            .map(|rule| rule.virtual_path.clone())
            .collect::<Vec<_>>();
        for page in protocol::build_del_rule_payloads(&paths, 0)? {
            let response = self.exchange(&page)?;
            // The rule may be gone or never have taken effect.
            protocol::ensure_status_allow_enoent(&response)?;
        }
        Ok(())
    }
}

/// Binds the single active VFS kernel provider (v2: K2 only).
///
/// 1. If a foreign NoMount implementation is present, refuse to attach.
/// 2. If K2 already responds, use it without loading anything.
/// 3. Otherwise load the bundled VFS LKM and probe again.
/// 4. Still unavailable: `Ok(None)`, leaving the caller to degrade or fail.
///
/// The magic must match the kernel header's `HYBRIDMOUNT_MAGIC_SIG`; on a mismatch the
/// kernel rejects the page with `-EFAULT` and the probe fails, which degrades rather
/// than failing the boot.
pub fn select_provider(
    kernel: &mut dyn VfsKernel,
    loader: &dyn LkmLoader,
    supported: &[&str],
    foreign_nomount: bool,
) -> Result<Option<VfsProvider>> {
    if foreign_nomount {
        return Err(Error::VfsForeignNomount {
            detail: "a foreign NoMount kernel implementation is present".to_owned(),
        });
    }

    match kernel.version() {
        Ok(found) if supported.contains(&found.as_str()) => return Ok(Some(VfsProvider::Hm)),
        Ok(found) => {
            return Err(Error::VfsUnsupportedVersion {
                found,
                supported: supported.join(","),
            });
        }
        Err(_) => {}
    }

    loader.load_hm_vfs()?;

    match kernel.version() {
        Ok(found) if supported.contains(&found.as_str()) => Ok(Some(VfsProvider::Hm)),
        Ok(found) => Err(Error::VfsUnsupportedVersion {
            found,
            supported: supported.join(","),
        }),
        Err(_) => Ok(None),
    }
}

#[cfg(test)]
#[path = "backend_tests.rs"]
mod tests;
