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
    assert_eq!(i32::from_le_bytes(page[16..20].try_into().unwrap()), -1_i32);
    assert_eq!(u32::from_le_bytes(page[20..24].try_into().unwrap()), 0);
    assert_eq!(u32::from_le_bytes(page[24..28].try_into().unwrap()), 0);
}

/// wire magic 是 HM 专属值，必须与内核头文件的 `HYBRIDMOUNT_MAGIC_SIG` 逐字节一致。
///
/// `payload_has_exact_wire_layout` 里的 `page[0..8] == MAGIC.to_le_bytes()` 是自引用断言：
/// 常量改了它照样通过，因此发现不了「只改一边」或「两边一起改错」。这里把字面量钉死——
/// 改动 wire 契约必须先改这条测试，也就必须同时想到内核那一边。
#[test]
fn wire_magic_is_pinned_to_the_hm_value() {
    assert_eq!(
        MAGIC, 0x4859_4252_4944_4D4F,
        "wire magic 必须与 module/vfs/src/hybridmount.h 的 HYBRIDMOUNT_MAGIC_SIG 相同"
    );
    // 常量按上游约定是 ASCII 串的大端读数（"HYBRIDMO"），小端机上落盘即反向字节。
    let page = build_payload(NmCommand::GetVersion, 0, &[]).unwrap();
    assert_eq!(&page[0..8], b"OMDIRBYH", "小端序写入后 wire 上的 8 字节");
}

/// 直接读内核头文件核对常量，防止用户态与内核单侧漂移。
///
/// 上面那条测试只能发现「用户态被改错」，发现不了「内核被改动而用户态没跟上」——
/// 而两者不一致时内核会在 preparse 阶段回 -EFAULT，表现为 VFS 静默降级（规则一条都
/// 不生效），是最难定位的一类故障。这里沿用 defs.rs 里 metainstall.sh 交叉校验的做法。
#[test]
fn kernel_header_magic_and_version_match_userspace() {
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
    // build_payload 的 status 是 -1 哨兵，这里模拟内核已回写成功状态。
    page[16..20].copy_from_slice(&0_i32.to_le_bytes());
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
    let mut page = build_payload(NmCommand::DelRule, 0, &[]).unwrap();
    page[16..20].copy_from_slice(&(-2_i32).to_le_bytes());
    assert!(ensure_status_allow_enoent(&page).is_ok());
}

#[test]
fn ensure_status_allow_enoent_rejects_other_negative_status() {
    let mut page = build_payload(NmCommand::DelRule, 0, &[]).unwrap();
    page[16..20].copy_from_slice(&(-22_i32).to_le_bytes());
    let err = ensure_status_allow_enoent(&page).unwrap_err();
    assert!(matches!(err, crate::errors::Error::VfsProtocol { .. }));
    assert!(!format!("{err}").is_empty());
}

#[test]
fn ensure_status_allow_enoent_rejects_truncated_batch() {
    // DEL_RULE 批内长度越界时内核 break，status 仍为 0 而 arg1 停在截断处；
    // 只看 status 会把「还有规则没删掉」误判成回滚成功。
    let mut page = build_payload(NmCommand::DelRule, 0, &[0_u8; 16]).unwrap();
    page[16..20].copy_from_slice(&0_i32.to_le_bytes());
    page[20..24].copy_from_slice(&6_u32.to_le_bytes());
    page[24..28].copy_from_slice(&16_u32.to_le_bytes());
    let err = ensure_status_allow_enoent(&page).unwrap_err();
    assert!(matches!(err, crate::errors::Error::VfsProtocol { .. }));
}

#[test]
fn ensure_status_allow_enoent_accepts_full_batch_cursor() {
    let mut page = build_payload(NmCommand::DelRule, 0, &[0_u8; 16]).unwrap();
    page[16..20].copy_from_slice(&0_i32.to_le_bytes());
    page[20..24].copy_from_slice(&16_u32.to_le_bytes());
    page[24..28].copy_from_slice(&16_u32.to_le_bytes());
    assert!(ensure_status_allow_enoent(&page).is_ok());
}

#[test]
fn ensure_status_accepts_already_isolated_uid() {
    // ADD_UID 是幂等操作：同一次启动内第二次运行流水线时 UID 已在表内，
    // 内核回 -EEXIST，这不是错误。
    let mut page = build_payload(NmCommand::AddUid, 1000, &[]).unwrap();
    page[16..20].copy_from_slice(&(-17_i32).to_le_bytes());
    assert!(ensure_status_allow_eexist(&page).is_ok());
}

#[test]
fn ensure_consumed_accepts_full_batch_cursor() {
    let mut page = build_payload(NmCommand::AddRule, 0, &[0_u8; 16]).unwrap();
    page[16..20].copy_from_slice(&0_i32.to_le_bytes());
    page[20..24].copy_from_slice(&16_u32.to_le_bytes());
    page[24..28].copy_from_slice(&16_u32.to_le_bytes());
    assert!(ensure_consumed(&page).is_ok());
}

#[test]
fn ensure_consumed_rejects_partial_batch_cursor() {
    let mut page = build_payload(NmCommand::AddRule, 0, &[0_u8; 16]).unwrap();
    page[16..20].copy_from_slice(&0_i32.to_le_bytes());
    page[20..24].copy_from_slice(&8_u32.to_le_bytes());
    page[24..28].copy_from_slice(&16_u32.to_le_bytes());
    let err = ensure_consumed(&page).unwrap_err();
    assert!(matches!(err, crate::errors::Error::VfsProtocol { .. }));
}

#[test]
fn ensure_consumed_reports_first_failure_and_its_offset() {
    let mut page = build_payload(NmCommand::AddRule, 0, &[0_u8; 16]).unwrap();
    page[16..20].copy_from_slice(&(-22_i32).to_le_bytes());
    page[20..24].copy_from_slice(&12_u32.to_le_bytes());
    page[24..28].copy_from_slice(&16_u32.to_le_bytes());
    let err = ensure_consumed(&page).unwrap_err();
    let text = format!("{err}");
    assert!(text.contains("-22"), "error was {text}");
    assert!(text.contains("offset 12"), "error was {text}");
}
