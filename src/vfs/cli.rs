// SPDX-License-Identifier: GPL-3.0-only

//! Runtime VFS CLI. Command vocabulary follows NoMount's nm at 016375cd4a9e7da0;
//! parsing, validation and output are implemented here for Hybrid Mount's hm1 protocol.

use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

use serde::Serialize;

use super::backend::KeyringKernel;
use super::control::{ClearScope, Controller, Mutation, MutationReport};
use super::doctor::{self, VfsDoctorReport};
use super::protocol::{self, EncodedRule, ListedRule};
use super::sys::KeyringChannel;
use crate::errors::{Error, Result};

const HELP: &str = "Hybrid Mount VFS runtime control
Usage: hybrid-mount vfs <command> [options]

  help | --help
  version [--json]                         Kernel protocol version (hm1)
  doctor [--json]                          Live provider, rules and isolated UIDs
  load [--json]                            Explicitly load and verify bundled VFS
  rule add <virtual> <real> [...] [--uid UID] [--json]
  rule add --whiteout <virtual> [...] [--uid UID] [--json]
  rule add --opaque <directory> [...] [--uid UID] [--json]
  rule del <virtual> [...] [--uid UID] [--json]
  rule list [--json]
  rule clear --yes [--json]
  uid add <UID> [...] [--json]
  uid del <UID> [...] [--json]
  uid list [--json]
  uid clear --yes [--json]
  clear {rules|uid|all} --yes [--json]

Aliases (inside vfs): add/a, del/d, whiteout/w, block/b, unblock/u,
  list/l, list uid/l uid, version/v/-v. Lists also accept bare 'json'.

Paths: virtual first, real second; relative paths use the current directory.
Use -- to end option parsing. --uid 0 (default) means a global rule.
'uid add' isolates a UID from VFS; it does not assign a rule to that UID.
--whiteout hides a path; --opaque keeps a directory with injected children only.
All clear commands affect the provider's shared tables, including boot rules.
Changes are runtime-only. No --save, wrapper binary or unload command is installed.
Queries and rule/UID commands never load a module; use 'vfs load' explicitly.
Writes are verified by read-back. Batches are not atomic; failures may be partial.
Exit codes: 0 success, 1 runtime/provider error, 2 invalid arguments.
";

#[derive(Debug, PartialEq, Eq)]
enum Operation {
    Help,
    Version,
    Doctor,
    Load,
    Rules,
    Uids,
    Mutate(Mutation),
}

#[derive(Debug, PartialEq, Eq)]
struct Command {
    operation: Operation,
    json: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Action {
    Help,
    Version,
    Doctor,
    Load,
    Rules,
    Uids,
    Add,
    Delete,
    AddUid,
    DeleteUid,
    Clear(ClearScope),
}

fn usage(detail: impl Into<String>) -> Error {
    Error::VfsCliUsage {
        detail: detail.into(),
    }
}

fn parse_uid(value: &str) -> Result<u32> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(usage(format!(
            "invalid UID {value:?}: expected a decimal u32"
        )));
    }
    value
        .parse()
        .map_err(|_| usage(format!("UID out of range: {value:?}")))
}

/// Lexical normalization preserves virtual paths that do not exist yet and avoids
/// following an active VFS redirection while constructing the rule's identity.
fn normalize_path(raw: &str, cwd: &Path) -> Result<Vec<u8>> {
    if raw.is_empty() || raw.contains('\0') {
        return Err(usage("paths must be nonempty and contain no NUL"));
    }
    let path = Path::new(raw);
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    };
    let mut normalized = PathBuf::new();
    for component in joined.components() {
        match component {
            Component::RootDir => normalized.push("/"),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(name) => normalized.push(name),
            Component::Prefix(_) => return Err(usage("VFS paths must be Unix paths")),
        }
    }
    let value = normalized
        .to_str()
        .ok_or_else(|| usage("VFS CLI paths must be UTF-8"))?;
    if !value.starts_with('/') || value.len() >= 4096 {
        return Err(usage(
            "normalized path must be absolute and shorter than 4096 bytes",
        ));
    }
    Ok(value.as_bytes().to_vec())
}

fn parse(args: &[String], cwd: &Path) -> Result<Command> {
    let Some(first) = args.first().map(String::as_str) else {
        return Ok(Command {
            operation: Operation::Help,
            json: false,
        });
    };
    let mut flags = 0;
    let (action, start) = match first {
        "help" | "--help" | "-h" => (Action::Help, 1),
        "version" | "v" | "-v" => (Action::Version, 1),
        "doctor" => (Action::Doctor, 1),
        "load" => (Action::Load, 1),
        "rule" | "uid" => {
            let second = args
                .get(1)
                .map(String::as_str)
                .ok_or_else(|| usage(format!("{first} requires add, del, list or clear")))?;
            let action = match (first, second) {
                (_, "help" | "--help" | "-h") => Action::Help,
                ("rule", "add") => Action::Add,
                ("rule", "del") => Action::Delete,
                ("rule", "list") => Action::Rules,
                ("rule", "clear") => Action::Clear(ClearScope::Rules),
                ("uid", "add") => Action::AddUid,
                ("uid", "del") => Action::DeleteUid,
                ("uid", "list") => Action::Uids,
                ("uid", "clear") => Action::Clear(ClearScope::Uids),
                _ => return Err(usage(format!("unknown command: {first} {second}"))),
            };
            (action, 2)
        }
        "clear" => {
            let scope = match args.get(1).map(String::as_str) {
                Some("rules") => ClearScope::Rules,
                Some("uid") => ClearScope::Uids,
                Some("all") => ClearScope::All,
                Some("--help" | "-h") => {
                    return Ok(Command {
                        operation: Operation::Help,
                        json: false,
                    });
                }
                _ => {
                    return Err(usage(
                        "clear requires an explicit target: rules, uid or all",
                    ));
                }
            };
            (Action::Clear(scope), 2)
        }
        "add" | "a" => (Action::Add, 1),
        "del" | "d" => (Action::Delete, 1),
        "whiteout" | "w" => {
            flags = protocol::FLAG_WHITEOUT;
            (Action::Add, 1)
        }
        "block" | "b" => (Action::AddUid, 1),
        "unblock" | "u" => (Action::DeleteUid, 1),
        "list" | "l" => {
            if args.get(1).is_some_and(|arg| arg == "uid") {
                (Action::Uids, 2)
            } else {
                (Action::Rules, 1)
            }
        }
        _ => return Err(usage(format!("unknown VFS command: {first}"))),
    };
    let mut json = false;
    let mut uid = None;
    let mut yes = false;
    let mut options = true;
    let mut values = Vec::new();
    let mut iter = args[start..].iter();
    while let Some(arg) = iter.next() {
        if options && arg == "--" {
            options = false;
            continue;
        }
        if options {
            match arg.as_str() {
                "--help" | "-h" => {
                    return Ok(Command {
                        operation: Operation::Help,
                        json: false,
                    });
                }
                "--json" => {
                    json = true;
                    continue;
                }
                "json" if matches!(action, Action::Rules | Action::Uids) => {
                    json = true;
                    continue;
                }
                "--yes" if matches!(action, Action::Clear(_)) => {
                    yes = true;
                    continue;
                }
                "--whiteout" | "--opaque" if action == Action::Add => {
                    let flag = if arg == "--whiteout" {
                        protocol::FLAG_WHITEOUT
                    } else {
                        protocol::FLAG_OPAQUE
                    };
                    if flags != 0 && flags != flag {
                        return Err(usage("--whiteout and --opaque are mutually exclusive"));
                    }
                    flags = flag;
                    continue;
                }
                _ => {}
            }
            if arg == "--uid" || arg.starts_with("--uid=") {
                if !matches!(action, Action::Add | Action::Delete) {
                    return Err(usage("--uid is only valid with rule add/del"));
                }
                if uid.is_some() {
                    return Err(usage("--uid may only be specified once"));
                }
                let value = match arg.strip_prefix("--uid=") {
                    Some(value) => value,
                    None => iter
                        .next()
                        .map(String::as_str)
                        .ok_or_else(|| usage("--uid requires a value"))?,
                };
                uid = Some(parse_uid(value)?);
                continue;
            }
            if arg.starts_with('-') {
                return Err(usage(format!("unknown or inapplicable option: {arg}")));
            }
        }
        values.push(arg.as_str());
    }
    let uid = uid.unwrap_or(0);
    let operation = match action {
        Action::Add => {
            let step = if flags == 0 { 2 } else { 1 };
            if values.is_empty() || values.len() % step != 0 {
                return Err(usage(
                    "rule add requires complete virtual/real pairs, or paths with --whiteout/--opaque",
                ));
            }
            let mut rules = Vec::new();
            for chunk in values.chunks(step) {
                let virtual_path = normalize_path(chunk[0], cwd)?;
                let real_path = if step == 2 {
                    normalize_path(chunk[1], cwd)?
                } else {
                    Vec::new()
                };
                let rule = EncodedRule {
                    flags,
                    virtual_path,
                    real_path,
                };
                if rule.record_len() > protocol::BUFFER_LEN {
                    return Err(usage("rule exceeds the 4068-byte protocol record limit"));
                }
                rules.push(rule);
            }
            Operation::Mutate(Mutation::Add { rules, uid })
        }
        Action::Delete => {
            if values.is_empty() {
                return Err(usage("rule del requires at least one virtual path"));
            }
            let paths = values
                .iter()
                .map(|value| {
                    let path = normalize_path(value, cwd)?;
                    if protocol::DEL_HEADER_LEN + path.len() > protocol::BUFFER_LEN {
                        return Err(usage("delete record exceeds 4068 bytes"));
                    }
                    Ok(path)
                })
                .collect::<Result<Vec<_>>>()?;
            Operation::Mutate(Mutation::Delete { paths, uid })
        }
        Action::AddUid | Action::DeleteUid => {
            if values.is_empty() {
                return Err(usage("uid add/del requires at least one UID"));
            }
            let mut uids = values
                .iter()
                .map(|value| parse_uid(value))
                .collect::<Result<Vec<_>>>()?;
            uids.sort_unstable();
            uids.dedup();
            Operation::Mutate(if action == Action::AddUid {
                Mutation::AddUids(uids)
            } else {
                Mutation::DeleteUids(uids)
            })
        }
        _ => {
            if !values.is_empty() {
                return Err(usage(format!("unexpected argument: {:?}", values[0])));
            }
            match action {
                Action::Help => Operation::Help,
                Action::Version => Operation::Version,
                Action::Doctor => Operation::Doctor,
                Action::Load => Operation::Load,
                Action::Rules => Operation::Rules,
                Action::Uids => Operation::Uids,
                Action::Clear(scope) => {
                    if !yes {
                        return Err(usage(
                            "clearing shared VFS tables requires --yes (including rules created at boot)",
                        ));
                    }
                    Operation::Mutate(Mutation::Clear(scope))
                }
                _ => return Err(usage("invalid VFS action")),
            }
        }
    };
    Ok(Command { operation, json })
}

#[derive(Serialize)]
struct RuleOutput<'a> {
    #[serde(flatten)]
    rule: &'a ListedRule,
    whiteout: bool,
    virtual_dir: bool,
    opaque: bool,
}

fn rules_json(rules: &[ListedRule]) -> Result<String> {
    let rows: Vec<_> = rules
        .iter()
        .map(|rule| RuleOutput {
            rule,
            whiteout: rule.flags & protocol::FLAG_WHITEOUT != 0,
            virtual_dir: rule.flags & protocol::FLAG_VIRTUAL_DIR != 0,
            opaque: rule.flags & protocol::FLAG_OPAQUE != 0,
        })
        .collect();
    Ok(serde_json::to_string_pretty(&rows)?)
}

fn rules_text(rules: &[ListedRule]) -> String {
    if rules.is_empty() {
        return "No VFS rules.\n".into();
    }
    rules
        .iter()
        .map(|rule| {
            let action = if rule.flags & protocol::FLAG_WHITEOUT != 0 {
                "(whiteout)".into()
            } else if rule.flags & protocol::FLAG_OPAQUE != 0 {
                "(opaque directory)".into()
            } else if rule.flags & protocol::FLAG_VIRTUAL_DIR != 0 {
                "(virtual directory)".into()
            } else {
                format!("-> {:?}", rule.real_path)
            };
            format!(
                "{:?} {action} [UID: {}, flags: {}]\n",
                rule.virtual_path, rule.uid, rule.flags
            )
        })
        .collect()
}

fn doctor_text(report: &VfsDoctorReport) -> String {
    let mut text = format!(
        "Provider: {} ({})\nPresence: {:?}\nVersion: {}\nSupported: {}\nResponds: {}\n{}\n",
        report.provider,
        report.key_type,
        report.presence,
        report.version.as_deref().unwrap_or("unavailable"),
        report.supported_versions.join(", "),
        report.responds,
        report.diagnosis
    );
    for error in [&report.probe_error, &report.list_error]
        .into_iter()
        .flatten()
    {
        text.push_str(&format!("Error: {error}\n"));
    }
    text.push_str(&rules_text(&report.rules));
    text.push_str(&format!("Isolated UIDs: {:?}\n", report.uids));
    text
}

fn mutation_text(report: &MutationReport) -> String {
    let mut text = format!(
        "{}: {}\n",
        report.operation,
        if report.ok {
            "verified"
        } else {
            "failed or partially applied"
        }
    );
    for item in &report.results {
        let uid = item
            .uid
            .map(|uid| format!(" [UID: {uid}]"))
            .unwrap_or_default();
        text.push_str(&format!(
            "  {} {:?}{uid}\n",
            if item.confirmed {
                "confirmed"
            } else {
                "unconfirmed"
            },
            item.target
        ));
    }
    text
}

fn emit(text: &str) -> Result<()> {
    let mut out = io::stdout().lock();
    out.write_all(text.as_bytes())?;
    if !text.ends_with('\n') {
        out.write_all(b"\n")?;
    }
    Ok(())
}

/// Parses and validates the whole request before touching the provider.
pub fn handle(args: &[String]) -> Result<()> {
    // Help and absolute-path commands still work if the caller's cwd was removed.
    let cwd = std::env::current_dir().unwrap_or_default();
    let command = parse(args, &cwd)?;
    // All provider mutations share the lifecycle mutex, including low-level CLI users.
    #[cfg(any(target_os = "linux", target_os = "android"))]
    let _operation_lock = if matches!(command.operation, Operation::Load | Operation::Mutate(_)) {
        Some(crate::runtime::ledger::OperationLock::acquire()?)
    } else {
        None
    };
    match command.operation {
        Operation::Help => emit(HELP),
        Operation::Doctor => {
            let report = doctor::report();
            emit(&if command.json {
                serde_json::to_string_pretty(&report)?
            } else {
                doctor_text(&report)
            })
        }
        Operation::Load => {
            let version = load_provider()?;
            emit(&if command.json {
                serde_json::to_string_pretty(
                    &serde_json::json!({"operation": "load", "ok": true, "version": version, "presence": doctor::presence_on_device()}),
                )?
            } else {
                format!(
                    "hybridmount ready: {version} ({:?})",
                    doctor::presence_on_device()
                )
            })
        }
        operation => {
            let mut controller = Controller::new(KeyringKernel::new(KeyringChannel::Hybridmount)?);
            if operation == Operation::Version {
                let version = controller.version()?;
                return emit(&if command.json {
                    serde_json::to_string_pretty(&serde_json::json!({"version": version}))?
                } else {
                    version
                });
            }
            controller.require_supported()?;
            match operation {
                Operation::Rules => {
                    let rules = controller.list_rules()?;
                    emit(&if command.json {
                        rules_json(&rules)?
                    } else {
                        rules_text(&rules)
                    })
                }
                Operation::Uids => {
                    let uids = controller.list_uids()?;
                    emit(&if command.json {
                        serde_json::to_string_pretty(&uids)?
                    } else {
                        format!("Isolated UIDs: {uids:?}")
                    })
                }
                Operation::Mutate(mutation) => {
                    let report = controller.mutate(&mutation)?;
                    emit(&if command.json {
                        serde_json::to_string_pretty(&report)?
                    } else {
                        mutation_text(&report)
                    })?;
                    if !report.ok {
                        return Err(Error::msg(format!(
                            "{} was not fully verified: {}; read-back: {}",
                            report.operation,
                            report
                                .error
                                .as_deref()
                                .unwrap_or("kernel accepted the request"),
                            report
                                .readback_error
                                .as_deref()
                                .unwrap_or("see confirmed/unconfirmed results on stdout")
                        )));
                    }
                    Ok(())
                }
                _ => Err(Error::msg("invalid VFS operation")),
            }
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn load_provider() -> Result<String> {
    super::lkm::load_explicit()
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn load_provider() -> Result<String> {
    Err(Error::msg("vfs load is only supported on linux/android"))
}

#[cfg(test)]
#[path = "cli_tests.rs"]
mod tests;
