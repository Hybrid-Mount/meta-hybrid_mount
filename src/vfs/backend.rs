// SPDX-License-Identifier: GPL-3.0-only

//! VFS 内核 Provider 绑定。v2 只支持 Hybrid Mount 自有的 K2；设备上若已存在
//! 外来 NoMount 实现，则拒绝附着。绑定结果在本次启动内固定，禁止热切换。

use crate::errors::{Error, Result};
use crate::vfs::protocol::{self, EncodedRule, NmCommand};
use crate::vfs::sys::{self, KeyringChannel, PageBuffer};

/// K2 支持的协议版本。上游 NoMount 的 "20" 不在其中。
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
    /// 定向删除指定的规则；不触碰 Provider 中其它来源的规则。
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

/// 选择并绑定唯一活动的 VFS 内核 Provider（v2：只有 K2）。
///
/// 1. 单向守卫：`foreign_nomount` 为真表示设备上已存在外来 NoMount 实现，此时拒绝
///    附着（不加载模块、不下发规则）；
/// 2. 已有可响应的 K2：直接采用，不加载任何模块；
/// 3. 否则尝试加载 HM 自有 VFS LKM，再重新探测；
/// 4. 仍不可用返回 `Ok(None)`，由调用方决定降级或失败。
///
/// 当前 `LkmLoader` 仍为 no-op（K2 内核子系统尚未接入构建），因此第 3 步尚不能真正
/// 加载模块；加载实现就绪后本函数无需改动。
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
