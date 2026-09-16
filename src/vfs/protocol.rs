// SPDX-License-Identifier: GPL-3.0-only

//! K2 wire 协议（布局沿用 NoMount v20 基线）：`add_key("hybridmount", "trigger", &payload_ptr)`
//! 指向的单页 `hm_payload`。这里只做纯字节编解码，便于主机单测。

use crate::errors::{Error, Result};
use crate::vfs::rule::{VfsAction, VfsRule};

/// K2 wire 规格字面量，必须与内核头文件的 `HYBRIDMOUNT_MAGIC_SIG` 逐字节一致。
///
/// 取值是 ASCII "HYBRIDMO" 的大端读数，即小端机落盘的 8 字节为 `OMDIRBYH`。沿用上游
/// 的记法（上游用 "NOMOUNT" 的大端读数），但换成 HM 专属值：上游魔数是公开常量，换掉
/// 后旧版 nm CLI 即使不检查版本串也会在 preparse 阶段被 `-EFAULT` 拒绝，二进制层面
/// 与上游彻底断开，而不再只靠 key type 名隔离。
pub const MAGIC: u64 = 0x4859_4252_4944_4D4F;
pub const PAYLOAD_LEN: usize = 4096;
pub const BUFFER_LEN: usize = 4068;
pub const RULE_HEADER_LEN: usize = 12;
// K2 wire 契约：DEL 命令的 6 字节头长度（u32 uid + u16 v_len，随后是 vpath 字节）。
pub const DEL_HEADER_LEN: usize = 6;

pub const FLAG_WHITEOUT: u32 = 1 << 2;
/// 与目录规则组合使用：目录保持可见，真实条目隐藏，只显示注入子项（`.replace`）。
pub const FLAG_OPAQUE: u32 = 1 << 3;

// K2 wire 契约的完整命令集，当前后端只发出 AddRule/DelRule/AddUid/GetVersion 子集，
// 其余（DelUid/ClearAll/ClearRules/ClearUids/GetList/GetUids）留给后续版本，命令号取值不可改动。
// 用 allow 而非 expect：非 linux/android 目标由 src/main.rs 的 crate 级 allow(dead_code) 覆盖，lint 不触发时
// expect 会产生 unfulfilled_lint_expectations，从而让宿主 -D warnings 失败。
#[allow(dead_code)]
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
        VfsAction::OpaqueDir { virtual_path } => Ok(EncodedRule {
            flags: FLAG_OPAQUE,
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
    // status 哨兵：内核未处理该 payload 时保持 -1，处理后会回写真实结果。
    page[16..20].copy_from_slice(&(-1_i32).to_le_bytes());
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

pub fn build_del_rule_payloads(paths: &[Vec<u8>], uid: u32) -> Result<Vec<Vec<u8>>> {
    let mut payloads = Vec::new();
    let mut buffer: Vec<u8> = Vec::new();
    for path in paths {
        let record_len = DEL_HEADER_LEN + path.len();
        if record_len > BUFFER_LEN {
            return Err(Error::VfsProtocol {
                detail: format!("single del rule needs {record_len} bytes"),
            });
        }
        if buffer.len() + record_len > BUFFER_LEN {
            payloads.push(build_payload(NmCommand::DelRule, uid, &buffer)?);
            buffer.clear();
        }
        buffer.extend_from_slice(&uid.to_le_bytes());
        buffer.extend_from_slice(&(path.len() as u16).to_le_bytes());
        buffer.extend_from_slice(path);
    }
    if !buffer.is_empty() {
        payloads.push(build_payload(NmCommand::DelRule, uid, &buffer)?);
    }
    Ok(payloads)
}

/// 内核回写的负 errno：规则不存在，回滚删除时容忍。
const KERNEL_ENOENT: i32 = -2;
/// 内核回写的负 errno：UID 已在隔离表内，ADD_UID 的幂等结果。
const KERNEL_EEXIST: i32 = -17;

/// 读取 payload 的 `status` 字段，并校验响应长度。
fn read_status(payload: &[u8]) -> Result<i32> {
    if payload.len() != PAYLOAD_LEN {
        return Err(Error::VfsProtocol {
            detail: format!("response {} bytes, expected {PAYLOAD_LEN}", payload.len()),
        });
    }
    let raw = payload[16..20].try_into().map_err(|_| Error::VfsProtocol {
        detail: "status field is not four bytes".to_owned(),
    })?;
    Ok(i32::from_le_bytes(raw))
}

/// 统一的 status 校验：`allowed` 中的负 errno 视为成功，其余负值报错。
fn ensure_status_allowing(payload: &[u8], allowed: &[i32]) -> Result<()> {
    let status = read_status(payload)?;
    if status < 0 && !allowed.contains(&status) {
        return Err(Error::VfsProtocol {
            detail: format!("kernel returned status {status}"),
        });
    }
    Ok(())
}

pub fn ensure_status(payload: &[u8]) -> Result<()> {
    ensure_status_allowing(payload, &[])
}

/// 回滚删除时容忍 ENOENT（规则本就不存在）；其它负 status 视为错误。
///
/// 与 ADD_RULE 一样校验续传游标：DEL_RULE 遇到批内长度越界会 `break`，此时 status
/// 仍为 0 而 `arg1` 停在截断处。只看 status 会把「还有规则没删掉」当成回滚成功，
/// 残留规则会在本次启动继续生效。整批一条都没删到时内核回 -ENOENT，游标同样停在
/// 消费量上，故 ENOENT 走与成功相同的游标校验。
pub fn ensure_status_allow_enoent(payload: &[u8]) -> Result<()> {
    ensure_status_allowing(payload, &[KERNEL_ENOENT])?;
    ensure_full_cursor(payload)
}

/// ADD_UID 幂等：UID 已在隔离表内时内核回 -EEXIST，这不是失败。
pub fn ensure_status_allow_eexist(payload: &[u8]) -> Result<()> {
    ensure_status_allowing(payload, &[KERNEL_EEXIST])
}

/// 校验 ADD_RULE 响应既成功又完整消费了批内字节。
///
/// K2 语义：成功时 `arg1` 是已消费的 buffer 字节数，必须等于 `data_size`，否则视为
/// 部分应用；失败时 `status` 是批内**首个**错误的 errno，而 `arg1` 是那条失败记录的
/// 起始偏移。上游实现逐条覆盖 status，批量中间的失败会被后续成功静默掩盖，K2 修掉了
/// 这一点，因此这里的错误信息会带上失败位置。
pub fn ensure_consumed(payload: &[u8]) -> Result<()> {
    let status = read_status(payload)?;
    let arg1 = read_arg1(payload)?;
    if status < 0 {
        return Err(Error::VfsProtocol {
            detail: format!("kernel returned status {status} for the record at offset {arg1}"),
        });
    }
    ensure_full_cursor(payload)
}

/// 校验批内字节被完整消费。截断的批次（`arg1 != data_size`）意味着还有记录没处理，
/// 无论 status 是 0 还是 ENOENT 都不能当成整批成功。
fn ensure_full_cursor(payload: &[u8]) -> Result<()> {
    let arg1 = read_arg1(payload)?;
    let data_size =
        u32::from_le_bytes(payload[24..28].try_into().map_err(|_| Error::VfsProtocol {
            detail: "data_size field is not four bytes".to_owned(),
        })?);
    if arg1 != data_size {
        return Err(Error::VfsProtocol {
            detail: format!("kernel consumed {arg1} of {data_size} bytes"),
        });
    }
    Ok(())
}

fn read_arg1(payload: &[u8]) -> Result<u32> {
    if payload.len() != PAYLOAD_LEN {
        return Err(Error::VfsProtocol {
            detail: format!("response {} bytes, expected {PAYLOAD_LEN}", payload.len()),
        });
    }
    let raw = payload[20..24].try_into().map_err(|_| Error::VfsProtocol {
        detail: "arg1 field is not four bytes".to_owned(),
    })?;
    Ok(u32::from_le_bytes(raw))
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
