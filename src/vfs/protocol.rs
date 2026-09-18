// SPDX-License-Identifier: GPL-3.0-only

//! Hybrid Mount VFS wire protocol: a single `hm_payload` page passed to
//! `add_key("hybridmount", "trigger", &payload_ptr)`. Byte codec only.

use crate::errors::{Error, Result};
use crate::vfs::rule::{VfsAction, VfsRule};

/// Must match `HYBRIDMOUNT_MAGIC_SIG` in `module/vfs/src/hybridmount.h`; the kernel
/// rejects the page with `-EFAULT` before parsing if it does not.
pub const MAGIC: u64 = 0x4859_4252_4944_4D4F;
pub const PAYLOAD_LEN: usize = 4096;
pub const BUFFER_LEN: usize = 4068;
pub const RULE_HEADER_LEN: usize = 12;
/// u32 uid followed by a u16 path length and the path bytes.
pub const DEL_HEADER_LEN: usize = 6;

pub const FLAG_WHITEOUT: u32 = 1 << 2;
/// Combined with a directory rule: the directory stays visible, real entries are
/// hidden and only injected children show through.
pub const FLAG_OPAQUE: u32 = 1 << 3;

// Command numbering is part of the wire contract and must not change. Only
// AddRule/DelRule/AddUid/GetVersion are issued today.
//
// `allow` rather than `expect`: the crate-level allow(dead_code) in main.rs covers the
// non-Linux targets, where `expect` would trip unfulfilled_lint_expectations under
// -D warnings.
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
    // The kernel leaves status at -1 unless it handled this payload.
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

/// `-ENOENT`: no such rule.
const KERNEL_ENOENT: i32 = -2;
/// `-EEXIST`: the uid is already isolated.
const KERNEL_EEXIST: i32 = -17;

/// Read a fixed-size field, checking that the response is long enough to hold it.
fn field<const N: usize>(payload: &[u8], offset: usize, name: &str) -> Result<[u8; N]> {
    let end = offset + N;
    payload
        .get(offset..end)
        .and_then(|raw| raw.try_into().ok())
        .ok_or_else(|| Error::VfsProtocol {
            detail: format!(
                "response is {} bytes, expected at least {end} for the {name} field",
                payload.len()
            ),
        })
}

fn read_status(payload: &[u8]) -> Result<i32> {
    Ok(i32::from_le_bytes(field(payload, 16, "status")?))
}

fn read_arg1(payload: &[u8]) -> Result<u32> {
    Ok(u32::from_le_bytes(field(payload, 20, "arg1")?))
}

fn read_data_size(payload: &[u8]) -> Result<u32> {
    Ok(u32::from_le_bytes(field(payload, 24, "data_size")?))
}

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

/// Tolerates `-ENOENT` so that deleting a rule that was never installed is not an error,
/// but still requires the batch cursor to be fully consumed.
pub fn ensure_status_allow_enoent(payload: &[u8]) -> Result<()> {
    ensure_status_allowing(payload, &[KERNEL_ENOENT])?;
    ensure_full_cursor(payload)
}

/// Tolerates `-EEXIST`: adding an isolated uid is idempotent.
pub fn ensure_status_allow_eexist(payload: &[u8]) -> Result<()> {
    ensure_status_allowing(payload, &[KERNEL_EEXIST])
}

/// On failure the kernel reports the first error's errno, with `arg1` set to the offset
/// of the record that failed.
pub fn ensure_consumed(payload: &[u8]) -> Result<()> {
    let status = read_status(payload)?;
    if status < 0 {
        return Err(Error::VfsProtocol {
            detail: format!(
                "kernel returned status {status} for the record at offset {}",
                read_arg1(payload)?
            ),
        });
    }
    ensure_full_cursor(payload)
}

/// A short cursor means records were left unprocessed, so the batch must not be treated
/// as applied even when status is 0 or `-ENOENT`.
fn ensure_full_cursor(payload: &[u8]) -> Result<()> {
    let arg1 = read_arg1(payload)?;
    let data_size = read_data_size(payload)?;
    if arg1 != data_size {
        return Err(Error::VfsProtocol {
            detail: format!("kernel consumed {arg1} of {data_size} bytes"),
        });
    }
    Ok(())
}

pub fn parse_version(payload: &[u8]) -> Result<String> {
    ensure_status(payload)?;
    // Clamp before adding to 28: len is a u32 and would overflow usize on 32-bit targets.
    let len = read_data_size(payload)? as usize;
    if len > BUFFER_LEN {
        return Err(Error::VfsProtocol {
            detail: format!("version length {len} exceeds {BUFFER_LEN}"),
        });
    }
    let raw = payload
        .get(28..28 + len)
        .ok_or_else(|| Error::VfsProtocol {
            detail: format!("response is too short for a {len} byte version"),
        })?;
    let text = std::str::from_utf8(raw).map_err(|_| Error::VfsProtocol {
        detail: "version is not valid utf-8".to_owned(),
    })?;
    Ok(text.trim().to_owned())
}

/// One rule as `GET_LIST` reports it. Paths are decoded lossily: a diagnostic listing
/// must not fail on a path the kernel accepted as opaque bytes.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct ListedRule {
    pub flags: u32,
    pub uid: u32,
    pub virtual_path: String,
    pub real_path: String,
}

/// A listing request carries no records; `arg1` is the index to resume from.
pub fn build_list_payload(cmd: NmCommand, cursor: u32) -> Result<Vec<u8>> {
    let mut page = build_payload(cmd, 0, &[])?;
    page[20..24].copy_from_slice(&cursor.to_le_bytes());
    Ok(page)
}

/// The response's `data_size` bytes, bounded by the fixed buffer.
fn response_body(payload: &[u8]) -> Result<&[u8]> {
    let len = read_data_size(payload)? as usize;
    if len > BUFFER_LEN {
        return Err(Error::VfsProtocol {
            detail: format!("response claims {len} bytes, exceeding {BUFFER_LEN}"),
        });
    }
    payload.get(28..28 + len).ok_or_else(|| Error::VfsProtocol {
        detail: format!("response is too short for its {len} byte body"),
    })
}

/// `GET_LIST` records plus the index of the next rule. Unlike the batch commands, `arg1`
/// is a rule index rather than a byte offset, so `ensure_full_cursor` does not apply.
pub fn parse_list(payload: &[u8]) -> Result<(Vec<ListedRule>, u32)> {
    ensure_status(payload)?;
    let body = response_body(payload)?;
    let mut rules = Vec::new();
    let mut pos = 0;

    while pos < body.len() {
        let header = body
            .get(pos..pos + RULE_HEADER_LEN)
            .ok_or_else(|| Error::VfsProtocol {
                detail: format!("truncated rule header at offset {pos}"),
            })?;
        let flags = u32::from_le_bytes(field(header, 0, "rule flags")?);
        let uid = u32::from_le_bytes(field(header, 4, "rule uid")?);
        let v_len = u16::from_le_bytes(field(header, 8, "rule v_len")?) as usize;
        let r_len = u16::from_le_bytes(field(header, 10, "rule r_len")?) as usize;
        pos += RULE_HEADER_LEN;

        let paths = body
            .get(pos..pos + v_len + r_len)
            .ok_or_else(|| Error::VfsProtocol {
                detail: format!(
                    "rule at offset {pos} claims {} path bytes but the batch ends early",
                    v_len + r_len
                ),
            })?;
        let (virtual_path, real_path) = paths.split_at(v_len);
        rules.push(ListedRule {
            flags,
            uid,
            virtual_path: String::from_utf8_lossy(virtual_path).into_owned(),
            real_path: String::from_utf8_lossy(real_path).into_owned(),
        });
        pos += v_len + r_len;
    }

    Ok((rules, read_arg1(payload)?))
}

/// Isolated uids plus the index of the next uid to read.
pub fn parse_uids(payload: &[u8]) -> Result<(Vec<u32>, u32)> {
    ensure_status(payload)?;
    let body = response_body(payload)?;
    if body.len() % std::mem::size_of::<u32>() != 0 {
        return Err(Error::VfsProtocol {
            detail: format!(
                "uid batch of {} bytes is not a whole number of u32",
                body.len()
            ),
        });
    }
    let uids = body
        .chunks_exact(4)
        .map(|word| u32::from_le_bytes([word[0], word[1], word[2], word[3]]))
        .collect();

    Ok((uids, read_arg1(payload)?))
}

/// Reads every page of a listing command. Both listing commands return a next-index in
/// `arg1` and an empty batch at the end.
pub fn paginate<T, F, P>(mut fetch: F, parse: P) -> Result<Vec<T>>
where
    F: FnMut(u32) -> Result<Vec<u8>>,
    P: Fn(&[u8]) -> Result<(Vec<T>, u32)>,
{
    let mut cursor = 0_u32;
    let mut all = Vec::new();
    loop {
        let payload = fetch(cursor)?;
        let (mut batch, next) = parse(&payload)?;
        if batch.is_empty() {
            return Ok(all);
        }
        // A non-empty batch that does not move the cursor would loop forever.
        if next <= cursor {
            return Err(Error::VfsProtocol {
                detail: format!(
                    "kernel returned cursor {next} for {} records requested from {cursor}",
                    batch.len()
                ),
            });
        }
        all.append(&mut batch);
        cursor = next;
    }
}

#[cfg(test)]
#[path = "protocol_tests.rs"]
mod tests;
