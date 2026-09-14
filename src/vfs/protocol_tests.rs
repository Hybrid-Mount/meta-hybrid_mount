// SPDX-License-Identifier: GPL-3.0-only

use super::*;
use crate::module_id::ModuleId;
use crate::vfs::rule::{VfsAction, VfsRule};
use std::path::PathBuf;

#[test]
fn payload_has_exact_wire_layout() {
    let page = build_payload(NmCommand::GetVersion, 0, &[]).unwrap();
    assert_eq!(page.len(), PAYLOAD_LEN);
    assert_eq!(&page[0..8], &MAGIC.to_le_bytes());
    assert_eq!(u32::from_le_bytes(page[8..12].try_into().unwrap()), 1);
    assert_eq!(u32::from_le_bytes(page[12..16].try_into().unwrap()), 0);
    assert_eq!(i32::from_le_bytes(page[16..20].try_into().unwrap()), 0);
    assert_eq!(u32::from_le_bytes(page[20..24].try_into().unwrap()), 0);
    assert_eq!(u32::from_le_bytes(page[24..28].try_into().unwrap()), 0);
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
    page[28..30].copy_from_slice(b"20");
    page[24..28].copy_from_slice(&2_u32.to_le_bytes());
    assert_eq!(parse_version(&page).unwrap(), "20");
}

#[test]
fn ensure_status_rejects_negative_kernel_status() {
    let mut page = build_payload(NmCommand::AddRule, 0, &[]).unwrap();
    page[16..20].copy_from_slice(&(-22_i32).to_le_bytes());
    let err = ensure_status(&page).unwrap_err();
    assert!(matches!(err, crate::errors::Error::VfsProtocol { .. }));
}
