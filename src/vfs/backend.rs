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
    /// 定向删除指定的规则；不触碰 Provider 中其它来源的规则。
    fn remove_rules(&mut self, rules: &[EncodedRule]) -> Result<()>;
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
            // ADD_RULE 必须整批消费；GET_VERSION 不设置 arg1，仍用 ensure_status。
            protocol::ensure_consumed(&response)?;
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

    fn remove_rules(&mut self, rules: &[EncodedRule]) -> Result<()> {
        // DEL_RULE 按虚拟路径索引，只需 vpath；uid 与 ADD 保持一致（当前为 0）。
        let paths = rules
            .iter()
            .map(|rule| rule.virtual_path.clone())
            .collect::<Vec<_>>();
        for page in protocol::build_del_rule_payloads(&paths, 0)? {
            let response = self.exchange(&page)?;
            // 规则可能已被移除或从未生效：ENOENT 不是回滚失败。
            protocol::ensure_status_allow_enoent(&response)?;
        }
        Ok(())
    }
}

/// 选择唯一活动的 Provider。
///
/// 1. 已有可响应且版本受支持的 Provider：直接采用，不加载任何模块；
/// 2. 否则尝试加载 HM 自有 VFS LKM，再重新探测；
/// 3. 仍不可用返回 `Ok(None)`，由调用方决定降级或失败。
///
/// 注意：当前 `LkmLoader` 为 no-op，K2（HM 自有内核实现）尚未接入，因此“两个实现
/// 同时可见”的分支不可达。K2 接入时**必须**在此实现同名 key type 的“双可见/二次注册”
/// 检测并触发 `Error::VfsProviderConflict`；在此之前不得假设该冲突已被覆盖。
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
