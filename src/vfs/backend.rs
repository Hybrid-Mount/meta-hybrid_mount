// SPDX-License-Identifier: GPL-3.0-only

//! 唯一活动的 VFS 内核 Provider：K1（NoMount）或 K2（HM 自有），二选一。
//! 选择结果在本次启动内固定，禁止热切换。

use crate::errors::{Error, Result};
use crate::vfs::protocol::{self, EncodedRule, NmCommand};
use crate::vfs::sys::{self, PageBuffer};

pub const SUPPORTED_VERSIONS: &[&str] = &["20"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VfsProvider {
    Nomount,
    Hm,
}

impl VfsProvider {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Nomount => "nomount",
            Self::Hm => "hm",
        }
    }
}

pub trait VfsKernel {
    fn version(&mut self) -> Result<String>;
    fn apply_rules(&mut self, rules: &[EncodedRule]) -> Result<()>;
    fn add_uids(&mut self, uids: &[u32]) -> Result<()>;
    fn clear_rules(&mut self) -> Result<()>;
}

pub trait LkmLoader {
    fn load_hm_vfs(&self) -> Result<()>;
}

pub struct KeyringKernel {
    page: PageBuffer,
}

impl KeyringKernel {
    pub fn new() -> Result<Self> {
        let page = PageBuffer::new().map_err(Error::Io)?;
        Ok(Self { page })
    }

    fn exchange(&mut self, request: &[u8]) -> Result<Vec<u8>> {
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
        sys::add_key(&mut self.page).map_err(Error::Io)?;
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
            protocol::ensure_status(&response)?;
        }
        Ok(())
    }

    fn add_uids(&mut self, uids: &[u32]) -> Result<()> {
        for uid in uids {
            let request = protocol::build_payload(NmCommand::AddUid, *uid, &[])?;
            let response = self.exchange(&request)?;
            protocol::ensure_status(&response)?;
        }
        Ok(())
    }

    fn clear_rules(&mut self) -> Result<()> {
        let request = protocol::build_payload(NmCommand::ClearRules, 0, &[])?;
        let response = self.exchange(&request)?;
        protocol::ensure_status(&response)?;
        Ok(())
    }
}

/// 选择唯一活动的 Provider。
///
/// 1. 已有可响应且版本受支持的 Provider：直接采用，不加载任何模块；
/// 2. 否则尝试加载 HM 自有 VFS LKM，再重新探测；
/// 3. 仍不可用返回 `Ok(None)`，由调用方决定降级或失败。
pub fn select_provider(
    kernel: &mut dyn VfsKernel,
    loader: &dyn LkmLoader,
    supported: &[&str],
    hm_loaded: bool,
) -> Result<Option<VfsProvider>> {
    match kernel.version() {
        Ok(found) if supported.contains(&found.as_str()) => {
            return Ok(Some(if hm_loaded {
                VfsProvider::Hm
            } else {
                VfsProvider::Nomount
            }));
        }
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
