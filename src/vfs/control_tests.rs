// SPDX-License-Identifier: GPL-3.0-only

use std::collections::VecDeque;

use super::*;

struct Scripted {
    replies: VecDeque<Result<Vec<u8>>>,
    requests: Vec<Vec<u8>>,
}

impl Scripted {
    fn new(replies: Vec<Result<Vec<u8>>>) -> Self {
        Self {
            replies: replies.into(),
            requests: Vec::new(),
        }
    }
}

impl Transport for Scripted {
    fn exchange(&mut self, request: &[u8]) -> Result<Vec<u8>> {
        self.requests.push(request.to_vec());
        self.replies
            .pop_front()
            .expect("unexpected keyring request")
    }
}

fn page(cmd: NmCommand, status: i32, cursor: u32, body: &[u8]) -> Result<Vec<u8>> {
    let mut page = protocol::build_payload(cmd, 0, body)?;
    page[16..20].copy_from_slice(&status.to_le_bytes());
    page[20..24].copy_from_slice(&cursor.to_le_bytes());
    Ok(page)
}

fn rule(path: &str, real: &str, flags: u32) -> EncodedRule {
    EncodedRule {
        virtual_path: path.as_bytes().to_vec(),
        real_path: real.as_bytes().to_vec(),
        flags,
    }
}

fn rules_page(rules: &[(&EncodedRule, u32)], cursor: u32) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    for (rule, uid) in rules {
        rule.write_into(&mut bytes, *uid);
    }
    page(NmCommand::GetList, 0, cursor, &bytes)
}

fn word(page: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(page[offset..offset + 4].try_into().unwrap())
}

#[test]
fn nonzero_uid_is_encoded_in_every_record_across_batch_boundaries() {
    let one = rule(
        &format!("/{}", "v".repeat(1500)),
        &format!("/{}", "r".repeat(1500)),
        0,
    );
    let small = rule("/small", "/real", 0);
    let pages =
        protocol::build_add_rule_payloads(&[one.clone(), one.clone(), small], 10234).unwrap();
    assert_eq!(pages.len(), 2);
    for page in &pages {
        assert_eq!(word(page, 12), 10234);
        let (rows, _) = protocol::parse_list(&{
            let mut response = page.clone();
            response[16..20].copy_from_slice(&0_i32.to_le_bytes());
            response
        })
        .unwrap();
        assert!(rows.iter().all(|row| row.uid == 10234));
    }
    assert_eq!(
        word(
            &protocol::build_add_rule_payloads(&[one], 0).unwrap()[0],
            32
        ),
        0
    );
}

#[test]
fn addition_requires_matching_uid_real_path_and_semantic_flags() {
    let expected = rule("/virtual", "/real", 0);
    for (actual, uid) in [
        (expected.clone(), 0),
        (rule("/virtual", "/wrong", 0), 42),
        (rule("/virtual", "/real", protocol::FLAG_WHITEOUT), 42),
    ] {
        let mut controller = Controller::new(Scripted::new(vec![
            page(
                NmCommand::AddRule,
                0,
                expected.record_len() as u32,
                &vec![0; expected.record_len()],
            ),
            rules_page(&[(&actual, uid)], 1),
            rules_page(&[], 1),
        ]));
        let report = controller
            .mutate(&Mutation::Add {
                rules: vec![expected.clone()],
                uid: 42,
            })
            .unwrap();
        assert!(!report.ok);
        assert!(!report.results[0].confirmed);
    }
}

#[test]
fn opaque_readback_accepts_kernel_added_directory_flags() {
    let expected = rule("/dir", "", protocol::FLAG_OPAQUE);
    let actual = rule(
        "/dir",
        "",
        protocol::FLAG_OPAQUE | protocol::FLAG_VIRTUAL_DIR | 1,
    );
    let mut controller = Controller::new(Scripted::new(vec![
        page(
            NmCommand::AddRule,
            0,
            expected.record_len() as u32,
            &vec![0; expected.record_len()],
        ),
        rules_page(&[(&actual, 44)], 1),
        rules_page(&[], 1),
    ]));
    let report = controller
        .mutate(&Mutation::Add {
            rules: vec![expected],
            uid: 44,
        })
        .unwrap();
    assert!(report.ok, "{report:?}");
    assert_eq!(word(&controller.transport.requests[0], 32), 44);
}

#[test]
fn failed_batch_still_reads_all_final_results_and_preserves_error_offset() {
    let first = rule("/a", "/real-a", 0);
    let second = rule("/b", "/missing", 0);
    let mut controller = Controller::new(Scripted::new(vec![
        page(
            NmCommand::AddRule,
            -2,
            first.record_len() as u32,
            &vec![0; first.record_len() + second.record_len()],
        ),
        rules_page(&[(&first, 42)], 1),
        rules_page(&[], 1),
    ]));
    let report = controller
        .mutate(&Mutation::Add {
            rules: vec![first.clone(), second],
            uid: 42,
        })
        .unwrap();
    assert!(!report.ok);
    assert!(report.results[0].confirmed);
    assert!(!report.results[1].confirmed);
    assert!(
        report
            .error
            .unwrap()
            .contains(&format!("offset {}", first.record_len()))
    );
    assert_eq!(controller.transport.requests.len(), 3);
}

#[test]
fn duplicate_path_verifies_last_requested_value() {
    let before = rule("/v", "/before", 0);
    let after = rule("/v", "/after", 0);
    let size = before.record_len() + after.record_len();
    let mut controller = Controller::new(Scripted::new(vec![
        page(NmCommand::AddRule, 0, size as u32, &vec![0; size]),
        rules_page(&[(&after, 0)], 1),
        rules_page(&[], 1),
    ]));
    let report = controller
        .mutate(&Mutation::Add {
            rules: vec![before, after],
            uid: 0,
        })
        .unwrap();
    assert!(report.ok);
    assert_eq!(report.results.len(), 1);
}

#[test]
fn delete_is_idempotent_and_does_not_match_another_uid() {
    let other_uid_rule = rule("/v", "/r", 0);
    let mut controller = Controller::new(Scripted::new(vec![
        page(NmCommand::DelRule, -2, 8, &[0; 8]),
        rules_page(&[(&other_uid_rule, 7)], 1),
        rules_page(&[], 1),
    ]));
    let report = controller
        .mutate(&Mutation::Delete {
            paths: vec![b"/v".to_vec()],
            uid: 42,
        })
        .unwrap();
    assert!(report.ok);
    assert_eq!(word(&controller.transport.requests[0], 28), 42);
}

#[test]
fn short_delete_cursor_is_failure_even_when_rule_is_absent() {
    let mut controller = Controller::new(Scripted::new(vec![
        page(NmCommand::DelRule, -2, 0, &[0; 8]),
        rules_page(&[], 0),
    ]));
    let report = controller
        .mutate(&Mutation::Delete {
            paths: vec![b"/v".to_vec()],
            uid: 0,
        })
        .unwrap();
    assert!(!report.ok);
    assert!(report.results[0].confirmed);
    assert!(report.error.unwrap().contains("consumed 0 of 8"));
}

#[test]
fn uid_add_and_delete_tolerate_only_expected_idempotency_errors() {
    let mut controller = Controller::new(Scripted::new(vec![
        page(NmCommand::AddUid, -17, 0, &[]),
        page(NmCommand::GetUids, 0, 1, &42_u32.to_le_bytes()),
        page(NmCommand::GetUids, 0, 1, &[]),
        page(NmCommand::DelUid, -2, 0, &[]),
        page(NmCommand::GetUids, 0, 0, &[]),
    ]));
    assert!(controller.mutate(&Mutation::AddUids(vec![42])).unwrap().ok);
    assert!(
        controller
            .mutate(&Mutation::DeleteUids(vec![42]))
            .unwrap()
            .ok
    );
    assert_eq!(
        word(&controller.transport.requests[0], 8),
        NmCommand::AddUid as u32
    );
    assert_eq!(word(&controller.transport.requests[0], 12), 42);
    assert_eq!(
        word(&controller.transport.requests[3], 8),
        NmCommand::DelUid as u32
    );
    let mut failed = Controller::new(Scripted::new(vec![
        page(NmCommand::DelUid, -12, 0, &[]),
        page(NmCommand::GetUids, 0, 0, &[]),
    ]));
    let report = failed.mutate(&Mutation::DeleteUids(vec![42])).unwrap();
    assert!(!report.ok);
    assert!(report.error.unwrap().contains("UID 42"));
}

#[test]
fn each_clear_command_reads_back_only_its_affected_tables() {
    for (scope, cmd, queries) in [
        (
            ClearScope::Rules,
            NmCommand::ClearRules,
            vec![NmCommand::GetList],
        ),
        (
            ClearScope::Uids,
            NmCommand::ClearUids,
            vec![NmCommand::GetUids],
        ),
        (
            ClearScope::All,
            NmCommand::ClearAll,
            vec![NmCommand::GetList, NmCommand::GetUids],
        ),
    ] {
        let mut replies = vec![page(cmd, 0, 0, &[])];
        replies.extend(queries.iter().map(|cmd| page(*cmd, 0, 0, &[])));
        let mut controller = Controller::new(Scripted::new(replies));
        let report = controller.mutate(&Mutation::Clear(scope)).unwrap();
        assert!(report.ok, "{scope:?}");
        let commands: Vec<_> = controller
            .transport
            .requests
            .iter()
            .map(|p| word(p, 8))
            .collect();
        assert_eq!(
            commands,
            [
                vec![cmd as u32],
                queries.iter().map(|c| *c as u32).collect()
            ]
            .concat()
        );
    }
}

#[test]
fn clear_all_collects_second_table_when_first_readback_fails() {
    let mut controller = Controller::new(Scripted::new(vec![
        page(NmCommand::ClearAll, 0, 0, &[]),
        Err(Error::msg("rule read failed")),
        page(NmCommand::GetUids, 0, 0, &[]),
    ]));
    let report = controller
        .mutate(&Mutation::Clear(ClearScope::All))
        .unwrap();
    assert!(!report.ok);
    assert!(!report.results[0].confirmed);
    assert!(report.results[1].confirmed);
    assert!(report.readback_error.unwrap().contains("rule read failed"));
}

#[test]
fn rule_and_uid_listing_follow_cursors_and_propagate_late_errors() {
    let a = rule("/a", "/x", 0);
    let b = rule("/b", "/y", 0);
    let mut controller = Controller::new(Scripted::new(vec![
        rules_page(&[(&a, 3)], 1),
        rules_page(&[(&b, 4)], 2),
        rules_page(&[], 2),
        page(NmCommand::GetUids, 0, 1, &3_u32.to_le_bytes()),
        Err(Error::msg("second uid page failed")),
    ]));
    assert_eq!(controller.list_rules().unwrap().len(), 2);
    assert!(
        controller
            .list_uids()
            .unwrap_err()
            .to_string()
            .contains("second uid page failed")
    );
    assert_eq!(
        controller
            .transport
            .requests
            .iter()
            .map(|p| word(p, 20))
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 0, 1]
    );
}

#[test]
fn version_probe_and_queries_issue_no_mutations() {
    let mut controller = Controller::new(Scripted::new(vec![
        page(NmCommand::GetVersion, 0, 0, b"hm1"),
        rules_page(&[], 0),
        page(NmCommand::GetUids, 0, 0, &[]),
    ]));
    assert_eq!(controller.require_supported().unwrap(), "hm1");
    assert!(controller.list_rules().unwrap().is_empty());
    assert!(controller.list_uids().unwrap().is_empty());
    assert_eq!(
        controller
            .transport
            .requests
            .iter()
            .map(|p| word(p, 8))
            .collect::<Vec<_>>(),
        vec![1, 9, 10]
    );
    let mut unsupported = Controller::new(Scripted::new(vec![page(
        NmCommand::GetVersion,
        0,
        0,
        b"20",
    )]));
    assert!(matches!(
        unsupported.require_supported(),
        Err(Error::VfsUnsupportedVersion { .. })
    ));
    assert_eq!(unsupported.transport.requests.len(), 1);
}

#[test]
fn unavailable_provider_returns_actionable_error_without_loading() {
    let mut controller = Controller::new(Scripted::new(vec![Err(Error::msg("no key type"))]));
    let error = controller.require_supported().unwrap_err();
    assert!(error.to_string().contains("vfs load"));
    assert_eq!(controller.transport.requests.len(), 1);
}

#[test]
fn oversized_late_record_is_rejected_before_any_mutation() {
    let mut controller = Controller::new(Scripted::new(Vec::new()));
    let mutation = Mutation::Add {
        rules: vec![rule("/ok", "/r", 0), rule(&"x".repeat(4069), "/r", 0)],
        uid: 4,
    };
    assert!(controller.mutate(&mutation).is_err());
    assert!(controller.transport.requests.is_empty());
}

#[test]
fn failed_uid_stops_later_mutations_and_reports_each_requested_state() {
    let mut controller = Controller::new(Scripted::new(vec![
        page(NmCommand::AddUid, 0, 0, &[]),
        page(NmCommand::AddUid, -12, 0, &[]),
        page(NmCommand::GetUids, 0, 1, &10_u32.to_le_bytes()),
        page(NmCommand::GetUids, 0, 1, &[]),
    ]));
    let report = controller
        .mutate(&Mutation::AddUids(vec![10, 20, 30]))
        .unwrap();
    assert!(!report.ok);
    assert_eq!(
        report
            .results
            .iter()
            .map(|item| item.confirmed)
            .collect::<Vec<_>>(),
        vec![true, false, false]
    );
    assert!(report.error.as_deref().unwrap().contains("UID 20"));
    assert_eq!(
        controller
            .transport
            .requests
            .iter()
            .map(|page| word(page, 8))
            .collect::<Vec<_>>(),
        vec![4, 4, 10, 10]
    );
    let json = serde_json::to_value(report).unwrap();
    assert_eq!(json["ok"], false);
    assert_eq!(json["results"][2]["uid"], 30);
}

#[test]
fn clear_success_is_not_reported_when_the_table_still_has_rules() {
    let stale = rule("/stale", "/real", 0);
    let mut controller = Controller::new(Scripted::new(vec![
        page(NmCommand::ClearRules, 0, 0, &[]),
        rules_page(&[(&stale, 0)], 1),
        rules_page(&[], 1),
    ]));
    let report = controller
        .mutate(&Mutation::Clear(ClearScope::Rules))
        .unwrap();
    assert!(!report.ok);
    assert!(!report.results[0].confirmed);
}

#[test]
fn whiteout_and_directory_injection_verify_successfully() {
    for (expected, actual) in [
        (
            rule("/white", "", protocol::FLAG_WHITEOUT),
            rule("/white", "", protocol::FLAG_WHITEOUT),
        ),
        (rule("/dir", "/real-dir", 0), rule("/dir", "/real-dir", 1)),
    ] {
        let mut controller = Controller::new(Scripted::new(vec![
            page(
                NmCommand::AddRule,
                0,
                expected.record_len() as u32,
                &vec![0; expected.record_len()],
            ),
            rules_page(&[(&actual, 0)], 1),
            rules_page(&[], 1),
        ]));
        let report = controller
            .mutate(&Mutation::Add {
                rules: vec![expected],
                uid: 0,
            })
            .unwrap();
        assert!(report.ok, "{report:?}");
    }
}
