// SPDX-License-Identifier: GPL-3.0-only

//! NoMount wire 协议：`add_key("nomount", "trigger", &payload_ptr)` 指向的
//! 单页 `nm_payload`。这里只做纯字节编解码，便于主机单测。

use crate::errors::{Error, Result};
use crate::vfs::rule::{VfsAction, VfsRule};

// 规格字面量 0x4E4F4D4F554E54（7 字节），此处补齐为 8 字节同样取值以通过 clippy。
pub const MAGIC: u64 = 0x004E_4F4D_4F55_4E54;
pub const PAYLOAD_LEN: usize = 4096;
pub const BUFFER_LEN: usize = 4068;
pub const RULE_HEADER_LEN: usize = 12;
pub const DEL_HEADER_LEN: usize = 6;

pub const FLAG_WHITEOUT: u32 = 1 << 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum NmCommand {
    GetVersion = 1,
    AddRule = 2,
    DelRule = 3,
    AddUid = 4,
    DelUid = 5,
    ClearAll = 6,
    ClearRules = 7,
    ClearUids = 8,
    GetList = 9,
    GetUids = 10,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncodedRule {
    pub flags: u32,
    pub virtual_path: Vec<u8>,
    pub real_path: Vec<u8>,
}

impl EncodedRule {
    pub fn record_len(&self) -> usize {
        RULE_HEADER_LEN + self.virtual_path.len() + self.real_path.len()
    }

    pub fn write_into(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.flags.to_le_bytes());
        out.extend_from_slice(&0_u32.to_le_bytes());
        out.extend_from_slice(&(self.virtual_path.len() as u16).to_le_bytes());
        out.extend_from_slice(&(self.real_path.len() as u16).to_le_bytes());
        out.extend_from_slice(&self.virtual_path);
        out.extend_from_slice(&self.real_path);
    }
}

pub fn encode_rule(rule: &VfsRule) -> Result<EncodedRule> {
    match &rule.action {
        VfsAction::Inject {
            virtual_path,
            real_path,
        } => Ok(EncodedRule {
            flags: 0,
            virtual_path: virtual_path.as_bytes().to_vec(),
            real_path: real_path.to_string_lossy().as_bytes().to_vec(),
        }),
        VfsAction::Whiteout { virtual_path } => Ok(EncodedRule {
            flags: FLAG_WHITEOUT,
            virtual_path: virtual_path.as_bytes().to_vec(),
            real_path: Vec::new(),
        }),
    }
}

pub fn build_payload(cmd: NmCommand, target_uid: u32, buffer: &[u8]) -> Result<Vec<u8>> {
    if buffer.len() > BUFFER_LEN {
        return Err(Error::VfsProtocol {
            detail: format!("buffer {} exceeds {BUFFER_LEN}", buffer.len()),
        });
    }
    let mut page = vec![0_u8; PAYLOAD_LEN];
    page[0..8].copy_from_slice(&MAGIC.to_le_bytes());
    page[8..12].copy_from_slice(&(cmd as u32).to_le_bytes());
    page[12..16].copy_from_slice(&target_uid.to_le_bytes());
    page[24..28].copy_from_slice(&(buffer.len() as u32).to_le_bytes());
    page[28..28 + buffer.len()].copy_from_slice(buffer);
    Ok(page)
}

pub fn build_add_rule_payloads(rules: &[EncodedRule], uid: u32) -> Result<Vec<Vec<u8>>> {
    let mut payloads = Vec::new();
    let mut buffer: Vec<u8> = Vec::new();
    for rule in rules {
        if rule.record_len() > BUFFER_LEN {
            return Err(Error::VfsProtocol {
                detail: format!("single rule needs {} bytes", rule.record_len()),
            });
        }
        if buffer.len() + rule.record_len() > BUFFER_LEN {
            payloads.push(build_payload(NmCommand::AddRule, uid, &buffer)?);
            buffer.clear();
        }
        rule.write_into(&mut buffer);
    }
    if !buffer.is_empty() {
        payloads.push(build_payload(NmCommand::AddRule, uid, &buffer)?);
    }
    Ok(payloads)
}

pub fn ensure_status(payload: &[u8]) -> Result<()> {
    if payload.len() != PAYLOAD_LEN {
        return Err(Error::VfsProtocol {
            detail: format!("response {} bytes, expected {PAYLOAD_LEN}", payload.len()),
        });
    }
    let status =
        i32::from_le_bytes(payload[16..20].try_into().map_err(|_| Error::VfsProtocol {
            detail: "status field is not four bytes".to_owned(),
        })?);
    if status < 0 {
        return Err(Error::VfsProtocol {
            detail: format!("kernel returned status {status}"),
        });
    }
    Ok(())
}

pub fn parse_version(payload: &[u8]) -> Result<String> {
    ensure_status(payload)?;
    let len = u32::from_le_bytes(payload[24..28].try_into().map_err(|_| Error::VfsProtocol {
        detail: "data_size field is not four bytes".to_owned(),
    })?) as usize;
    if len > BUFFER_LEN {
        return Err(Error::VfsProtocol {
            detail: format!("version length {len} exceeds {BUFFER_LEN}"),
        });
    }
    let raw = &payload[28..28 + len];
    let text = std::str::from_utf8(raw).map_err(|_| Error::VfsProtocol {
        detail: "version is not valid utf-8".to_owned(),
    })?;
    Ok(text.trim().to_owned())
}

#[cfg(test)]
#[path = "protocol_tests.rs"]
mod tests;
