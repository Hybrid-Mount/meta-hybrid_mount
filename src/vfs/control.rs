// SPDX-License-Identifier: GPL-3.0-only

//! Runtime control and read-back verification, with an injectable keyring transport.

use std::collections::BTreeMap;

use serde::Serialize;

use super::backend::{KeyringKernel, SUPPORTED_VERSIONS, parse_version_response};
use super::protocol::{self, EncodedRule, ListedRule, NmCommand};
use crate::errors::{Error, Result};

pub(super) trait Transport {
    fn exchange(&mut self, request: &[u8]) -> Result<Vec<u8>>;
}

impl Transport for KeyringKernel {
    fn exchange(&mut self, request: &[u8]) -> Result<Vec<u8>> {
        KeyringKernel::exchange(self, request)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ClearScope {
    Rules,
    Uids,
    All,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Mutation {
    Add { rules: Vec<EncodedRule>, uid: u32 },
    Delete { paths: Vec<Vec<u8>>, uid: u32 },
    AddUids(Vec<u32>),
    DeleteUids(Vec<u32>),
    Clear(ClearScope),
}

impl Mutation {
    fn name(&self) -> &'static str {
        match self {
            Self::Add { .. } => "rule.add",
            Self::Delete { .. } => "rule.del",
            Self::AddUids(_) => "uid.add",
            Self::DeleteUids(_) => "uid.del",
            Self::Clear(ClearScope::Rules) => "rule.clear",
            Self::Clear(ClearScope::Uids) => "uid.clear",
            Self::Clear(ClearScope::All) => "clear.all",
        }
    }

    /// Encode every page before issuing any mutation, including later batches.
    fn pages(&self) -> Result<Vec<Vec<u8>>> {
        match self {
            Self::Add { rules, uid } => protocol::build_add_rule_payloads(rules, *uid),
            Self::Delete { paths, uid } => protocol::build_del_rule_payloads(paths, *uid),
            Self::AddUids(uids) | Self::DeleteUids(uids) => {
                let cmd = if matches!(self, Self::AddUids(_)) {
                    NmCommand::AddUid
                } else {
                    NmCommand::DelUid
                };
                uids.iter()
                    .map(|uid| protocol::build_payload(cmd, *uid, &[]))
                    .collect()
            }
            Self::Clear(scope) => {
                let cmd = match scope {
                    ClearScope::Rules => NmCommand::ClearRules,
                    ClearScope::Uids => NmCommand::ClearUids,
                    ClearScope::All => NmCommand::ClearAll,
                };
                Ok(vec![protocol::build_payload(cmd, 0, &[])?])
            }
        }
    }

    fn check_response(&self, response: &[u8]) -> Result<()> {
        match self {
            Self::Add { .. } => protocol::ensure_consumed(response),
            Self::Delete { .. } | Self::DeleteUids(_) => {
                protocol::ensure_status_allow_enoent(response)
            }
            Self::AddUids(_) => protocol::ensure_status_allow_eexist(response),
            Self::Clear(_) => protocol::ensure_status(response),
        }
    }
}

#[derive(Debug, Serialize)]
pub(super) struct VerifiedItem {
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uid: Option<u32>,
    pub confirmed: bool,
}

#[derive(Debug, Serialize)]
pub(super) struct MutationReport {
    pub operation: &'static str,
    pub ok: bool,
    pub results: Vec<VerifiedItem>,
    pub error: Option<String>,
    pub readback_error: Option<String>,
}

pub(super) struct Controller<T> {
    transport: T,
}

impl<T: Transport> Controller<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    pub fn version(&mut self) -> Result<String> {
        let request = protocol::build_payload(NmCommand::GetVersion, 0, &[])?;
        parse_version_response(&self.transport.exchange(&request)?)
    }

    /// Ordinary CLI operations never load a provider as a side effect.
    pub fn require_supported(&mut self) -> Result<String> {
        let found = self.version().map_err(|err| Error::VfsUnavailable {
            reason: format!("{err}; use 'hybrid-mount vfs doctor' to diagnose or 'hybrid-mount vfs load' to load explicitly"),
        })?;
        if !SUPPORTED_VERSIONS.contains(&found.as_str()) {
            return Err(Error::VfsUnsupportedVersion {
                found,
                supported: SUPPORTED_VERSIONS.join(","),
            });
        }
        Ok(found)
    }

    pub fn list_rules(&mut self) -> Result<Vec<ListedRule>> {
        protocol::paginate(
            |cursor| {
                self.transport
                    .exchange(&protocol::build_list_payload(NmCommand::GetList, cursor)?)
            },
            protocol::parse_list,
        )
    }

    pub fn list_uids(&mut self) -> Result<Vec<u32>> {
        protocol::paginate(
            |cursor| {
                self.transport
                    .exchange(&protocol::build_list_payload(NmCommand::GetUids, cursor)?)
            },
            protocol::parse_uids,
        )
    }

    pub fn mutate(&mut self, mutation: &Mutation) -> Result<MutationReport> {
        let pages = mutation.pages()?;
        let mut error = None;
        for (index, page) in pages.iter().enumerate() {
            let result = self
                .transport
                .exchange(page)
                .and_then(|response| mutation.check_response(&response));
            if let Err(err) = result {
                let item = match mutation {
                    Mutation::AddUids(uids) | Mutation::DeleteUids(uids) => {
                        format!("UID {}", uids[index])
                    }
                    _ => format!("batch {}", index + 1),
                };
                error = Some(format!(
                    "{} {item}: {err}; earlier records may have taken effect",
                    mutation.name()
                ));
                break;
            }
        }

        // Even a failed batch may have applied some records. Verify desired final state,
        // including records after the failing offset; never roll back an overwritten rule.
        let mut readback_errors = Vec::new();
        let mut results = Vec::new();
        match mutation {
            Mutation::Add { rules, uid } => {
                let listed = self
                    .list_rules()
                    .map_err(|err| readback_errors.push(err.to_string()))
                    .ok();
                // Repeated paths use the last requested value, matching kernel replacement.
                let expected: BTreeMap<&[u8], &EncodedRule> = rules
                    .iter()
                    .map(|rule| (rule.virtual_path.as_slice(), rule))
                    .collect();
                for (path, rule) in expected {
                    let confirmed = listed.as_ref().is_some_and(|rows| {
                        rows.iter().any(|row| {
                            row.uid == *uid
                                && row.virtual_path.as_bytes() == path
                                && row.real_path.as_bytes() == rule.real_path
                                && row.flags & (protocol::FLAG_WHITEOUT | protocol::FLAG_OPAQUE)
                                    == rule.flags
                                && (rule.flags & protocol::FLAG_OPAQUE == 0
                                    || row.flags & protocol::FLAG_VIRTUAL_DIR != 0)
                        })
                    });
                    results.push(VerifiedItem {
                        target: String::from_utf8_lossy(path).into_owned(),
                        uid: Some(*uid),
                        confirmed,
                    });
                }
            }
            Mutation::Delete { paths, uid } => {
                let listed = self
                    .list_rules()
                    .map_err(|err| readback_errors.push(err.to_string()))
                    .ok();
                for path in paths {
                    let confirmed = listed.as_ref().is_some_and(|rows| {
                        !rows
                            .iter()
                            .any(|row| row.uid == *uid && row.virtual_path.as_bytes() == path)
                    });
                    results.push(VerifiedItem {
                        target: String::from_utf8_lossy(path).into_owned(),
                        uid: Some(*uid),
                        confirmed,
                    });
                }
            }
            Mutation::AddUids(uids) | Mutation::DeleteUids(uids) => {
                let listed = self
                    .list_uids()
                    .map_err(|err| readback_errors.push(err.to_string()))
                    .ok();
                let present = matches!(mutation, Mutation::AddUids(_));
                for uid in uids {
                    results.push(VerifiedItem {
                        target: uid.to_string(),
                        uid: Some(*uid),
                        confirmed: listed
                            .as_ref()
                            .is_some_and(|rows| rows.contains(uid) == present),
                    });
                }
            }
            Mutation::Clear(scope) => {
                if matches!(scope, ClearScope::Rules | ClearScope::All) {
                    let listed = self
                        .list_rules()
                        .map_err(|err| readback_errors.push(err.to_string()))
                        .ok();
                    results.push(VerifiedItem {
                        target: "rules".into(),
                        uid: None,
                        confirmed: listed.is_some_and(|rows| rows.is_empty()),
                    });
                }
                if matches!(scope, ClearScope::Uids | ClearScope::All) {
                    let listed = self
                        .list_uids()
                        .map_err(|err| readback_errors.push(err.to_string()))
                        .ok();
                    results.push(VerifiedItem {
                        target: "uids".into(),
                        uid: None,
                        confirmed: listed.is_some_and(|rows| rows.is_empty()),
                    });
                }
            }
        }
        let readback_error = (!readback_errors.is_empty()).then(|| readback_errors.join("; "));
        let ok = error.is_none()
            && readback_error.is_none()
            && results.iter().all(|item| item.confirmed);
        Ok(MutationReport {
            operation: mutation.name(),
            ok,
            results,
            error,
            readback_error,
        })
    }
}

#[cfg(test)]
#[path = "control_tests.rs"]
mod tests;
