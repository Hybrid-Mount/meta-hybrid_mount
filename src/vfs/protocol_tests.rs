// SPDX-License-Identifier: GPL-3.0-only

use super::*;
use crate::module_id::ModuleId;
use crate::vfs::rule::{VfsAction, VfsRule};
use std::path::PathBuf;

/// A response page as the kernel would write it back: `build_payload` leaves the -1
/// sentinel, overwritten here with the real status / arg1 / data_size.
fn response(status: i32, arg1: u32, data_size: u32) -> Vec<u8> {
    let mut page = build_payload(NmCommand::DelRule, 0, &[]).unwrap();
    page[16..20].copy_from_slice(&status.to_le_bytes());
    page[20..24].copy_from_slice(&arg1.to_le_bytes());
    page[24..28].copy_from_slice(&data_size.to_le_bytes());
    page
}

#[test]
fn payload_has_exact_wire_layout() {
    let page = build_payload(NmCommand::GetVersion, 0, &[]).unwrap();
    assert_eq!(page.len(), PAYLOAD_LEN);
    assert_eq!(&page[0..8], &MAGIC.to_le_bytes());
    assert_eq!(u32::from_le_bytes(page[8..12].try_into().unwrap()), 1);
    assert_eq!(u32::from_le_bytes(page[12..16].try_into().unwrap()), 0);
    assert_eq!(i32::from_le_bytes(page[16..20].try_into().unwrap()), -1_i32);
    assert_eq!(u32::from_le_bytes(page[20..24].try_into().unwrap()), 0);
    assert_eq!(u32::from_le_bytes(page[24..28].try_into().unwrap()), 0);
}

/// Checks the constants against the kernel header so the two cannot drift apart.
///
/// A mismatch makes the kernel answer -EFAULT and VFS degrade silently with no rule
/// applied, which is hard to diagnose. The `page[0..8] == MAGIC.to_le_bytes()` assertion
/// in `payload_has_exact_wire_layout` is self-referential and would pass for any value.
#[test]
fn wire_magic_and_version_match_the_kernel_header() {
    let header = include_str!("../../module/vfs/src/hybridmount.h");

    let magic_line = header
        .lines()
        .find(|line| line.starts_with("#define HYBRIDMOUNT_MAGIC_SIG"))
        .expect("hybridmount.h is missing HYBRIDMOUNT_MAGIC_SIG");
    let raw = magic_line
        .split_whitespace()
        .nth(2)
        .and_then(|value| value.strip_suffix("ULL"))
        .expect("HYBRIDMOUNT_MAGIC_SIG is not a ULL literal");
    assert_eq!(
        u64::from_str_radix(raw.trim_start_matches("0x"), 16).unwrap(),
        MAGIC,
        "内核 HYBRIDMOUNT_MAGIC_SIG 与用户态 MAGIC 不一致，内核会以 -EFAULT 拒绝所有 payload"
    );

    let page = build_payload(NmCommand::GetVersion, 0, &[]).unwrap();
    assert_eq!(&page[0..8], b"OMDIRBYH", "小端序写入后 wire 上的 8 字节");

    let version_line = header
        .lines()
        .find(|line| line.starts_with("#define HYBRIDMOUNT_VERSION"))
        .expect("hybridmount.h is missing HYBRIDMOUNT_VERSION");
    let version = version_line
        .split('"')
        .nth(1)
        .expect("HYBRIDMOUNT_VERSION is not a string literal");
    assert!(
        crate::vfs::backend::SUPPORTED_VERSIONS.contains(&version),
        "内核版本串 {version} 不在用户态 SUPPORTED_VERSIONS 内，Provider 会被判为不支持"
    );
}

#[test]
fn add_rule_record_encodes_header_and_paths() {
    let rule = VfsRule {
        action: VfsAction::Inject {
            virtual_path: "/system/etc/hosts".to_owned(),
            real_path: PathBuf::from("/data/local/tmp/hosts"),
        },
        module_id: ModuleId::try_from("m").unwrap(),
    };
    let encoded = encode_rule(&rule).unwrap();
    assert_eq!(encoded.flags, 0);
    assert_eq!(
        encoded.record_len(),
        RULE_HEADER_LEN + "/system/etc/hosts".len() + "/data/local/tmp/hosts".len()
    );

    let pages = build_add_rule_payloads(&[encoded], 0).unwrap();
    assert_eq!(pages.len(), 1);
    let buffer = &pages[0][28..];
    assert_eq!(u32::from_le_bytes(buffer[0..4].try_into().unwrap()), 0);
    assert_eq!(u32::from_le_bytes(buffer[4..8].try_into().unwrap()), 0);
    assert_eq!(
        u16::from_le_bytes(buffer[8..10].try_into().unwrap()),
        "/system/etc/hosts".len() as u16
    );
    assert_eq!(
        u16::from_le_bytes(buffer[10..12].try_into().unwrap()),
        "/data/local/tmp/hosts".len() as u16
    );
    assert_eq!(&buffer[12..12 + 17], b"/system/etc/hosts");
    assert_eq!(
        u32::from_le_bytes(pages[0][8..12].try_into().unwrap()),
        NmCommand::AddRule as u32
    );
}

#[test]
fn whiteout_rule_sets_flag_and_empty_real_path() {
    let rule = VfsRule {
        action: VfsAction::Whiteout {
            virtual_path: "/system/etc/hidden.xml".to_owned(),
        },
        module_id: ModuleId::try_from("m").unwrap(),
    };
    let encoded = encode_rule(&rule).unwrap();
    assert_eq!(encoded.flags, FLAG_WHITEOUT);
    assert!(encoded.real_path.is_empty());
    assert_eq!(
        encoded.record_len(),
        RULE_HEADER_LEN + "/system/etc/hidden.xml".len()
    );
}

#[test]
fn batches_split_when_buffer_is_full() {
    let rule = VfsRule {
        action: VfsAction::Inject {
            virtual_path: "/system/etc/hosts".to_owned(),
            real_path: PathBuf::from("/data/local/tmp/hosts"),
        },
        module_id: ModuleId::try_from("m").unwrap(),
    };
    let repeated = vec![encode_rule(&rule).unwrap(); 200];
    let pages = build_add_rule_payloads(&repeated, 0).unwrap();
    assert!(pages.len() > 1);
    for page in &pages {
        assert_eq!(page.len(), PAYLOAD_LEN);
        let data_size = u32::from_le_bytes(page[24..28].try_into().unwrap()) as usize;
        assert!(data_size <= BUFFER_LEN);
    }
}

#[test]
fn parse_version_reads_buffer_and_len() {
    let mut page = build_payload(NmCommand::GetVersion, 0, &[]).unwrap();
    // Status was written back as success.
    page[16..20].copy_from_slice(&0_i32.to_le_bytes());
    page[28..30].copy_from_slice(b"20");
    page[24..28].copy_from_slice(&2_u32.to_le_bytes());
    assert_eq!(parse_version(&page).unwrap(), "20");
}

#[test]
fn ensure_status_rejects_negative_kernel_status() {
    let page = response(-22, 0, 0);
    let err = ensure_status(&page).unwrap_err();
    assert!(matches!(err, crate::errors::Error::VfsProtocol { .. }));
}

#[test]
fn del_rule_record_encodes_uid_v_len_and_path() {
    let paths = vec![b"/system/etc/hosts".to_vec()];
    let pages = build_del_rule_payloads(&paths, 1000).unwrap();

    assert_eq!(pages.len(), 1);
    assert_eq!(
        u32::from_le_bytes(pages[0][8..12].try_into().unwrap()),
        NmCommand::DelRule as u32
    );
    assert_eq!(
        u32::from_le_bytes(pages[0][12..16].try_into().unwrap()),
        1000
    );
    let buffer = &pages[0][28..];
    assert_eq!(u32::from_le_bytes(buffer[0..4].try_into().unwrap()), 1000);
    assert_eq!(
        u16::from_le_bytes(buffer[4..6].try_into().unwrap()),
        "/system/etc/hosts".len() as u16
    );
    assert_eq!(&buffer[6..6 + 17], b"/system/etc/hosts");
    assert_eq!(
        u32::from_le_bytes(pages[0][24..28].try_into().unwrap()) as usize,
        DEL_HEADER_LEN + "/system/etc/hosts".len()
    );
}

#[test]
fn del_rule_payloads_split_when_buffer_is_full() {
    let path = vec![b'a'; 100];
    let paths = vec![path; 100];
    let pages = build_del_rule_payloads(&paths, 0).unwrap();

    assert!(pages.len() > 1);
    for page in &pages {
        assert_eq!(page.len(), PAYLOAD_LEN);
        let data_size = u32::from_le_bytes(page[24..28].try_into().unwrap()) as usize;
        assert!(data_size <= BUFFER_LEN);
        assert_eq!(data_size % (DEL_HEADER_LEN + 100), 0);
    }
}

#[test]
fn del_rule_rejects_path_that_does_not_fit_one_page() {
    let paths = vec![vec![b'a'; BUFFER_LEN + 1 - DEL_HEADER_LEN]];
    let err = build_del_rule_payloads(&paths, 0).unwrap_err();
    assert!(matches!(err, crate::errors::Error::VfsProtocol { .. }));
}

#[test]
fn ensure_status_allow_enoent_accepts_missing_rule() {
    assert!(ensure_status_allow_enoent(&response(-2, 0, 0)).is_ok());
}

#[test]
fn ensure_status_allow_enoent_rejects_other_negative_status() {
    let err = ensure_status_allow_enoent(&response(-22, 0, 0)).unwrap_err();
    assert!(matches!(err, crate::errors::Error::VfsProtocol { .. }));
    assert!(!format!("{err}").is_empty());
}

/// An out-of-range record length leaves status at 0 with the cursor mid-batch; trusting
/// status alone would read a partial delete as a complete one.
#[test]
fn ensure_status_allow_enoent_rejects_truncated_batch() {
    let err = ensure_status_allow_enoent(&response(0, 6, 16)).unwrap_err();
    assert!(matches!(err, crate::errors::Error::VfsProtocol { .. }));
}

#[test]
fn ensure_status_allow_enoent_accepts_full_batch_cursor() {
    assert!(ensure_status_allow_enoent(&response(0, 16, 16)).is_ok());
}

/// Adding an already-isolated uid is idempotent, not an error.
#[test]
fn ensure_status_accepts_already_isolated_uid() {
    assert!(ensure_status_allow_eexist(&response(-17, 0, 0)).is_ok());
}

#[test]
fn ensure_consumed_accepts_full_batch_cursor() {
    assert!(ensure_consumed(&response(0, 16, 16)).is_ok());
}

#[test]
fn ensure_consumed_rejects_partial_batch_cursor() {
    let err = ensure_consumed(&response(0, 8, 16)).unwrap_err();
    assert!(matches!(err, crate::errors::Error::VfsProtocol { .. }));
}

#[test]
fn ensure_consumed_reports_first_failure_and_its_offset() {
    let err = ensure_consumed(&response(-22, 12, 16)).unwrap_err();
    let text = format!("{err}");
    assert!(text.contains("-22"), "error was {text}");
    assert!(text.contains("offset 12"), "error was {text}");
}

/// A response page carrying real buffer content, as GET_LIST / GET_UIDS return it.
fn page_with_data(status: i32, cursor: u32, data: &[u8]) -> Vec<u8> {
    let mut page = response(status, cursor, data.len() as u32);
    page[28..28 + data.len()].copy_from_slice(data);
    page
}

fn listed_record(flags: u32, uid: u32, virtual_path: &str, real_path: &str) -> Vec<u8> {
    let mut record = Vec::new();
    record.extend_from_slice(&flags.to_le_bytes());
    record.extend_from_slice(&uid.to_le_bytes());
    record.extend_from_slice(&(virtual_path.len() as u16).to_le_bytes());
    record.extend_from_slice(&(real_path.len() as u16).to_le_bytes());
    record.extend_from_slice(virtual_path.as_bytes());
    record.extend_from_slice(real_path.as_bytes());
    record
}

#[test]
fn parse_list_reads_records_and_the_next_cursor() {
    let mut data = listed_record(0, 0, "/system/etc/hosts", "/data/adb/modules/m/hosts");
    data.extend(listed_record(FLAG_WHITEOUT, 10123, "/system/app/Foo", ""));
    let page = page_with_data(0, 2, &data);

    let (rules, cursor) = parse_list(&page).unwrap();

    assert_eq!(cursor, 2, "arg1 is the index of the next rule to read");
    assert_eq!(rules.len(), 2);
    assert_eq!(rules[0].flags, 0);
    assert_eq!(rules[0].uid, 0);
    assert_eq!(rules[0].virtual_path, "/system/etc/hosts");
    assert_eq!(rules[0].real_path, "/data/adb/modules/m/hosts");
    assert_eq!(rules[1].flags, FLAG_WHITEOUT);
    assert_eq!(rules[1].uid, 10123);
    assert_eq!(rules[1].virtual_path, "/system/app/Foo");
    assert!(rules[1].real_path.is_empty());
}

#[test]
fn parse_list_rejects_negative_status() {
    let err = parse_list(&page_with_data(-22, 0, &[])).unwrap_err();
    assert!(matches!(err, crate::errors::Error::VfsProtocol { .. }));
}

#[test]
fn parse_list_rejects_a_truncated_record_header() {
    let page = page_with_data(0, 1, &[0_u8; RULE_HEADER_LEN - 1]);
    let err = parse_list(&page).unwrap_err();
    assert!(matches!(err, crate::errors::Error::VfsProtocol { .. }));
}

#[test]
fn parse_list_rejects_paths_that_exceed_the_reported_length() {
    // Claims 64 bytes of paths but the record body is empty.
    let mut data = vec![0_u8; RULE_HEADER_LEN];
    data[8..10].copy_from_slice(&32_u16.to_le_bytes());
    data[10..12].copy_from_slice(&32_u16.to_le_bytes());
    let err = parse_list(&page_with_data(0, 1, &data)).unwrap_err();
    assert!(matches!(err, crate::errors::Error::VfsProtocol { .. }));
}

#[test]
fn parse_list_rejects_data_size_beyond_the_buffer() {
    let mut page = response(0, 1, BUFFER_LEN as u32 + 1);
    page[28..32].copy_from_slice(b"junk");
    let err = parse_list(&page).unwrap_err();
    assert!(matches!(err, crate::errors::Error::VfsProtocol { .. }));
}

#[test]
fn parse_uids_reads_words_and_the_next_cursor() {
    let mut data = Vec::new();
    for uid in [1000_u32, 10123] {
        data.extend_from_slice(&uid.to_le_bytes());
    }

    let (uids, cursor) = parse_uids(&page_with_data(0, 2, &data)).unwrap();

    assert_eq!(uids, vec![1000, 10123]);
    assert_eq!(cursor, 2);
}

#[test]
fn parse_uids_rejects_a_length_that_is_not_whole_words() {
    let err = parse_uids(&page_with_data(0, 0, &[0_u8; 6])).unwrap_err();
    assert!(matches!(err, crate::errors::Error::VfsProtocol { .. }));
}

#[test]
fn parse_uids_rejects_negative_status() {
    let err = parse_uids(&page_with_data(-13, 0, &[])).unwrap_err();
    assert!(matches!(err, crate::errors::Error::VfsProtocol { .. }));
}

#[test]
fn build_list_payload_writes_the_cursor_into_arg1() {
    let page = build_list_payload(NmCommand::GetList, 7).unwrap();

    assert_eq!(
        u32::from_le_bytes(page[8..12].try_into().unwrap()),
        NmCommand::GetList as u32
    );
    assert_eq!(u32::from_le_bytes(page[20..24].try_into().unwrap()), 7);
    // A listing request carries no input records.
    assert_eq!(u32::from_le_bytes(page[24..28].try_into().unwrap()), 0);
}

#[test]
fn paginate_walks_the_cursor_until_the_kernel_returns_an_empty_batch() {
    let pages = [
        page_with_data(0, 2, &listed_record(0, 0, "/a", "/x")),
        page_with_data(0, 3, &listed_record(0, 0, "/b", "/y")),
        page_with_data(0, 3, &[]),
    ];
    let mut index = 0;

    let rules = paginate(
        |_cursor| {
            let page = pages[index].clone();
            index += 1;
            Ok(page)
        },
        parse_list,
    )
    .unwrap();

    assert_eq!(index, 3, "stops at the first empty batch");
    assert_eq!(rules.len(), 2);
    assert_eq!(rules[1].virtual_path, "/b");
}

#[test]
fn paginate_asks_for_the_next_cursor_the_kernel_reported() {
    let mut seen = Vec::new();
    let pages = [
        page_with_data(0, 4, &listed_record(0, 0, "/a", "/x")),
        page_with_data(0, 4, &[]),
    ];
    let mut index = 0;

    paginate(
        |cursor| {
            seen.push(cursor);
            let page = pages[index].clone();
            index += 1;
            Ok(page)
        },
        parse_list,
    )
    .unwrap();

    assert_eq!(seen, vec![0, 4]);
}

#[test]
fn paginate_rejects_a_cursor_that_does_not_advance() {
    // A kernel that keeps reporting the same cursor would loop forever. The first page
    // still moves 0 -> 3, so the stall only shows up once the cursor is re-requested.
    let mut calls = 0;
    let err = paginate(
        |_cursor| {
            calls += 1;
            Ok(page_with_data(0, 3, &listed_record(0, 0, "/a", "/x")))
        },
        parse_list,
    )
    .unwrap_err();

    let text = format!("{err}");
    assert!(text.contains("cursor"), "error was {text}");
    assert_eq!(calls, 2, "gives up as soon as the cursor stops moving");
}
