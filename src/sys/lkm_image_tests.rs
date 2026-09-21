// SPDX-License-Identifier: GPL-3.0-only

use super::*;
use std::io::{self, Cursor};

const MODULE: &[u8] = include_bytes!("../../module/vfs/binaries/hybridmount-android13-5.15.ko");

#[test]
fn resolves_core_symbols_without_using_hidden_or_module_owned_addresses() {
    let mut image = ModuleImage::parse(MODULE.to_vec()).unwrap();
    let kallsyms = "0000000000000000 T kern_path\nffff800000001000 T kern_path [other_driver]\nffff800000002000 T kern_path.llvm.123\nffff800000003000 T kern_path\n";
    assert_eq!(image.resolve_symbols(Cursor::new(kallsyms)).unwrap(), 1);
    let file = object::File::parse(image.bytes.as_slice()).unwrap();
    let symbol = file
        .symbols()
        .find(|s| s.name() == Ok("kern_path"))
        .unwrap();
    assert_eq!(symbol.address(), 0xffff800000003000);
    assert_eq!(symbol.section(), object::SymbolSection::Absolute);
}

#[test]
fn ambiguous_symbols_are_left_for_the_kernel_to_resolve() {
    let mut image = ModuleImage::parse(MODULE.to_vec()).unwrap();
    assert_eq!(
        image
            .resolve_symbols(Cursor::new(
                "ffff800000001000 T kern_path\nffff800000002000 T kern_path\n"
            ))
            .unwrap(),
        0
    );
    assert_eq!(image.bytes, MODULE);
}

#[test]
fn grows_vermagic_without_changing_the_packaged_image_or_other_metadata() {
    let mut image = ModuleImage::parse(MODULE.to_vec()).unwrap();
    let required = "5.15.197-@Coolpak@Kugouzei_NB_LKM SMP preempt mod_unload modversions aarch64";
    image.patch_vermagic(required).unwrap();
    let reparsed = ModuleImage::parse(image.bytes).unwrap();
    assert_eq!(reparsed.vermagic, required);
    assert_eq!(reparsed.name, "hybridmount");
    for entry in image
        .modinfo
        .split(|b| *b == 0)
        .filter(|entry| !entry.is_empty() && !entry.starts_with(b"vermagic="))
    {
        assert!(
            reparsed
                .modinfo
                .split(|b| *b == 0)
                .any(|actual| actual == entry)
        );
    }
    assert_ne!(
        ModuleImage::parse(MODULE.to_vec()).unwrap().vermagic,
        required
    );
}

#[test]
fn malformed_and_wrong_architecture_images_are_rejected() {
    for length in [0, 1, 8, 63, 64, 128] {
        assert!(ModuleImage::parse(MODULE[..length].to_vec()).is_err());
    }
    let mut wrong_arch = MODULE.to_vec();
    wrong_arch[18..20].copy_from_slice(&62_u16.to_le_bytes()); // x86_64
    assert!(ModuleImage::parse(wrong_arch).is_err());
    let mut overflow = MODULE.to_vec();
    overflow[40..48].copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(ModuleImage::parse(overflow).is_err());
}

#[test]
fn vermagic_diagnostic_must_match_both_module_and_attempted_version() {
    let image = ModuleImage::parse(MODULE.to_vec()).unwrap();
    let record = format!(
        "4,123,456,-;hybridmount: version magic '{}' should be 'custom SMP'\n",
        image.vermagic
    );
    assert_eq!(
        image.requested_vermagic(&record).as_deref(),
        Some("custom SMP")
    );
    assert!(
        image
            .requested_vermagic(&record.replace("hybridmount:", "other_driver:"))
            .is_none()
    );
    assert!(
        image
            .requested_vermagic(&record.replace("hybridmount:", "other_hybridmount:"))
            .is_none()
    );
    assert!(
        image
            .requested_vermagic(&record.replace(&image.vermagic, "stale-version"))
            .is_none()
    );
}

#[test]
fn retries_only_once_after_an_explicit_vermagic_rejection() {
    let mut image = ModuleImage::parse(MODULE.to_vec()).unwrap();
    let mut calls = 0;
    let result = load_with_vermagic_retry(
        &mut image,
        |bytes| {
            calls += 1;
            if calls == 1 {
                return Err(io::Error::from_raw_os_error(8));
            }
            assert_eq!(
                ModuleImage::parse(bytes.to_vec()).unwrap().vermagic,
                "custom SMP"
            );
            Ok(())
        },
        |_| Some("custom SMP".into()),
    );
    assert!(result.is_ok());
    assert_eq!(calls, 2);
}

#[test]
fn does_not_retry_success_or_errors_unrelated_to_vermagic() {
    for errno in [None, Some(17), Some(1), Some(22)] {
        let mut image = ModuleImage::parse(MODULE.to_vec()).unwrap();
        let mut calls = 0;
        let result = load_with_vermagic_retry(
            &mut image,
            |_| {
                calls += 1;
                errno.map_or(Ok(()), |errno| Err(io::Error::from_raw_os_error(errno)))
            },
            |_| panic!("must not inspect unrelated kernel diagnostics"),
        );
        assert_eq!(result.is_ok(), errno.is_none());
        assert_eq!(calls, 1);
    }
}

#[test]
fn incompatible_abi_remains_an_error_after_vermagic_is_adapted() {
    let mut image = ModuleImage::parse(MODULE.to_vec()).unwrap();
    let mut calls = 0;
    assert!(
        load_with_vermagic_retry(
            &mut image,
            |_| {
                calls += 1;
                Err(io::Error::from_raw_os_error(8))
            },
            |_| Some("custom SMP".into())
        )
        .is_err()
    );
    assert_eq!(calls, 2);
}

#[test]
fn missing_vermagic_diagnostic_does_not_retry_or_modify_the_image() {
    let mut image = ModuleImage::parse(MODULE.to_vec()).unwrap();
    let mut calls = 0;
    assert!(
        load_with_vermagic_retry(
            &mut image,
            |_| {
                calls += 1;
                Err(io::Error::from_raw_os_error(8))
            },
            |_| None
        )
        .is_err()
    );
    assert_eq!(calls, 1);
    assert_eq!(image.bytes, MODULE);
}

#[test]
fn every_shipped_module_can_be_adapted_without_losing_its_identity() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for (directory, expected_count) in [("module/vfs/binaries", 7), ("module/lkm/binaries", 8)] {
        let mut count = 0;
        for entry in std::fs::read_dir(root.join(directory)).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|ext| ext != "ko") {
                continue;
            }
            let bytes = std::fs::read(&path).unwrap();
            let mut image =
                ModuleImage::parse(bytes).unwrap_or_else(|err| panic!("{}: {err}", path.display()));
            let original_name = image.name.clone();
            image
                .patch_vermagic("custom SMP preempt mod_unload aarch64")
                .unwrap();
            let reparsed = ModuleImage::parse(image.bytes).unwrap();
            assert_eq!(reparsed.name, original_name);
            assert_eq!(reparsed.vermagic, "custom SMP preempt mod_unload aarch64");
            count += 1;
        }
        assert_eq!(count, expected_count, "{directory}");
    }
}
